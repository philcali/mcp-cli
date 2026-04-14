//! Initialization and connection management handlers.

use crate::protocol::*;
use anyhow::{Context, Result};
use tracing::{debug, info};

/// Initialize connection and negotiate capabilities.
pub async fn handle_initialize(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let init_params: InitParams =
        serde_json::from_value(params.clone()).context("Failed to parse initialize parameters")?;

    info!(
        "Received initialize request from {}: {}",
        init_params.client_info.name, init_params.protocol_version
    );

    if let Some(roots_cap) = &init_params.capabilities.roots {
        debug!(
            "Client supports roots listing: {:?}",
            roots_cap.list_changed
        );
    }

    if let Some(ref roots) = init_params.roots {
        info!("Received {} root directory(ies) from client", roots.len());
        for root in roots {
            server.add_root(root.uri.clone(), root.name.clone());
        }
    }

    if !init_params.protocol_version.starts_with("2024-") {
        return Err(anyhow::anyhow!("Unsupported protocol version"));
    }

    let result = InitResult {
        protocol_version: "2024-11-05".to_string(),
        capabilities: server.capabilities.clone(),
        server_info: Implementation {
            name: server.state.name.clone(),
            version: server.state.version.clone(),
        },
    };

    Ok(serde_json::to_value(result)?)
}
