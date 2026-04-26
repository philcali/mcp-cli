//! Resource listing, reading, and subscription handlers.

use crate::protocol::*;
use anyhow::{Context, Result};
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tracing::{debug, info};

/// List available resources.
pub async fn handle_resources_list(server: &crate::server::McpServer) -> Result<serde_json::Value> {
    let mut cached = server.state.cached_resources.lock().unwrap();

    // Load resources from directory if not already cached and directory is configured
    if cached.is_empty() && server.state.resources_dir.is_some() {
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

/// Read resource contents by URI with streaming.
pub async fn handle_resources_read_streaming(
    server: &crate::server::McpServer,
    read_params: &ReadResourceParams,
) -> Result<serde_json::Value> {
    info!("Reading resource with streaming: {}", read_params.uri);

    // Load resources to find the entry
    let resources = server.state.load_resources()?;

    let entry = resources.iter().find(|r| r.uri == read_params.uri).cloned();

    if entry.is_none() {
        return Err(anyhow::anyhow!(
            "Resource '{}' is not available",
            read_params.uri
        ));
    }

    let entry = entry.unwrap();
    info!("Found resource: {:?}", entry.file_path);

    // Generate a stream ID
    let stream_id = format!("stream_{}", std::time::UNIX_EPOCH.elapsed()?.as_nanos());

    // Clone the notification channel for use
    let notification_tx = server.notification_tx.clone();

    // Get file size for metadata
    let file_size = tokio::fs::metadata(&entry.file_path)
        .await
        .map(|m| m.len() as usize)
        .ok();

    // Send meta notification using helper method
    let meta_params = json!({
        "request_id": stream_id,
        "chunk": {"type": "meta", "chunk_count": -1, "total_bytes": file_size}
    });
    server
        .send_notification("resources/stream", meta_params)
        .await;

    // Open and read file line by line
    let file = tokio::fs::File::open(&entry.file_path).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(tx) = notification_tx.as_ref() {
            let _ = tx.send(
                json!({
                    "jsonrpc": "2.0",
                    "method": "resources/stream",
                    "params": {
                        "request_id": stream_id,
                        "chunk": {"type": "content", "data": line, "is_error": None::<bool>}
                    }
                })
                .to_string(),
            );
        }
    }

    // Send done notification using helper method
    let done_params = json!({
        "request_id": stream_id,
        "chunk": {"type": "done", "summary": None::<String>}
    });
    server
        .send_notification("resources/stream", done_params)
        .await;

    Ok(json!({
        "stream_id": stream_id
    }))
}

/// Read resource contents by URI.
pub async fn handle_resources_read(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    // Parse parameters to check for streaming flag
    let uri_value = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'uri' parameter"))?;

    let stream_flag = params
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    if stream_flag {
        let read_params = ReadResourceParams {
            uri: uri_value.to_string(),
            stream: true,
        };
        return handle_resources_read_streaming(server, &read_params).await;
    }

    info!("Reading resource: {}", uri_value);

    // Load resources - they are cached in state, but we just read from directory
    let resources = server.state.load_resources()?;

    let entry = resources.iter().find(|r| r.uri == uri_value).cloned();

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
    let resources = server.state.load_resources()?;

    if !resources.iter().any(|r| r.uri == subscribe_params.uri) {
        return Err(anyhow::anyhow!(
            "Resource '{}' does not exist",
            subscribe_params.uri
        ));
    }

    // Subscribe to the resource
    let was_new = server
        .state
        .subscription_manager
        .subscribe(&subscribe_params.uri);

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
    let resources = server.state.load_resources()?;

    if !resources.iter().any(|r| r.uri == unsubscribe_params.uri) {
        return Err(anyhow::anyhow!(
            "Resource '{}' does not exist",
            unsubscribe_params.uri
        ));
    }

    // Unsubscribe from the resource
    let was_subscribed = server
        .state
        .subscription_manager
        .unsubscribe(&unsubscribe_params.uri);

    if !was_subscribed {
        debug!("Not subscribed to: {}", unsubscribe_params.uri);
    } else {
        info!("Successfully unsubscribed from: {}", unsubscribe_params.uri);
    }

    Ok(json!({}))
}

/// List available resource templates.
pub async fn handle_resource_templates_list(
    server: &crate::server::McpServer,
) -> Result<serde_json::Value> {
    let mut cached = server.state.cached_resource_templates.lock().unwrap();

    // Load templates from directory if not already cached and directory is configured
    if cached.is_empty() && server.state.resource_templates_dir.is_some() {
        *cached = server.load_resource_templates()?;
    }

    let template_list: Vec<_> = cached
        .iter()
        .map(|t| {
            json!({
                "uriTemplate": t.uri_template,
                "name": t.name,
                "description": t.description,
                "mimeType": t.mime_type,
            })
        })
        .collect();

    Ok(json!({ "templates": template_list }))
}
