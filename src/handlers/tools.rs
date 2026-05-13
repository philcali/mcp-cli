//! Tool list and execution handlers.

use crate::protocol::{CallToolParams, TaskSupportLevel, ToolAuthConfig};
use anyhow::{Context, Result};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tracing::{debug, info};

const TOOL_TIMEOUT_SECS: u64 = 30;

/// Look up a tool by name, reloading the cache if needed.
async fn find_tool(
    server: &crate::server::McpServer,
    name: &str,
) -> Result<(std::path::PathBuf, Option<ToolAuthConfig>)> {
    let found = {
        let cached = server.state.cached_tools.lock().unwrap();
        cached
            .get(name)
            .map(|t| (t.script_path.clone(), t.auth_config.clone()))
    };

    if let Some(tool) = found {
        return Ok(tool);
    }

    let tools = server.load_tools().await?;
    let mut cached = server.state.cached_tools.lock().unwrap();
    *cached = tools;
    match cached.get(name) {
        Some(tool) => Ok((tool.script_path.clone(), tool.auth_config.clone())),
        None => Err(anyhow::anyhow!("Tool '{}' not found", name)),
    }
}

/// List available tools.
pub async fn handle_tools_list(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let list_params: crate::protocol::ListToolsParams =
        serde_json::from_value(params.clone()).unwrap_or_default();

    let need_load = {
        let cached = server.state.cached_tools.lock().unwrap();
        cached.is_empty() && server.state.tools_dir.is_some()
    };

    if need_load {
        let tools = server.load_tools().await?;
        let mut cached = server.state.cached_tools.lock().unwrap();
        *cached = tools;
    }

    let cached = server.state.cached_tools.lock().unwrap();

    let mut tool_list: Vec<_> = cached
        .values()
        .map(|t| {
            let mut tool_obj = json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": t.input_schema,
            });
            if let Some(ref ts) = t.task_support {
                let exec = json!({
                    "taskSupport": match ts {
                        crate::protocol::TaskSupportLevel::Forbidden => "forbidden",
                        crate::protocol::TaskSupportLevel::Optional => "optional",
                        crate::protocol::TaskSupportLevel::Required => "required",
                    }
                });
                tool_obj["execution"] = exec;
            }
            tool_obj
        })
        .collect();

    // Filter by tool_names if provided
    if let Some(ref names) = list_params.tool_names {
        tool_list.retain(|t| {
            names
                .iter()
                .any(|n| t.get("name").and_then(|v| v.as_str()) == Some(n.as_str()))
        });
    }

    // Apply pagination
    let (sliced, next_cursor) =
        paginate_list(&tool_list, list_params.cursor.as_deref(), DEFAULT_PAGE_SIZE);

    Ok(json!({
        "tools": sliced,
        "nextCursor": next_cursor,
    }))
}

/// Slice a list by cursor position and return next cursor if more items remain.
pub fn paginate_list<'a, T>(
    items: &'a [T],
    cursor: Option<&str>,
    page_size: usize,
) -> (Vec<&'a T>, Option<String>) {
    let start = cursor.and_then(|c| c.parse::<usize>().ok()).unwrap_or(0);

    let end = (start + page_size).min(items.len());
    let sliced = items[start..end].iter().collect();
    let next_cursor = if end < items.len() {
        Some(end.to_string())
    } else {
        None
    };

    (sliced, next_cursor)
}

pub const DEFAULT_PAGE_SIZE: usize = 100;

