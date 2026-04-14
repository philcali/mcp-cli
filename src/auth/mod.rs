//! Authentication configuration and credential resolution.

use anyhow::{Context, Result};
use std::collections::HashMap;
use tracing::debug;

pub use crate::protocol::ToolAuthConfig;

/// Resolve credentials for a tool.
pub fn resolve_credentials(
    tools_dir: &std::path::Path,
    tool_name: &str,
) -> Result<HashMap<String, String>> {
    let auth_config = load_auth_config(tools_dir, tool_name)?;

    match auth_config {
        Some(config) => validate_and_inject(&config),
        None => Ok(HashMap::new()),
    }
}

fn load_auth_config(
    tools_dir: &std::path::Path,
    tool_name: &str,
) -> Result<Option<ToolAuthConfig>> {
    let auth_path = tools_dir.join(tool_name).join(".auth.json");

    if auth_path.exists() {
        return load_tool_auth_config(&auth_path);
    }

    let flat_auth_path = tools_dir.join(format!("{}.auth.json", tool_name));
    if flat_auth_path.exists() {
        return load_tool_auth_config(&flat_auth_path);
    }

    Ok(None)
}

fn validate_and_inject(config: &ToolAuthConfig) -> Result<HashMap<String, String>> {
    let mut creds = HashMap::new();

    for env_var in &config.required_env_vars {
        match std::env::var(env_var) {
            Ok(value) => {
                if value.is_empty() {
                    return Err(anyhow::anyhow!(
                        "Environment variable '{}' is set but empty.",
                        env_var
                    ));
                }
                creds.insert(env_var.clone(), value);
            }
            Err(_) => {
                let all_env_vars: Vec<String> = config.required_env_vars.clone();
                return Err(anyhow::anyhow!(
                    "Missing required environment variable '{}' for tool '{:?}'.\nAvailable: {}\nPlease set {}.",
                    env_var,
                    config.strategy,
                    all_env_vars.join(", "),
                    env_var
                ));
            }
        }
    }

    debug!(
        "Resolved {} credential(s) for auth strategy '{:?}'",
        creds.len(),
        config.strategy
    );
    Ok(creds)
}

pub fn load_tool_auth_config(path: &std::path::Path) -> Result<Option<ToolAuthConfig>> {
    let content = std::fs::read_to_string(path)?;
    let config: ToolAuthConfig = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse auth config from {:?}", path))?;

    Ok(Some(config))
}
