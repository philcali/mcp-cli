//! Completion handler for argument autocompletion.

use crate::protocol::*;
use anyhow::{Context, Result};
use serde_json::json;
use tracing::debug;

/// Handle completion/complete requests.
///
/// Provides autocompletion suggestions for tool argument values.
/// Currently supports completions for tool names when completing a `ref` of type `tool`.
pub async fn handle_completion_complete(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let params: CompleteParams =
        serde_json::from_value(params.clone()).context("Failed to parse completion parameters")?;

    match params.ref_.ref_type.as_str() {
        "tool" => handle_tool_completion(server, &params).await,
        _ => Ok(json!({
            "values": Vec::<String>::new()
        })),
    }
}

/// Provide completions for tool names.
async fn handle_tool_completion(
    server: &crate::server::McpServer,
    params: &CompleteParams,
) -> Result<serde_json::Value> {
    // Only support completing the tool name (empty or partial value)
    if params.argument.name != "name" {
        return Ok(json!({
            "values": Vec::<String>::new()
        }));
    }

    let mut cached = server.state.cached_tools.lock().unwrap();

    if cached.is_empty() && server.state.tools_dir.is_some() {
        *cached = server.load_tools()?;
    }

    let prefix = params.argument.value.to_lowercase();
    let values: Vec<String> = cached
        .keys()
        .filter(|name| name.to_lowercase().contains(&prefix))
        .cloned()
        .collect();

    debug!(
        "Completion for tool 'name': {} matches for '{}'",
        values.len(),
        params.argument.value
    );

    Ok(json!({
        "values": values,
        "total": values.len(),
        "has_more": false
    }))
}