/// Execute a tool with the given arguments.
pub async fn handle_tools_call(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let call_params: CallToolParams =
        serde_json::from_value(params.clone()).context("Failed to parse tool call parameters")?;

    // Check if task augmentation was requested
    if call_params.task.is_some() {
        return handle_tools_call_as_task(server, &call_params, params).await;
    }

    // Check if streaming was requested
    if call_params.is_streaming() {
        return handle_tools_call_streaming(server, &call_params).await;
    }

    let (script_path, auth_config) = find_tool(server, &call_params.name).await?;

    let creds = if let Some(ref _config) = auth_config
        && let Some(ref tools_dir) = server.state.tools_dir
    {
        match crate::auth::resolve_credentials(
            &server.state.oauth_cache,
            tools_dir,
            &call_params.name,
        )
        .await
        {
            Ok(creds) => {
                debug!(
                    "Resolved {} credential(s) for tool '{}'",
                    creds.len(),
                    call_params.name
                );

                if !creds.is_empty() {
                    info!(
                        "Credentials resolved successfully for tool '{}'",
                        call_params.name
                    );
                }
                creds
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Credential resolution failed for tool '{}': {}",
                    call_params.name,
                    e
                ));
            }
        }
    } else {
        std::collections::HashMap::new()
    };

    let input = json!({
        "name": call_params.name,
        "arguments": call_params.arguments,
    });

    debug!(
        "Executing tool from {:?} with input: {}",
        script_path, input
    );

    let mut cmd = tokio::process::Command::new(&script_path);
    for (k, v) in &creds {
        cmd.env(k, v);
    }
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn tool script")?;

    let (stdout_result, stderr_output) = {
        let mut stdin = child.stdin.take().context("Failed to open stdin")?;
        use tokio::io::AsyncWriteExt;
        // Handle EPIPE (broken pipe) if the tool exits before we finish writing.
        // This can happen with fast tools that don't read stdin.
        let write_result = stdin.write_all(input.to_string().as_bytes()).await;
        if let Err(e) = write_result {
            if e.kind() == std::io::ErrorKind::BrokenPipe {
                debug!(
                    "Tool '{}' exited before stdin was fully written (broken pipe)",
                    call_params.name
                );
            } else {
                return Err(anyhow::anyhow!(
                    "Failed to write to tool '{}': {}",
                    call_params.name,
                    e
                ));
            }
        }
        drop(stdin);

        let mut stdout = child.stdout.take().context("Failed to open stdout")?;
        let mut stderr = child.stderr.take().context("Failed to open stderr")?;

        let (stdout_res, stderr_res) = tokio::join!(
            async {
                use tokio::io::AsyncReadExt;
                let mut output = String::new();
                stdout.read_to_string(&mut output).await?;
                anyhow::Result::<String>::Ok(output)
            },
            async {
                use tokio::io::AsyncReadExt;
                let mut error_output = String::new();
                stderr.read_to_string(&mut error_output).await?;
                anyhow::Result::<String>::Ok(error_output)
            }
        );

        (stdout_res?, stderr_res?)
    };

    match tokio::time::timeout(
        std::time::Duration::from_secs(TOOL_TIMEOUT_SECS),
        child.wait(),
    )
    .await
    {
        Ok(Ok(status)) => {
            debug!(
                "Tool '{}' exited with status: {} (stdout: {}, stderr: {})",
                call_params.name,
                status,
                stdout_result.len(),
                stderr_output.len()
            );

            if !status.success() {
                return Err(anyhow::anyhow!(
                    "Tool '{}' failed with exit code {:?}\nstderr: {}",
                    call_params.name,
                    status.code(),
                    stderr_output.trim()
                ));
            }
        }
        Ok(Err(e)) => {
            let _ = child.kill().await;
            return Err(anyhow::anyhow!(
                "Tool '{}' process error: {}",
                call_params.name,
                e
            ));
        }
        Err(_) => {
            let _ = child.kill().await;
            return Err(anyhow::anyhow!(
                "Tool '{}' timed out after {} seconds",
                call_params.name,
                TOOL_TIMEOUT_SECS
            ));
        }
    }

    let response = if !stderr_output.is_empty() {
        json!({
            "content": [
                {
                    "type": "text",
                    "text": stdout_result.trim()
                }
            ],
            "stderr": stderr_output.trim()
        })
    } else {
        json!({
            "content": [
                {
                    "type": "text",
                    "text": stdout_result.trim()
                }
            ]
        })
    };

    Ok(response)
}

