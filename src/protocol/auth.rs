//! Authentication configuration helpers.
//!
//! These are business-logic functions for loading and parsing auth configs.

use std::path::Path;

use anyhow::Result;

use super::types::ToolAuthConfig;

/// Authentication strategy for a tool.
pub use super::types::AuthStrategy;

/// OAuth2 configuration for a tool.
pub use super::types::OAuthConfig;

/// Load tool auth config from a file path.
pub fn load_tool_auth_config(path: &Path) -> Result<Option<ToolAuthConfig>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(path)?;
    let config: ToolAuthConfig = serde_json::from_str(&content)?;
    Ok(Some(config))
}

/// Load tool auth config from JSON string.
pub fn parse_tool_auth_config(json: &str) -> Result<ToolAuthConfig> {
    let config: ToolAuthConfig = serde_json::from_str(json)?;
    Ok(config)
}
