//! Authentication configuration and credential resolution.

use crate::protocol::AuthStrategy;
use crate::protocol::ToolAuthConfig;
use anyhow::{Context, Result};
use std::collections::HashMap;
use tracing::debug;

pub mod api_key;
pub mod bearer;
pub mod oauth2;
pub mod token_cache;

pub use token_cache::TokenCache;

/// Resolve credentials for a tool based on its auth strategy.
pub async fn resolve_credentials(
    tools_dir: &std::path::Path,
    tool_name: &str,
) -> Result<HashMap<String, String>> {
    let auth_config = load_auth_config(tools_dir, tool_name)?;

    match auth_config {
        Some(config) => resolve_with_strategy(&config, tool_name).await,
        None => Ok(HashMap::new()),
    }
}

async fn resolve_with_strategy(
    config: &ToolAuthConfig,
    tool_name: &str,
) -> Result<HashMap<String, String>> {
    let cache = TokenCache::new();

    match &config.strategy {
        AuthStrategy::EnvVar => validate_and_inject(config),
        AuthStrategy::OAuth2 => {
            let oauth_config = config
                .oauth_config
                .as_ref()
                .context("OAuth2 strategy configured but no oauth_config provided")?;
            oauth2::resolve_oauth2(oauth_config, config, tool_name, &cache).await
        }
        AuthStrategy::ApiKeyHeader => api_key::resolve(config),
        AuthStrategy::BearerToken => bearer::resolve(config),
    }
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

fn load_auth_config(
    tools_dir: &std::path::Path,
    tool_name: &str,
) -> Result<Option<ToolAuthConfig>> {
    // Try flat auth file first: {tool_name}.auth.json
    let flat_auth_path = tools_dir.join(format!("{tool_name}.auth.json"));
    if flat_auth_path.exists() {
        return load_tool_auth_config(&flat_auth_path);
    }

    // Try directory-based auth: {tool_name}/.auth.json
    let auth_path = tools_dir.join(tool_name).join(".auth.json");
    if auth_path.is_file() {
        return load_tool_auth_config(&auth_path);
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::AuthStrategy;
    use tempfile::TempDir;

    fn make_auth_config(strategy: AuthStrategy, env_vars: Vec<&str>) -> ToolAuthConfig {
        ToolAuthConfig {
            strategy,
            required_env_vars: env_vars.into_iter().map(String::from).collect(),
            oauth_config: None,
        }
    }

    #[test]
    fn test_env_var_strategy_success() {
        unsafe {
            std::env::set_var("TEST_AUTH_KEY", "secret_value");
        }
        let config = make_auth_config(AuthStrategy::EnvVar, vec!["TEST_AUTH_KEY"]);
        let result = validate_and_inject(&config).unwrap();
        assert_eq!(
            result.get("TEST_AUTH_KEY"),
            Some(&"secret_value".to_string())
        );
        unsafe {
            std::env::remove_var("TEST_AUTH_KEY");
        }
    }

    #[test]
    fn test_env_var_strategy_missing() {
        let config = make_auth_config(AuthStrategy::EnvVar, vec!["MISSING_KEY_XYZ"]);
        let result = validate_and_inject(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_env_var_strategy_empty() {
        unsafe {
            std::env::set_var("EMPTY_KEY", "");
        }
        let config = make_auth_config(AuthStrategy::EnvVar, vec!["EMPTY_KEY"]);
        let result = validate_and_inject(&config);
        assert!(result.is_err());
        unsafe {
            std::env::remove_var("EMPTY_KEY");
        }
    }

    #[test]
    fn test_load_auth_config_from_dir() {
        let temp_dir = TempDir::new().unwrap();
        let tool_dir = temp_dir.path().join("my-tool");
        std::fs::create_dir(&tool_dir).unwrap();
        std::fs::write(
            tool_dir.join(".auth.json"),
            r#"{"strategy": "env_var", "required_env_vars": ["MY_KEY"]}"#,
        )
        .unwrap();

        let result = load_auth_config(temp_dir.path(), "my-tool").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().strategy, AuthStrategy::EnvVar);
    }

    #[test]
    fn test_load_auth_config_from_flat_file() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join("my-tool.auth.json"),
            r#"{"strategy": "bearer_token", "required_env_vars": ["BEARER_KEY"]}"#,
        )
        .unwrap();

        let result = load_auth_config(temp_dir.path(), "my-tool").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().strategy, AuthStrategy::BearerToken);
    }

    #[test]
    fn test_load_auth_config_missing() {
        let temp_dir = TempDir::new().unwrap();
        let result = load_auth_config(temp_dir.path(), "no-such-tool").unwrap();
        assert!(result.is_none());
    }
}
