//! Sampling handler for LLM message creation.

use crate::protocol::{CreateMessageParams, CreateMessageResult};
use anyhow::{Context, Result};
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::info;

/// Handle sampling/createMessage requests.
pub async fn handle_sampling_create_message(
    server: &mut crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    if server.capabilities.sampling.is_none() {
        return Err(anyhow::anyhow!("Server does not support sampling"));
    }

    let client_caps = server
        .state
        .client_capabilities
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Server not initialized"))?;

    if client_caps.sampling.is_none() {
        return Err(anyhow::anyhow!(
            "Client does not support sampling capability"
        ));
    }

    let sampling_params: CreateMessageParams =
        serde_json::from_value(params.get("params").cloned().unwrap_or_default())
            .context("Failed to parse createMessage parameters")?;

    info!(
        "Forwarding sampling/createMessage to client with {} message(s)",
        sampling_params.messages.len()
    );

    let request_id = format!("sampling_{}", std::time::UNIX_EPOCH.elapsed()?.as_nanos());

    let sampling_request = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "sampling/createMessage",
        "params": {
            "messages": sampling_params.messages,
            "systemPrompt": sampling_params.system_prompt,
            "temperature": sampling_params.temperature,
            "maxTokens": sampling_params.max_tokens,
            "stopSequences": sampling_params.stop_sequences,
            "metadata": sampling_params.metadata,
        }
    });

    // Create oneshot channel for awaiting the client's response
    let (tx, rx) = tokio::sync::oneshot::channel();
    let sender = std::sync::Arc::new(Mutex::new(Some(tx)));

    // Store in server for main loop to use
    server.pending_sampling = Some((request_id.clone(), sender.clone()));

    // Write request to stdout
    let stdout = server
        .stdout
        .as_ref()
        .expect("stdout should be set")
        .clone();
    let mut out = stdout.lock().await;
    let _ = out
        .write_all(format!("{}\n", serde_json::to_string(&sampling_request)?).as_bytes())
        .await;
    let _ = out.flush().await;
    drop(out);

    // Wait for client response
    let response = {
        let mut guard = sender.lock().await;
        let _tx = guard.take().expect("sender should be present");
        drop(guard);
        tokio::time::timeout(std::time::Duration::from_secs(60), rx)
            .await
            .map_err(|_| anyhow::anyhow!("Sampling request timed out after 60 seconds"))?
    }
    .map_err(|_| anyhow::anyhow!("Sampling response channel dropped"))?;

    // Clear pending sampling
    server.pending_sampling = None;

    let result: CreateMessageResult = serde_json::from_value(response)
        .context("Failed to parse sampling response from client")?;

    Ok(json!({
        "model": result.model,
        "stopReason": result.stop_reason,
        "role": result.role,
        "content": result.content_value,
    }))
}
