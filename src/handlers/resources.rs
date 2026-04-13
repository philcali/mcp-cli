//! Resource listing, reading, and subscription handlers.

use crate::protocol::*;
use anyhow::{Context, Result};
use serde_json::json;
use tracing::{debug, info};

/// List available resources.
pub async fn handle_resources_list(server: &crate::server::McpServer) -> Result<serde_json::Value> {
    let mut cached = server.cached_resources.lock().unwrap();

    // Load resources from directory if not already cached and directory is configured
    if cached.is_empty() && server.resources_dir.is_some() {
        *cached = server.load_resources()?;
    }

    let resource_list: Vec<_> = cached
        .iter()
        .map(|r| {
            json!({
                "uri": r.uri,
                "type": r.resource_type,
                "name": r.name,
                "description": r.description,
                "mimeType": r.mime_type,
            })
        })
        .collect();

    Ok(json!({ "resources": resource_list }))
}

/// Read resource contents by URI.
pub async fn handle_resources_read(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    // Extract resource URI from parameters
    let uri_value = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'uri' parameter"))?;

    info!("Reading resource: {}", uri_value);

    // First check cache, reload if empty
    let mut cached = server.cached_resources.lock().unwrap();

    if cached.is_empty() {
        *cached = server.load_resources()?;
        info!("Reloaded {} resources", cached.len());
    }

    let entry = cached.iter().find(|r| r.uri == uri_value).cloned();

    if let Some(entry) = entry {
        info!("Found resource: {:?}", entry.file_path);

        // Read the file contents
        let content = std::fs::read_to_string(&entry.file_path)?;

        Ok(json!({
            "contents": [
                {
                    "uri": entry.uri,
                    "text": content,
                    "mimeType": entry.mime_type,
                }
            ]
        }))
    } else {
        Err(anyhow::anyhow!("Resource '{}' is not available", uri_value))
    }
}

/// Subscribe to resource change notifications.
pub async fn handle_resources_subscribe(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let subscribe_params: SubscribeResourceParams = serde_json::from_value(params.clone())
        .context("Failed to parse resources/subscribe parameters")?;

    info!("Subscribing to resource: {}", subscribe_params.uri);

    // Check if resource exists
    let mut cached = server.cached_resources.lock().unwrap();
    if cached.is_empty() {
        *cached = server.load_resources()?;
    }

    if !cached.iter().any(|r| r.uri == subscribe_params.uri) {
        return Err(anyhow::anyhow!(
            "Resource '{}' does not exist",
            subscribe_params.uri
        ));
    }

    // Subscribe to the resource
    let was_new = server.subscription_manager.subscribe(&subscribe_params.uri);

    if was_new {
        info!("Successfully subscribed to: {}", subscribe_params.uri);
    } else {
        debug!("Already subscribed to: {}", subscribe_params.uri);
    }

    Ok(json!({}))
}

/// Unsubscribe from resource change notifications.
pub async fn handle_resources_unsubscribe(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let unsubscribe_params: UnsubscribeResourceParams = serde_json::from_value(params.clone())
        .context("Failed to parse resources/unsubscribe parameters")?;

    info!("Unsubscribing from resource: {}", unsubscribe_params.uri);

    // Check if resource exists first
    let mut cached = server.cached_resources.lock().unwrap();
    if cached.is_empty() {
        *cached = server.load_resources()?;
    }

    if !cached.iter().any(|r| r.uri == unsubscribe_params.uri) {
        return Err(anyhow::anyhow!(
            "Resource '{}' does not exist",
            unsubscribe_params.uri
        ));
    }

    // Unsubscribe from the resource
    let was_subscribed = server
        .subscription_manager
        .unsubscribe(&unsubscribe_params.uri);

    if !was_subscribed {
        debug!("Not subscribed to: {}", unsubscribe_params.uri);
    } else {
        info!("Successfully unsubscribed from: {}", unsubscribe_params.uri);
    }

    Ok(json!({}))
}