/// Execute a tool with streaming output.
pub(crate) async fn handle_tools_call_streaming(
    server: &crate::server::McpServer,
    call_params: &CallToolParams,
) -> Result<serde_json::Value> {
    let (script_path, auth_config) = find_tool(server, &call_params.name).await?;

    let creds = if let Some(ref _config) = auth_config
        && let Some(ref tools_dir) = server.state.tools_dir
    {
        match crate::auth::resolve_credentials(
            &server.state.oauth_cache,
            tools_dir,
            &call_params.name,
        )
        .await
        {
            Ok(creds) => creds,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Credential resolution failed for tool '{}': {}",
                    call_params.name,
                    e
                ));
            }
        }
    } else {
        std::collections::HashMap::new()
    };

    // Generate a stream ID
    let stream_id = format!(
        "stream_{}_{}",
        call_params.name,
        std::time::UNIX_EPOCH.elapsed()?.as_nanos()
    );

    debug!(
        "Executing tool with streaming from {:?}, stream_id={}",
        script_path, stream_id
    );

    // Spawn the tool process
    let mut cmd = tokio::process::Command::new(&script_path);
    for (k, v) in &creds {
        cmd.env(k, v);
    }
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn tool script")?;

    // Clone the notification channel for spawned tasks
    let notification_tx = server.notification_tx.clone();

    // Send meta notification using server helper method
    let meta_params = json!({
        "request_id": stream_id,
        "chunk": {"type": "meta", "chunk_count": -1, "total_bytes": None::<usize>}
    });
    server.send_notification("tools/stream", meta_params).await;

    // Extract stdin/stdout/stderr handles
    let mut stdin = child.stdin.take().context("Failed to open stdin")?;
    let stdout = child.stdout.take().context("Failed to open stdout")?;
    let stderr = child.stderr.take().context("Failed to open stderr")?;

    // Write input to tool
    let input = json!({
        "name": call_params.name,
        "arguments": call_params.arguments,
    });
    stdin.write_all(input.to_string().as_bytes()).await?;
    drop(stdin);

    // Spawn async tasks for streaming stdout/stderr
    let stream_id_clone_stdout = stream_id.clone();
    let notification_tx_stdout = notification_tx.clone();
    let stdout_task = tokio::spawn(async move {
        let reader = tokio::io::BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(ref tx) = notification_tx_stdout {
                let _ = tx.send(
                    json!({
                        "jsonrpc": "2.0",
                        "method": "tools/stream",
                        "params": {
                            "request_id": stream_id_clone_stdout,
                            "chunk": {"type": "content", "data": line, "is_error": None::<bool>}
                        }
                    })
                    .to_string(),
                );
            }
        }
    });

    let stream_id_clone_stderr = stream_id.clone();
    let notification_tx_stderr = notification_tx.clone();
    let stderr_task = tokio::spawn(async move {
        let reader = tokio::io::BufReader::new(stderr);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(ref tx) = notification_tx_stderr {
                let _ = tx.send(
                    json!({
                        "jsonrpc": "2.0",
                        "method": "tools/stream",
                        "params": {
                            "request_id": stream_id_clone_stderr,
                            "chunk": {"type": "content", "data": line, "is_error": Some(true)}
                        }
                    })
                    .to_string(),
                );
            }
        }
    });

    // Wait for tool to complete with timeout
    let wait_result = tokio::time::timeout(
        std::time::Duration::from_secs(TOOL_TIMEOUT_SECS),
        child.wait(),
    )
    .await;

    // Wait for stdout/stderr tasks to complete
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    match wait_result {
        Ok(Ok(status)) => {
            debug!("Tool '{}' exited with status: {}", call_params.name, status,);

            if !status.success() {
                // Send error notification
                if let Some(ref tx) = notification_tx {
                    let _ = tx.send(json!({
                        "jsonrpc": "2.0",
                        "method": "tools/stream",
                        "params": {
                            "request_id": stream_id,
                            "chunk": {"type": "done", "summary": format!("Tool failed with exit code: {:?}", status.code())}
                        }
                    }).to_string());
                }

                return Err(anyhow::anyhow!(
                    "Tool '{}' failed with exit code {:?}",
                    call_params.name,
                    status.code()
                ));
            }

            // Send done notification
            if let Some(ref tx) = notification_tx {
                let _ = tx.send(
                    json!({
                        "jsonrpc": "2.0",
                        "method": "tools/stream",
                        "params": {
                            "request_id": stream_id,
                            "chunk": {"type": "done", "summary": None::<String>}
                        }
                    })
                    .to_string(),
                );
            }

            Ok(json!({
                "content": [],
                "is_error": false,
                "stream_id": stream_id
            }))
        }
        Ok(Err(e)) => {
            let _ = child.kill().await;

            // Send error notification
            if let Some(ref tx) = notification_tx {
                let _ = tx.send(
                    json!({
                        "jsonrpc": "2.0",
                        "method": "tools/stream",
                        "params": {
                            "request_id": stream_id,
                            "chunk": {"type": "done", "summary": format!("Process error: {}", e)}
                        }
                    })
                    .to_string(),
                );
            }

            Err(anyhow::anyhow!(
                "Tool '{}' process error: {}",
                call_params.name,
                e
            ))
        }
        Err(_) => {
            let _ = child.kill().await;

            // Send error notification
            if let Some(ref tx) = notification_tx {
                let _ = tx.send(json!({
                    "jsonrpc": "2.0",
                    "method": "tools/stream",
                    "params": {
                        "request_id": stream_id,
                        "chunk": {"type": "done", "summary": format!("Tool timed out after {} seconds", TOOL_TIMEOUT_SECS)}
                    }
                }).to_string());
            }

            Err(anyhow::anyhow!(
                "Tool '{}' timed out after {} seconds",
                call_params.name,
                TOOL_TIMEOUT_SECS
            ))
        }
    }
}

