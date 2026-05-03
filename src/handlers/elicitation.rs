//! Elicitation handler for requesting user input from the client.

use crate::protocol::{ElicitationCreateParams, ElicitationMode, ElicitationResult};
use anyhow::{Context, Result};
use serde_json::json;
use tracing::info;

/// Handle elicitation/create requests.
///
/// Elicitation allows the server to request structured data from users
/// through the client. Form mode collects data via JSON schema, url mode
/// directs users to external URLs for sensitive interactions.
pub async fn handle_elicitation_create(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    // Check client supports elicitation
    let client_caps = server
        .state
        .get_client_capabilities()
        .ok_or_else(|| anyhow::anyhow!("Server not initialized"))?;

    if client_caps.elicitation.is_none() {
        return Err(anyhow::anyhow!(
            "Client does not support elicitation capability"
        ));
    }

    let elicitation_params: ElicitationCreateParams =
        serde_json::from_value(params.clone()).context("Failed to parse elicitation parameters")?;

    let mode = elicitation_params
        .mode
        .clone()
        .unwrap_or(ElicitationMode::Form);

    // Validate required fields based on mode
    match &mode {
        ElicitationMode::Form => {
            if client_caps
                .elicitation
                .as_ref()
                .and_then(|c| c.form)
                .unwrap_or(true)
            {
                // Form mode enabled (default for backwards compat)
            } else {
                return Err(anyhow::anyhow!(
                    "Client does not support form mode elicitation"
                ));
            }
        }
        ElicitationMode::Url => {
            if !client_caps
                .elicitation
                .as_ref()
                .and_then(|c| c.url)
                .unwrap_or(false)
            {
                return Err(anyhow::anyhow!(
                    "Client does not support url mode elicitation"
                ));
            }
            if elicitation_params.url.is_none() {
                return Err(anyhow::anyhow!("URL is required for url mode elicitation"));
            }
            if elicitation_params.elicitation_id.is_none() {
                return Err(anyhow::anyhow!(
                    "elicitationId is required for url mode elicitation"
                ));
            }
        }
    }

    info!("Sending elicitation/create to client (mode={:?})", mode);

    let request_id = format!(
        "elicitation_{}",
        std::time::UNIX_EPOCH.elapsed()?.as_nanos()
    );

    let mut elicitation_request = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "elicitation/create",
        "params": {
            "mode": match &mode {
                ElicitationMode::Form => "form",
                ElicitationMode::Url => "url",
            },
            "message": elicitation_params.message,
        }
    });

    // Add mode-specific fields
    if let Some(ref schema) = elicitation_params.requested_schema {
        elicitation_request["params"]["requestedSchema"] = schema.clone();
    }
    if let Some(ref url) = elicitation_params.url {
        elicitation_request["params"]["url"] = json!(url);
    }
    if let Some(ref elicitation_id) = elicitation_params.elicitation_id {
        elicitation_request["params"]["elicitationId"] = json!(elicitation_id);
    }

    // Create oneshot channel for awaiting the client's response
    let (tx, rx) = tokio::sync::oneshot::channel();

    // Store in pending_requests for main loop (stdio) or POST handler (HTTP) to resolve
    server.add_pending_request(request_id.clone(), tx).await;

    // Send through the notification broadcast channel.
    // For stdio: the notification sender writes it to stdout.
    // For HTTP: the SSE stream picks it up and sends it to the client.
    if let Some(ref notification_tx) = server.notification_tx {
        let _ = notification_tx.send(serde_json::to_string(&elicitation_request)?);
    }

    // Wait for client response
    let response = tokio::time::timeout(std::time::Duration::from_secs(60), rx)
        .await
        .map_err(|_| anyhow::anyhow!("Elicitation request timed out after 60 seconds"))?
        .map_err(|_| anyhow::anyhow!("Elicitation response channel dropped"))?;

    let result: ElicitationResult = serde_json::from_value(response)
        .context("Failed to parse elicitation response from client")?;

    info!("Elicitation response received: action={:?}", result.action);

    Ok(json!({
        "action": result.action,
        "content": result.content,
    }))
}
