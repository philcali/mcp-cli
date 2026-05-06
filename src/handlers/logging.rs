//! Logging handler for MCP server.

use anyhow::Result;
use serde_json::json;

use crate::protocol::LogLevel;
use crate::server::McpServer;

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