/// Execute a tool as a task — spawns async, returns immediately with task ID.
async fn handle_tools_call_as_task(
    server: &crate::server::McpServer,
    call_params: &CallToolParams,
    _raw_params: &serde_json::Value,
) -> Result<serde_json::Value> {
    // Check if the tool supports tasks
    {
        let cached = server.state.cached_tools.lock().unwrap();
        if let Some(tool) = cached.get(&call_params.name)
            && tool.task_support == Some(TaskSupportLevel::Forbidden)
        {
            return Err(anyhow::anyhow!(
                "Tool '{}' does not support task execution",
                call_params.name
            ));
        }
    }

    let (script_path, auth_config) = find_tool(server, &call_params.name).await?;

    let creds = if let Some(ref _config) = auth_config
        && let Some(ref tools_dir) = server.state.tools_dir
    {
        match crate::auth::resolve_credentials(
            &server.state.oauth_cache,
            tools_dir,
            &call_params.name,
        )
        .await
        {
            Ok(c) => c,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Credential resolution failed for tool '{}': {}",
                    call_params.name,
                    e
                ));
            }
        }
    } else {
        std::collections::HashMap::new()
    };

    let ttl = call_params
        .task
        .as_ref()
        .and_then(|t| t.ttl)
        .or(Some(3600000)); // default 1 hour

    let task_id = server.state.task_manager.create_task("tools/call", ttl);

    let task_manager = server.state.task_manager.clone();

    let input = json!({
        "name": call_params.name,
        "arguments": call_params.arguments,
    });

    // Clone task_id for the spawned task
    let task_id_spawn = task_id.clone();

    // Spawn the tool process asynchronously
    let handle = tokio::spawn(async move {
        let mut cmd = tokio::process::Command::new(&script_path);
        for (k, v) in &creds {
            cmd.env(k, v);
        }

        let mut child = match cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                task_manager.fail_task(
                    &task_id_spawn,
                    json!({
                        "error": format!("Failed to spawn tool: {}", e)
                    }),
                );
                return;
            }
        };

        // Write input
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input.to_string().as_bytes()).await;
        }

        // Read stdout/stderr concurrently
        let (stdout_result, stderr_output) = {
            let mut stdout = match child.stdout.take() {
                Some(s) => s,
                None => {
                    task_manager.fail_task(
                        &task_id_spawn,
                        json!({ "error": "Tool stdout not available" }),
                    );
                    let _ = child.kill().await;
                    return;
                }
            };
            let mut stderr = match child.stderr.take() {
                Some(s) => s,
                None => {
                    task_manager.fail_task(
                        &task_id_spawn,
                        json!({ "error": "Tool stderr not available" }),
                    );
                    let _ = child.kill().await;
                    return;
                }
            };

            let (stdout_res, stderr_res) = tokio::join!(
                async {
                    use tokio::io::AsyncReadExt;
                    let mut output = String::new();
                    match stdout.read_to_string(&mut output).await {
                        Ok(_) => anyhow::Result::<String>::Ok(output),
                        Err(e) => anyhow::Result::<String>::Err(anyhow::anyhow!(e)),
                    }
                },
                async {
                    use tokio::io::AsyncReadExt;
                    let mut error_output = String::new();
                    match stderr.read_to_string(&mut error_output).await {
                        Ok(_) => anyhow::Result::<String>::Ok(error_output),
                        Err(e) => anyhow::Result::<String>::Err(anyhow::anyhow!(e)),
                    }
                }
            );

            match (stdout_res, stderr_res) {
                (Ok(so), Ok(se)) => (so, se),
                (Err(e), _) => {
                    task_manager.fail_task(
                        &task_id_spawn,
                        json!({
                            "error": format!("Tool I/O error: {}", e)
                        }),
                    );
                    let _ = child.kill().await;
                    return;
                }
                (_, Err(e)) => {
                    task_manager.fail_task(
                        &task_id_spawn,
                        json!({
                            "error": format!("Tool I/O error: {}", e)
                        }),
                    );
                    let _ = child.kill().await;
                    return;
                }
            }
        };

        // Wait for tool with longer timeout (1 hour default for tasks)
        const TASK_TOOL_TIMEOUT_SECS: u64 = 3600;
        match tokio::time::timeout(
            std::time::Duration::from_secs(TASK_TOOL_TIMEOUT_SECS),
            child.wait(),
        )
        .await
        {
            Ok(Ok(status)) => {
                if status.success() {
                    task_manager.complete_task(
                        &task_id_spawn,
                        json!({
                            "content": [{ "type": "text", "text": stdout_result.trim() }],
                            "isError": false,
                        }),
                    );
                } else {
                    task_manager.fail_task(
                        &task_id_spawn,
                        json!({
                            "error": format!("Tool failed with exit code {:?}", status.code()),
                            "stderr": stderr_output.trim(),
                        }),
                    );
                }
            }
            _ => {
                let _ = child.kill().await;
                task_manager.fail_task(
                    &task_id_spawn,
                    json!({
                        "error": "Tool execution timed out after 3600 seconds"
                    }),
                );
            }
        }
    });

    // Store abort handle so cancel_task can kill the spawned process
    server.state.task_manager.set_abort_handle(&task_id, handle);

    // Return immediately with the task
    let task = server
        .state
        .task_manager
        .get_task(&task_id)
        .ok_or_else(|| anyhow::anyhow!("Task creation failed"))?;

    Ok(json!({ "task": task }))
}
