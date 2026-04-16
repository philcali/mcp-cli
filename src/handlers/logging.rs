//! Logging message handler for MCP server.

use anyhow::Result;
use serde_json::json;

use crate::server::McpServer;

/// Handle logging/messages request from client.
///
/// This allows clients to send log messages to the server for unified logging.
/// The messages are currently logged using the tracing crate.
pub async fn handle_logging_messages(
    _server: &McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    // Parse the log message parameters
    let level = params
        .get("level")
        .and_then(|v| v.as_str())
        .unwrap_or("info");

    let logger = params.get("logger").and_then(|v| v.as_str());

    let message = params.get("message").and_then(|v| v.as_str()).unwrap_or("");

    // Log the message using the appropriate tracing level
    match level {
        "debug" => {
            if let Some(logger) = logger {
                tracing::debug!(logger, "{}", message);
            } else {
                tracing::debug!("{}", message);
            }
        }
        "info" => {
            if let Some(logger) = logger {
                tracing::info!(logger, "{}", message);
            } else {
                tracing::info!("{}", message);
            }
        }
        "notice" | "warning" => {
            if let Some(logger) = logger {
                tracing::warn!(logger, "{}", message);
            } else {
                tracing::warn!("{}", message);
            }
        }
        "error" => {
            if let Some(logger) = logger {
                tracing::error!(logger, "{}", message);
            } else {
                tracing::error!("{}", message);
            }
        }
        "critical" | "alert" | "emergency" => {
            if let Some(logger) = logger {
                tracing::error!(logger, "{}", message);
            } else {
                tracing::error!("{}", message);
            }
        }
        unknown => {
            // Fallback: log as info with unknown level noted
            if let Some(logger) = logger {
                tracing::info!(logger, "[unknown level: {}] {}", unknown, message);
            } else {
                tracing::info!("[unknown level: {}] {}", unknown, message);
            }
        }
    }

    Ok(json!({}))
}
