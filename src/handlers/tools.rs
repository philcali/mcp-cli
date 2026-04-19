//! Tool list and execution handlers.

use crate::protocol::*;
use anyhow::{Context, Result};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tracing::{debug, info};

/// List available tools.
pub async fn handle_tools_list(server: &crate::server::McpServer) -> Result<serde_json::Value> {
    let mut cached = server.state.cached_tools.lock().unwrap();

    if cached.is_empty() && server.state.tools_dir.is_some() {
        *cached = server.load_tools()?;
    }

    let tool_list: Vec<_> = cached
        .values()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
            })
        })
        .collect();

    Ok(json!({ "tools": tool_list }))
}

/// Execute a tool with the given arguments.
pub async fn handle_tools_call(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let call_params: CallToolParams =
        serde_json::from_value(params.clone()).context("Failed to parse tool call parameters")?;

    // Check if streaming was requested
    if call_params.is_streaming() {
        return handle_tools_call_streaming(server, &call_params).await;
    }

    let (script_path, auth_config) = {
        let cached = server.state.cached_tools.lock().unwrap();
        if let Some(tool) = cached.get(&call_params.name) {
            (tool.script_path.clone(), tool.auth_config.clone())
        } else {
            drop(cached);
            let mut cached = server.state.cached_tools.lock().unwrap();
            *cached = server.load_tools()?;
            match cached.get(&call_params.name) {
                Some(tool) => (tool.script_path.clone(), tool.auth_config.clone()),
                None => return Err(anyhow::anyhow!("Tool '{}' not found", call_params.name)),
            }
        }
    };

    if let Some(ref _config) = auth_config
        && let Some(ref tools_dir) = server.state.tools_dir
    {
        match crate::auth::resolve_credentials(tools_dir, &call_params.name) {
            Ok(creds) => {
                debug!(
                    "Resolved {} credential(s) for tool '{}'",
                    creds.len(),
                    call_params.name
                );

                if !creds.is_empty() {
                    info!(
                        "Credentials validated successfully for tool '{}'",
                        call_params.name
                    );
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Credential resolution failed for tool '{}': {}",
                    call_params.name,
                    e
                ));
            }
        }
    }

    let input = json!({
        "name": call_params.name,
        "arguments": call_params.arguments,
    });

    debug!(
        "Executing tool from {:?} with input: {}",
        script_path, input
    );

    let mut child = tokio::process::Command::new(&script_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn tool script")?;

    let (stdout_result, stderr_output) = {
        let mut stdin = child.stdin.take().context("Failed to open stdin")?;
        use tokio::io::AsyncWriteExt;
        stdin.write_all(input.to_string().as_bytes()).await?;
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

    const TOOL_TIMEOUT_SECS: u64 = 30;
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
        Ok(Err(_)) => {
            let _ = child.kill().await;
            return Err(anyhow::anyhow!(
                "Tool '{}' timed out after {} seconds",
                call_params.name,
                TOOL_TIMEOUT_SECS
            ));
        }
        Err(_) => {
            let _ = child.kill().await;
            return Err(anyhow::anyhow!(
                "Failed to wait for tool '{}' to complete",
                call_params.name
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
pub async fn handle_tools_call_streaming(
    server: &crate::server::McpServer,
    call_params: &CallToolParams,
) -> Result<serde_json::Value> {
    let (script_path, _auth_config) = {
        let cached = server.state.cached_tools.lock().unwrap();
        if let Some(tool) = cached.get(&call_params.name) {
            (tool.script_path.clone(), tool.auth_config.clone())
        } else {
            drop(cached);
            let mut cached = server.state.cached_tools.lock().unwrap();
            *cached = server.load_tools()?;
            match cached.get(&call_params.name) {
                Some(tool) => (tool.script_path.clone(), tool.auth_config.clone()),
                None => return Err(anyhow::anyhow!("Tool '{}' not found", call_params.name)),
            }
        }
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
    let mut child = tokio::process::Command::new(&script_path)
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
    const TOOL_TIMEOUT_SECS: u64 = 30;
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
        Ok(Err(_)) => {
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
        Err(_) => {
            let _ = child.kill().await;

            // Send error notification
            if let Some(ref tx) = notification_tx {
                let _ = tx.send(json!({
                    "jsonrpc": "2.0",
                    "method": "tools/stream",
                    "params": {
                        "request_id": stream_id,
                        "chunk": {"type": "done", "summary": "Failed to wait for tool completion"}
                    }
                }).to_string());
            }

            Err(anyhow::anyhow!(
                "Failed to wait for tool '{}' to complete",
                call_params.name
            ))
        }
    }
}
