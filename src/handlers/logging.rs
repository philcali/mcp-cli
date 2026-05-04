//! Logging message handler for MCP server.

use anyhow::Result;
use serde_json::json;

use crate::protocol::LogLevel;
use crate::server::McpServer;

/// Compare two log levels. Returns true if `msg_level` is >= `min_level` (should be logged).
fn should_log(msg_level: &LogLevel, min_level: &LogLevel) -> bool {
    level_to_priority(msg_level) >= level_to_priority(min_level)
}

/// Map log levels to numeric priority (higher = more severe).
fn level_to_priority(level: &LogLevel) -> u8 {
    match level {
        LogLevel::Debug => 0,
        LogLevel::Info => 1,
        LogLevel::Notice => 2,
        LogLevel::Warning => 3,
        LogLevel::Error => 4,
        LogLevel::Critical => 5,
        LogLevel::Alert => 6,
        LogLevel::Emergency => 7,
    }
}

/// Handle logging/setLevel notification from client.
///
/// The client uses this to configure the minimum log level the server should forward.
pub async fn handle_logging_set_level(
    server: &McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let level = params
        .get("level")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required field: level"))?;

    let log_level: LogLevel = serde_json::from_value(json!(level))
        .map_err(|e| anyhow::anyhow!("Invalid log level: {}", e))?;

    let mut current_level = server.state.log_level.write().unwrap();
    *current_level = log_level;

    Ok(json!({}))
}

/// Handle logging/messages request from client.
///
/// This allows clients to send log messages to the server for unified logging.
/// Messages below the configured minimum log level are dropped.
pub async fn handle_logging_messages(
    server: &McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let level_str = params
        .get("level")
        .and_then(|v| v.as_str())
        .unwrap_or("info");

    let level: LogLevel = serde_json::from_value(json!(level_str)).unwrap_or(LogLevel::Info);

    // Drop messages below the configured minimum log level
    let min_level = *server.state.log_level.read().unwrap();
    if !should_log(&level, &min_level) {
        return Ok(json!({}));
    }

    let logger = params.get("logger").and_then(|v| v.as_str());
    let message = params.get("message").and_then(|v| v.as_str()).unwrap_or("");

    // Log the message using the appropriate tracing level
    match &level {
        LogLevel::Debug => {
            if let Some(logger) = logger {
                tracing::debug!(logger, "{}", message);
            } else {
                tracing::debug!("{}", message);
            }
        }
        LogLevel::Info => {
            if let Some(logger) = logger {
                tracing::info!(logger, "{}", message);
            } else {
                tracing::info!("{}", message);
            }
        }
        LogLevel::Notice | LogLevel::Warning => {
            if let Some(logger) = logger {
                tracing::warn!(logger, "{}", message);
            } else {
                tracing::warn!("{}", message);
            }
        }
        LogLevel::Error => {
            if let Some(logger) = logger {
                tracing::error!(logger, "{}", message);
            } else {
                tracing::error!("{}", message);
            }
        }
        LogLevel::Critical | LogLevel::Alert | LogLevel::Emergency => {
            if let Some(logger) = logger {
                tracing::error!(logger, "{}", message);
            } else {
                tracing::error!("{}", message);
            }
        }
    }

    Ok(json!({}))
}
