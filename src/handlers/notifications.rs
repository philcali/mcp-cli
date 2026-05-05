//! Handlers for progress and cancellation notifications.

use anyhow::Result;
use serde_json::json;
use tracing::info;

use crate::protocol::{CancelParams, ProgressParams};

/// Handle notifications/progress sent by the server to the client.
///
/// This handler exists so the method is recognized in the routing table.
/// Actual progress notifications are emitted via server.send_notification().
pub async fn handle_notifications_progress(
    _server: &crate::server::McpServer,
    _params: &serde_json::Value,
) -> Result<serde_json::Value> {
    Ok(json!({}))
}

/// Handle notifications/cancelled from the client.
///
/// The client sends this to cancel an in-flight request.
/// The request ID is stored so handlers can check for cancellation.
pub async fn handle_notifications_cancelled(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let cancel_params: CancelParams =
        serde_json::from_value(params.clone()).unwrap_or(CancelParams {
            request_id: json!(null),
            reason: None,
        });

    let reason = cancel_params
        .reason
        .clone()
        .unwrap_or_else(|| "Cancelled by client".to_string());

    // Store the cancelled request ID so handlers can check
    let mut cancelled = server.state.cancelled_requests.lock().unwrap();
    cancelled.insert(cancel_params.request_id.clone(), reason.clone());

    info!(
        "notifications/cancelled: request_id={:?} reason={}",
        cancel_params.request_id, reason
    );

    Ok(json!({}))
}

/// Send a progress notification via the server's broadcast channel.
pub async fn send_progress(
    server: &crate::server::McpServer,
    progress_token: &serde_json::Value,
    progress: f64,
    total: Option<f64>,
    message: Option<&str>,
) {
    let params = json!(ProgressParams {
        progress_token: progress_token.clone(),
        progress,
        total,
        message: message.map(String::from),
    });
    server
        .send_notification("notifications/progress", params)
        .await;
}

/// Check if a request has been cancelled.
/// Returns the cancellation reason if cancelled, None otherwise.
pub fn is_request_cancelled(
    state: &crate::state::ServerState,
    request_id: &serde_json::Value,
) -> Option<String> {
    let cancelled = state.cancelled_requests.lock().unwrap();
    cancelled.get(request_id).cloned()
}
