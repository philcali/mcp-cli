//! Tool discovery logic.

use crate::protocol::{ToolAuthConfig, load_tool_auth_config};
use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub script_path: PathBuf,
    pub auth_config: Option<ToolAuthConfig>,
}

/// Discover tools from a directory.
pub fn discover_tools(tools_dir: &Path) -> Result<HashMap<String, ToolDefinition>> {
    if !tools_dir.exists() {
        warn!("Tools directory does not exist: {:?}", tools_dir);
        return Ok(HashMap::new());
    }

    let mut tools = HashMap::new();

    for entry in std::fs::read_dir(tools_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to read metadata for {:?}: {}", path, e);
                continue;
            }
        };

        #[cfg(unix)]
        {
            use std::os::unix::prelude::*;
            let mode = metadata.permissions().mode();
            if mode & 0o111 == 0 {
                debug!("Skipping non-executable tool: {}", path.display());
                continue;
            }
        }

        let name = match path.file_stem() {
            Some(stem) => stem.to_string_lossy().to_string(),
            None => {
                warn!("Failed to get file stem for {:?}", path);
                continue;
            }
        };

        let auth_config = match load_tool_auth_config(&path.with_extension("")) {
            Ok(Some(cfg)) => Some(cfg),
            Err(e) => {
                warn!("Failed to load auth config for {}: {}", name, e);
                None
            }
            Ok(None) => None,
        };

        tools.insert(
            name.clone(),
            ToolDefinition {
                name: name.clone(),
                description: format!("Tool script: {}", path.display()),
                script_path: path.clone(),
                auth_config,
            },
        );

        debug!("Discovered tool: {} -> {}", name, path.display());
    }

    info!("Discovered {} tools", tools.len());
    Ok(tools)
}

/// List tools as MCP protocol format.
pub fn list_tools(tools: &HashMap<String, ToolDefinition>) -> serde_json::Value {
    let tool_list: Vec<_> = tools
        .values()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
            })
        })
        .collect();

    json!({ "tools": tool_list })
}
