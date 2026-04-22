//! Bearer token strategy.
//!
//! Reads a bearer token from an environment variable and sets BEARER_TOKEN for the tool.

use crate::protocol::ToolAuthConfig;
use anyhow::{Context, Result};
use std::collections::HashMap;

pub fn resolve(config: &ToolAuthConfig) -> Result<HashMap<String, String>> {
    let key_env = config
        .required_env_vars
        .first()
        .context("No required_env_vars configured for bearer_token strategy")?;

    let value = std::env::var(key_env).with_context(|| {
        format!("Missing required environment variable '{key_env}' for bearer_token auth")
    })?;

    if value.is_empty() {
        return Err(anyhow::anyhow!(
            "Environment variable '{key_env}' is set but empty"
        ));
    }

    Ok(HashMap::from([("BEARER_TOKEN".to_string(), value)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::AuthStrategy;

    fn make_config(env_vars: Vec<&str>) -> ToolAuthConfig {
        ToolAuthConfig {
            strategy: AuthStrategy::BearerToken,
            required_env_vars: env_vars.into_iter().map(String::from).collect(),
            oauth_config: None,
        }
    }

    #[test]
    fn test_resolve_success() {
        unsafe {
            std::env::set_var("MY_TOKEN", "bearer_val");
        }
        let config = make_config(vec!["MY_TOKEN"]);
        let result = resolve(&config).unwrap();
        assert_eq!(result.get("BEARER_TOKEN"), Some(&"bearer_val".to_string()));
        unsafe {
            std::env::remove_var("MY_TOKEN");
        }
    }

    #[test]
    fn test_resolve_missing_env_var() {
        let config = make_config(vec!["NONEXISTENT_TOKEN_XYZ"]);
        let result = resolve(&config);
        assert!(result.is_err());
    }
}
