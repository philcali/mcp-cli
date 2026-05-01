//! Roots list handler.

use serde_json::json;
use tracing::info;

pub async fn handle_roots_list(
    server: &crate::server::McpServer,
) -> anyhow::Result<serde_json::Value> {
    info!("Handling roots list request");
    let roots = server.state.roots.lock().unwrap();
    let roots_list: Vec<_> = roots
        .iter()
        .map(|root| {
            if let Some(ref name) = root.name {
                json!({ "uri": root.uri, "name": name })
            } else {
                json!({ "uri": root.uri })
            }
        })
        .collect();
    Ok(json!({ "roots": roots_list }))
}
