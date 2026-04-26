//! Resource discovery logic.

use anyhow::{Context, Result};
use serde_json::json;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::protocol::ResourceTemplate;

#[derive(Debug, Clone)]
pub struct ResourceEntry {
    pub uri: String,
    pub resource_type: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub file_path: PathBuf,
}

/// Discovery entry for a resource template file.
#[derive(Debug, Clone)]
pub struct ResourceTemplateEntry {
    pub uri_template: String,
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

/// Discover resource templates from a directory of .template.json files.
pub fn discover_resource_templates(templates_dir: &Path) -> Result<Vec<ResourceTemplateEntry>> {
    if !templates_dir.exists() {
        warn!(
            "Resource templates directory does not exist: {:?}",
            templates_dir
        );
        return Ok(Vec::new());
    }

    debug!("Loading resource templates from: {:?}", templates_dir);
    let mut templates = Vec::new();

    for entry in std::fs::read_dir(templates_dir)? {
        let entry = entry?;
        let path = entry.path();

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if ext != "json" || !stem.ends_with(".template") {
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read template file: {:?}", path))?;

        let template: ResourceTemplate = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse template file: {:?}", path))?;

        debug!(
            "Found resource template: {} -> {}",
            template.name, template.uri_template
        );

        templates.push(ResourceTemplateEntry {
            uri_template: template.uri_template,
            name: template.name,
            description: template.description.clone(),
            mime_type: template.mime_type.clone(),
            file_path: path,
        });
    }

    info!("Discovered {} resource templates", templates.len());
    Ok(templates)
}

/// List resource templates as MCP protocol format.
pub fn list_resource_templates(templates: &[ResourceTemplateEntry]) -> serde_json::Value {
    let template_list: Vec<_> = templates
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

    json!({ "templates": template_list })
}

fn mime_from_extension(ext: &str) -> String {
    match ext {
        "txt" | "text" => "text/plain".to_string(),
        "md" | "markdown" => "text/markdown".to_string(),
        "json" => "application/json".to_string(),
        "jsonc" => "application/jsonc".to_string(),
        "xml" => "application/xml".to_string(),
        "yaml" | "yml" => "application/yaml".to_string(),
        "toml" => "application/toml".to_string(),
        "ini" => "text/plain".to_string(),
        "conf" => "text/plain".to_string(),
        "cfg" => "text/plain".to_string(),
        "rs" => "text/x-rust".to_string(),
        "sh" => "application/x-sh".to_string(),
        "bash" => "application/x-sh".to_string(),
        "py" => "text/x-python".to_string(),
        "pyi" => "text/x-python".to_string(),
        "js" => "application/javascript".to_string(),
        "mjs" => "application/javascript".to_string(),
        "cjs" => "application/javascript".to_string(),
        "ts" => "application/typescript".to_string(),
        "tsx" => "text/x.tsx".to_string(),
        "java" => "text/x-java".to_string(),
        "c" => "text/x-c".to_string(),
        "h" => "text/x-c".to_string(),
        "cpp" | "cc" | "cxx" => "text/x-c++".to_string(),
        "hpp" | "hh" | "hxx" => "text/x-c++".to_string(),
        "go" => "text/x-go".to_string(),
        "rb" => "text/x-ruby".to_string(),
        "php" => "text/x-php".to_string(),
        "swift" => "text/x-swift".to_string(),
        "html" | "htm" => "text/html".to_string(),
        "css" => "text/css".to_string(),
        "scss" => "text/x-scss".to_string(),
        "sass" => "text/x-sass".to_string(),
        "csv" => "text/csv".to_string(),
        "tsv" => "text/tab-separated-values".to_string(),
        "pdf" => "application/pdf".to_string(),
        "png" => "image/png".to_string(),
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "gif" => "image/gif".to_string(),
        "webp" => "image/webp".to_string(),
        "svg" => "image/svg+xml".to_string(),
        "ico" => "image/x-icon".to_string(),
        "woff" => "font/woff".to_string(),
        "woff2" => "font/woff2".to_string(),
        "ttf" => "font/ttf".to_string(),
        "otf" => "font/otf".to_string(),
        "eot" => "application/vnd.ms-fontobject".to_string(),
        "zip" => "application/zip".to_string(),
        "tar" => "application/x-tar".to_string(),
        "gz" => "application/gzip".to_string(),
        "bz2" => "application/x-bzip2".to_string(),
        "xz" => "application/x-xz".to_string(),
        "7z" => "application/x-7z-compressed".to_string(),
        "rar" => "application/vnd.rar".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}
