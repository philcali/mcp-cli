//! API key header strategy.
//!
//! Reads the API key from an environment variable and sets API_KEY for the tool.

use crate::protocol::ToolAuthConfig;
use anyhow::{Context, Result};
use std::collections::HashMap;

pub fn resolve(config: &ToolAuthConfig) -> Result<HashMap<String, String>> {
    let key_env = config
        .required_env_vars
        .first()
        .context("No required_env_vars configured for api_key_header strategy")?;

    let value = std::env::var(key_env).with_context(|| {
        format!("Missing required environment variable '{key_env}' for api_key_header auth")
    })?;

    if value.is_empty() {
        return Err(anyhow::anyhow!(
            "Environment variable '{key_env}' is set but empty"
        ));
    }

    Ok(HashMap::from([("API_KEY".to_string(), value)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::AuthStrategy;

    fn make_config(env_vars: Vec<&str>) -> ToolAuthConfig {
        ToolAuthConfig {
            strategy: AuthStrategy::ApiKeyHeader,
            required_env_vars: env_vars.into_iter().map(String::from).collect(),
            oauth_config: None,
        }
    }

    #[test]
    fn test_resolve_success() {
        unsafe {
            std::env::set_var("MY_API_KEY", "key123");
        }
        let config = make_config(vec!["MY_API_KEY"]);
        let result = resolve(&config).unwrap();
        assert_eq!(result.get("API_KEY"), Some(&"key123".to_string()));
        unsafe {
            std::env::remove_var("MY_API_KEY");
        }
    }

    #[test]
    fn test_resolve_missing_env_var() {
        let config = make_config(vec!["NONEXISTENT_KEY_XYZ"]);
        let result = resolve(&config);
        assert!(result.is_err());
    }
}
