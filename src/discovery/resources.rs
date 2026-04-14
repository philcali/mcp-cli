//! Resource discovery logic.

use anyhow::{Context, Result};
use serde_json::json;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ResourceEntry {
    pub uri: String,
    pub resource_type: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub file_path: PathBuf,
}

/// Discover resources from a directory.
pub fn discover_resources(resources_dir: &Path) -> Result<Vec<ResourceEntry>> {
    if !resources_dir.exists() {
        warn!("Resources directory does not exist: {:?}", resources_dir);
        return Ok(Vec::new());
    }

    debug!("Loading resources from: {:?}", resources_dir);
    let mut resources = Vec::new();

    for entry in std::fs::read_dir(resources_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let (name, mime_type) = match (path.file_stem(), path.extension()) {
            (Some(stem), Some(ext)) => (
                stem.to_string_lossy().to_string(),
                Some(mime_from_extension(ext.to_str().unwrap_or(""))),
            ),
            (Some(stem), None) => (stem.to_string_lossy().to_string(), None),
            _ => continue,
        };

        let uri = format!("file://{}", path.display());
        debug!("Found resource: {} -> {}", name, uri);

        resources.push(ResourceEntry {
            uri: uri.clone(),
            resource_type: "text".to_string(),
            name: name.clone(),
            description: Some(format!("Resource file: {}", path.display())),
            mime_type,
            file_path: path,
        });
    }

    info!("Discovered {} resources", resources.len());
    Ok(resources)
}

/// Read resource content.
pub fn read_resource(file_path: &PathBuf) -> Result<String> {
    std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read resource: {:?}", file_path))
}

/// List resources as MCP protocol format.
pub fn list_resources(resources: &[ResourceEntry]) -> serde_json::Value {
    let resource_list: Vec<_> = resources
        .iter()
        .map(|r| {
            json!({
                "uri": r.uri,
                "name": r.name,
                "description": r.description,
                "mimeType": r.mime_type,
            })
        })
        .collect();

    json!({ "resources": resource_list })
}

fn mime_from_extension(ext: &str) -> String {
    match ext {
        "txt" | "text" => "text/plain".to_string(),
        "md" => "text/markdown".to_string(),
        "json" => "application/json".to_string(),
        "xml" => "application/xml".to_string(),
        "yaml" | "yml" => "application/yaml".to_string(),
        "toml" => "application/toml".to_string(),
        "rs" => "text/x-rust".to_string(),
        "sh" => "application/x-sh".to_string(),
        "py" => "text/x-python".to_string(),
        "js" => "application/javascript".to_string(),
        "html" | "htm" => "text/html".to_string(),
        "css" => "text/css".to_string(),
        "csv" => "text/csv".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}
