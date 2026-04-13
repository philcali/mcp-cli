//! Tool list and execution handlers.

use crate::protocol::*;
use crate::server::CredentialResolver;
use anyhow::{Context, Result};
use serde_json::json;
use tracing::{debug, info};

/// List available tools.
pub async fn handle_tools_list(server: &crate::server::McpServer) -> Result<serde_json::Value> {
    let mut cached = server.cached_tools.lock().unwrap();

    if cached.is_empty() && server.tools_dir.is_some() {
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

    let (script_path, auth_config) = {
        let cached = server.cached_tools.lock().unwrap();
        if let Some(tool) = cached.get(&call_params.name) {
            (tool.script_path.clone(), tool.auth_config.clone())
        } else {
            drop(cached);
            let mut cached = server.cached_tools.lock().unwrap();
            *cached = server.load_tools()?;
            match cached.get(&call_params.name) {
                Some(tool) => (tool.script_path.clone(), tool.auth_config.clone()),
                None => return Err(anyhow::anyhow!("Tool '{}' not found", call_params.name)),
            }
        }
    };

    if let Some(ref _config) = auth_config
        && let Some(ref tools_dir) = server.tools_dir
    {
        match CredentialResolver::resolve_for_tool(tools_dir, &call_params.name) {
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
