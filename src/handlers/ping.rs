//! Ping / health check handler.

use serde_json::json;

pub fn handle_ping(server: &crate::server::McpServer) -> anyhow::Result<serde_json::Value> {
    let state = &server.state;
    Ok(json!({
        "initialized": state.is_initialized(),
        "server_info": {
            "name": state.name.clone(),
            "version": state.version.clone(),
        },
        "capabilities": {
            "tools": server.capabilities.tools,
            "resources": server.capabilities.resources,
            "prompts": server.capabilities.prompts,
            "logging": server.capabilities.logging,
            "roots": server.capabilities.roots,
            "sampling": server.capabilities.sampling,
            "tasks": server.capabilities.tasks,
            "elicitation": server.capabilities.elicitation,
        },
    }))
}
