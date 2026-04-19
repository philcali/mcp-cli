//! Request routing and method dispatch.

use serde_json::json;

/// Route request to appropriate handler based on method name.
pub async fn route_request(
    method: &str,
    params: &serde_json::Value,
    initialized: bool,
    server: &crate::server::McpServer,
) -> anyhow::Result<serde_json::Value> {
    match method {
        "initialize" => crate::handlers::handle_initialize(server, params).await,
        "resources/list" => crate::handlers::handle_resources_list(server).await,
        "resources/subscribe" => crate::handlers::handle_resources_subscribe(server, params).await,
        "resources/unsubscribe" => {
            crate::handlers::handle_resources_unsubscribe(server, params).await
        }
        // After initialize succeeds, server will be marked initialized
        // We only reject requests if explicitly not initialized (initialize not yet called)
        _ if !initialized && method != "ping" => Err(anyhow::anyhow!("Server not initialized")),
        "initialized" => Ok(json!({})),
        "logging/messages" => crate::handlers::handle_logging_messages(server, params).await,
        "ping" => crate::handlers::handle_ping(server),
        "roots/list" => server.handle_roots_list().await,
        "tools/list" => crate::handlers::handle_tools_list(server).await,
        "tools/call" => crate::handlers::handle_tools_call(server, params).await,
        "resources/read" => server.handle_resources_read(params).await,
        "prompts/list" => crate::handlers::handle_prompts_list(server).await,
        "prompts/get" => crate::handlers::handle_prompts_get(server, params).await,
        "telemetry/event" => crate::handlers::handle_telemetry_event(server, params).await,
        "notifications/initialized" => Ok(json!({})),
        _ => Err(anyhow::anyhow!("Unknown method: {}", method)),
    }
}

/// Map of known MCP methods with optional documentation.
pub const KNOWN_METHODS: &[(&str, &str)] = &[
    (
        "initialize",
        "Initialize connection and negotiate capabilities",
    ),
    ("initialized", "Notification that client is initialized"),
    ("logging/messages", "Send log message to server"),
    (
        "ping",
        "Health check - returns server status and capabilities",
    ),
    ("roots/list", "List root directories provided by client"),
    ("tools/list", "List available tools"),
    ("tools/call", "Execute a tool"),
    ("resources/list", "List available resources"),
    ("resources/read", "Read resource contents"),
    ("resources/subscribe", "Subscribe to resource changes"),
    ("resources/unsubscribe", "Unsubscribe from resource changes"),
    ("prompts/list", "List available prompts"),
    ("prompts/get", "Get prompt with arguments"),
    ("telemetry/event", "Send telemetry event to server"),
    (
        "notifications/initialized",
        "Notification: client initialized",
    ),
];

/// Get list of all known methods (for introspection/debugging).
pub fn list_known_methods() -> Vec<&'static str> {
    KNOWN_METHODS.iter().map(|(m, _)| *m).collect()
}
