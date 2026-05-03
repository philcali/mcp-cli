//! Initialization and connection management handlers.

use crate::protocol::{
    Implementation, InitError, InitParams, InitResult, VersionNegotiationResult,
    negotiate_protocol_version,
};
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

    // Negotiate protocol version
    let negotiated_version = match negotiate_protocol_version(&init_params.protocol_version) {
        VersionNegotiationResult::Supported(version)
        | VersionNegotiationResult::Compatible(version) => {
            debug!("Negotiated protocol version: {}", version);
            version
        }
        VersionNegotiationResult::Unsupported {
            received,
            supported,
        } => {
            let error = InitError::UnsupportedProtocol {
                received: received.clone(),
                supported: supported.clone(),
            };
            return Err(anyhow::anyhow!("{}", error));
        }
    };

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

    // Store client capabilities for later use (e.g., sampling)
    server
        .state
        .set_client_capabilities(init_params.capabilities);

    // Set initialized flag on successful initialization (via Arc)
    server
        .state
        .initialized
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let result = InitResult {
        protocol_version: negotiated_version,
        capabilities: server.capabilities.clone(),
        server_info: Implementation {
            name: server.state.name.clone(),
            version: server.state.version.clone(),
        },
    };

    Ok(serde_json::to_value(result)?)
}
