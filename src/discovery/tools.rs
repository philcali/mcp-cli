//! Tool discovery logic.

use crate::protocol::{TaskSupportLevel, ToolAuthConfig, load_tool_auth_config};
use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};

/// Default input schema used when a tool doesn't support --describe.
fn default_input_schema() -> serde_json::Value {
    json!({ "type": "object" })
}

#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub script_path: PathBuf,
    pub auth_config: Option<ToolAuthConfig>,
    pub input_schema: serde_json::Value,
    pub task_support: Option<TaskSupportLevel>,
}

/// Probe a tool with --describe and parse the JSON response.
/// Returns (name, description, input_schema, task_support) or falls back to defaults.
fn describe_tool(path: &Path) -> (String, String, serde_json::Value, Option<TaskSupportLevel>) {
    let output = match Command::new(path).arg("--describe").output() {
        Ok(o) => o,
        Err(e) => {
            debug!("Failed to spawn {:?} --describe: {}", path, e);
            return tool_defaults(path);
        }
    };

    if !output.status.success() {
        debug!(
            "{:?} --describe exited with {}",
            path,
            output
                .status
                .code()
                .map_or("unknown".to_string(), |c| c.to_string())
        );
        return tool_defaults(path);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => {
            debug!("Failed to parse {:?} --describe output: {}", path, e);
            return tool_defaults(path);
        }
    };

    let name = parsed
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        });

    let description = parsed
        .get("description")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Tool script: {}", path.display()));

    let input_schema = parsed
        .get("inputSchema")
        .and_then(|v| if v.is_object() { Some(v.clone()) } else { None })
        .unwrap_or(default_input_schema());

    let task_support = parsed
        .get("execution")
        .and_then(|e| e.get("taskSupport"))
        .and_then(|v| v.as_str())
        .and_then(|s| match s {
            "forbidden" => Some(TaskSupportLevel::Forbidden),
            "optional" => Some(TaskSupportLevel::Optional),
            "required" => Some(TaskSupportLevel::Required),
            _ => None,
        });

    (name, description, input_schema, task_support)
}

/// Return fallback (name, description, input_schema, task_support) derived from the path.
fn tool_defaults(path: &Path) -> (String, String, serde_json::Value, Option<TaskSupportLevel>) {
    (
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
        format!("Tool script: {}", path.display()),
        default_input_schema(),
        None,
    )
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

        // Skip auth config files (e.g., my-tool.auth.json)
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".auth.json"))
        {
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

        let (name, description, input_schema, task_support) = describe_tool(&path);

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
                description,
                script_path: path.clone(),
                auth_config,
                input_schema,
                task_support,
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
                "inputSchema": t.input_schema,
            })
        })
        .collect();

    json!({ "tools": tool_list })
}
