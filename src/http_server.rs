//! HTTP transport for MCP using Axum.
//!
//! Implements the MCP Streamable HTTP transport:
//! - POST /mcp for JSON-RPC requests/responses
//! - GET /mcp for SSE notification stream

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use async_stream::stream;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use axum::routing::post;
use axum::{Json, Router};
use futures::Stream;
use serde_json::json;
use tracing::{debug, error, info};

use crate::protocol::{JsonRpcError, JsonRpcNotification, JsonRpcRequest};
use crate::server::McpServer;

#[derive(Clone)]
struct AppState {
    server: Arc<McpServer>,
}

/// Start the HTTP server on the given address.
pub async fn run_http(server: &McpServer, addr: SocketAddr) -> Result<()> {
    let server = server.clone();
    server.setup_task_status_notifications();
    let state = AppState {
        server: Arc::new(server),
    };

    let app = Router::new()
        .route("/mcp", post(json_rpc_handler).get(sse_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;

    info!("MCP HTTP server listening on {}", actual_addr);

    let server_handle = axum::serve(listener, app);

    if let Err(e) = server_handle
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        error!("Server error: {}", e);
    }

    Ok(())
}

/// Shutdown signal listener — waits for SIGINT/SIGTERM (unix) or Ctrl+C.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to create SIGTERM signal handler");
        let mut int = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .expect("Failed to create SIGINT signal handler");
        tokio::select! {
            _ = term.recv() => {},
            _ = int.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .expect("Failed to create Ctrl+C handler")
            .await
            .ok();
    }
    info!("Shutdown signal received");
}

/// Handle POST /mcp — JSON-RPC requests (single or batch).
async fn json_rpc_handler(
    State(state): State<AppState>,
    Json(raw): Json<serde_json::Value>,
) -> impl IntoResponse {
    if raw.is_array() {
        handle_batch(&state.server, &raw).await
    } else {
        handle_single(&state.server, &raw).await
    }
}

/// Extract the JSON-RPC id from a message as a string.
fn extract_id(raw: &serde_json::Value) -> Option<String> {
    raw.get("id").map(|v| {
        if let Some(s) = v.as_str() {
            s.to_string()
        } else if let Some(n) = v.as_i64() {
            n.to_string()
        } else if let Some(u) = v.as_u64() {
            u.to_string()
        } else {
            serde_json::to_string(v).unwrap_or_default()
        }
    })
}

/// Extract the result or error content from a response message.
fn extract_result_or_error(raw: &serde_json::Value) -> Option<serde_json::Value> {
    raw.get("result")
        .cloned()
        .or_else(|| raw.get("error").cloned())
}

/// Route a single JSON-RPC message through the MCP server.
async fn route_single_message(
    server: &McpServer,
    raw: &serde_json::Value,
    initialized: bool,
) -> Option<serde_json::Value> {
    // Notification (no id) — handle but return None (no response)
    if raw.get("id").is_none()
        && let Ok(notification) = serde_json::from_value::<JsonRpcNotification>(raw.clone())
    {
        debug!("Processing notification: {}", notification.method);
        let _ = crate::routing::route_request(
            &notification.method,
            &notification.params,
            initialized,
            server,
        )
        .await;
        return None;
    }

    // Request with id
    match serde_json::from_value::<JsonRpcRequest>(raw.clone()) {
        Ok(request) => {
            debug!(
                "Processing {} with id={:?}",
                request.method, request.id_value
            );
            let result = crate::routing::route_request(
                &request.method,
                &request.params,
                initialized,
                server,
            )
            .await;

            Some(match result {
                Ok(resp) => json!({ "jsonrpc": "2.0", "result": resp, "id": request.id_value }),
                Err(e) => {
                    let err = JsonRpcError::internal_error(&e.to_string());
                    json!({ "jsonrpc": "2.0", "error": err, "id": request.id_value })
                }
            })
        }
        Err(e) => {
            error!("Failed to parse request: {}", e);
            Some(json!({
                "jsonrpc": "2.0",
                "error": JsonRpcError::internal_error(&e.to_string()),
                "id": null
            }))
        }
    }
}

/// Handle a single JSON-RPC message.
async fn handle_single(server: &McpServer, raw: &serde_json::Value) -> axum::response::Response {
    // First check if this is a response to a pending request (e.g., sampling)
    if let Some(id) = extract_id(raw)
        && let Some(response) = extract_result_or_error(raw)
        && server.resolve_pending_request(&id, response).await
    {
        return (StatusCode::OK, Json(json!({"status": "ok"}))).into_response();
    }

    let initialized = server.state.is_initialized();

    match route_single_message(server, raw, initialized).await {
        Some(response) => (StatusCode::OK, Json(response)).into_response(),
        None => StatusCode::OK.into_response(),
    }
}

/// Handle a batch of JSON-RPC messages.
async fn handle_batch(server: &McpServer, raw: &serde_json::Value) -> axum::response::Response {
    let messages = match raw.as_array() {
        Some(arr) => arr,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": JsonRpcError::internal_error("Expected array for batch request"),
                    "id": null
                })),
            )
                .into_response();
        }
    };

    let initialized = server.state.is_initialized();
    let mut responses = Vec::new();

    for msg in messages {
        // Check if this is a pending response
        if let Some(id) = extract_id(msg)
            && let Some(resp) = extract_result_or_error(msg)
            && server.resolve_pending_request(&id, resp).await
        {
            responses.push(json!({
                "jsonrpc": "2.0",
                "result": {"status": "ok"},
                "id": id
            }));
            continue;
        }

        if let Some(response) = route_single_message(server, msg, initialized).await {
            responses.push(response);
        }
    }

    if responses.is_empty() {
        StatusCode::OK.into_response()
    } else {
        (StatusCode::OK, Json(responses)).into_response()
    }
}

/// Handle GET /mcp — SSE notification stream.
async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let notification_tx = state
        .server
        .notification_tx
        .as_ref()
        .expect("notification channel should be set");

    let mut rx = notification_tx.subscribe();

    let stream = stream! {
        while let Ok(msg) = rx.recv().await {
            yield Ok(Event::default().data(&msg));
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new().interval(std::time::Duration::from_secs(15)),
    )
}
