//! Sampling handler for LLM message creation.

use crate::protocol::{CreateMessageParams, CreateMessageResult};
use anyhow::{Context, Result};
use serde_json::json;
use tracing::info;

/// Handle sampling/createMessage requests.
pub async fn handle_sampling_create_message(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    if server.capabilities.sampling.is_none() {
        return Err(anyhow::anyhow!("Server does not support sampling"));
    }

    let client_caps = server
        .state
        .get_client_capabilities()
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

    // Store in pending_requests for main loop (stdio) or POST handler (HTTP) to resolve
    server.add_pending_request(request_id.clone(), tx).await;

    // Send the sampling request through the notification broadcast channel.
    // For stdio: the notification sender writes it to stdout.
    // For HTTP: the SSE stream picks it up and sends it to the client.
    if let Some(ref notification_tx) = server.notification_tx {
        let _ = notification_tx.send(serde_json::to_string(&sampling_request)?);
    }

    // Wait for client response
    let response = tokio::time::timeout(std::time::Duration::from_secs(60), rx)
        .await
        .map_err(|_| anyhow::anyhow!("Sampling request timed out after 60 seconds"))?
        .map_err(|_| anyhow::anyhow!("Sampling response channel dropped"))?;

    let result: CreateMessageResult = serde_json::from_value(response)
        .context("Failed to parse sampling response from client")?;

    Ok(json!({
        "model": result.model,
        "stopReason": result.stop_reason,
        "role": result.role,
        "content": result.content_value,
    }))
}
