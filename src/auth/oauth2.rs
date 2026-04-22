//! OAuth2 client credentials flow.
//!
//! Reads client credentials from environment variables, fetches a token,
//! and caches it for reuse.

use crate::auth::token_cache::TokenCache;
use crate::protocol::OAuthConfig;
use crate::protocol::ToolAuthConfig;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default = "default_ttl")]
    expires_in: u64,
}

fn default_ttl() -> u64 {
    3600
}

pub async fn resolve_oauth2(
    config: &OAuthConfig,
    auth_config: &ToolAuthConfig,
    tool_name: &str,
    cache: &TokenCache,
) -> Result<HashMap<String, String>> {
    let cache_key = format!("oauth2:{}", tool_name);

    // Check cache first
    if let Some(token) = cache.get(&cache_key) {
        return Ok(HashMap::from([("OAUTH_ACCESS_TOKEN".to_string(), token)]));
    }

    // Read client credentials from environment
    let client_id = std::env::var(&config.client_id_env).with_context(|| {
        format!(
            "Missing env var '{}' for OAuth2 client_id",
            config.client_id_env
        )
    })?;

    let client_secret = auth_config
        .required_env_vars
        .first()
        .map(std::env::var)
        .ok_or_else(|| anyhow::anyhow!("No client_secret_env configured for OAuth2"))??;

    // Fetch fresh token
    let resp = reqwest::Client::new()
        .post(&config.token_url)
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
        ])
        .send()
        .await
        .context("Failed to send OAuth2 token request")?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "OAuth2 token request failed (HTTP {}): {}",
            status,
            body
        ));
    }

    let token_resp: TokenResponse = resp
        .json()
        .await
        .context("Failed to parse OAuth2 token response")?;

    let ttl = std::cmp::max(token_resp.expires_in, 60);
    cache.set(&cache_key, token_resp.access_token.clone(), ttl);

    Ok(HashMap::from([(
        "OAUTH_ACCESS_TOKEN".to_string(),
        token_resp.access_token,
    )]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_config_deserialization() {
        let json = r#"{
            "client_id_env": "OAUTH_CLIENT_ID",
            "token_url": "https://auth.example.com/token",
            "scopes": ["read", "write"]
        }"#;
        let config: OAuthConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.client_id_env, "OAUTH_CLIENT_ID");
        assert_eq!(config.token_url, "https://auth.example.com/token");
        assert_eq!(config.scopes, vec!["read", "write"]);
    }
}
