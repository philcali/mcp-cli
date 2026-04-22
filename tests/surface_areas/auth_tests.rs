//! Integration tests for tool authentication strategies.

use mcp_cli::auth::token_cache::TokenCache;
use mcp_cli::auth::api_key;
use mcp_cli::auth::bearer;
use mcp_cli::protocol::{AuthStrategy, OAuthConfig, ToolAuthConfig};
use tempfile::TempDir;

use crate::common::run_request_sequence_all;

fn make_env_config(env_vars: Vec<&str>) -> ToolAuthConfig {
    ToolAuthConfig {
        strategy: AuthStrategy::EnvVar,
        required_env_vars: env_vars.into_iter().map(String::from).collect(),
        oauth_config: None,
    }
}

fn make_oauth_config(client_id_env: &str, token_url: &str) -> ToolAuthConfig {
    ToolAuthConfig {
        strategy: AuthStrategy::OAuth2,
        required_env_vars: vec!["OAUTH_CLIENT_SECRET".to_string()],
        oauth_config: Some(OAuthConfig {
            client_id_env: client_id_env.to_string(),
            token_url: token_url.to_string(),
            scopes: vec![],
        }),
    }
}

fn make_api_key_config(env_var: &str) -> ToolAuthConfig {
    ToolAuthConfig {
        strategy: AuthStrategy::ApiKeyHeader,
        required_env_vars: vec![env_var.to_string()],
        oauth_config: None,
    }
}

fn make_bearer_config(env_var: &str) -> ToolAuthConfig {
    ToolAuthConfig {
        strategy: AuthStrategy::BearerToken,
        required_env_vars: vec![env_var.to_string()],
        oauth_config: None,
    }
}

// ===========================================================================
// UNIT TESTS FOR STRATEGY RESOLVERS
// ===========================================================================

#[test]
fn test_token_cache_basic() {
    let cache = TokenCache::new();
    cache.set("tool1", "tok_abc".to_string(), 300);
    assert_eq!(cache.get("tool1"), Some("tok_abc".to_string()));
    assert_eq!(cache.get("tool2"), None);
}

#[test]
fn test_token_cache_expired() {
    let cache = TokenCache::new();
    cache.set("tool1", "tok_abc".to_string(), 0);
    assert_eq!(cache.get("tool1"), None);
}

#[test]
fn test_api_key_resolve_success() {
    unsafe { std::env::set_var("MY_API_KEY", "key123"); }
    let config = make_api_key_config("MY_API_KEY");
    let result = api_key::resolve(&config).unwrap();
    assert_eq!(result.get("API_KEY"), Some(&"key123".to_string()));
    unsafe { std::env::remove_var("MY_API_KEY"); }
}

#[test]
fn test_api_key_resolve_missing() {
    let config = make_api_key_config("NONEXISTENT_KEY_XYZ");
    assert!(api_key::resolve(&config).is_err());
}

#[test]
fn test_bearer_resolve_success() {
    unsafe { std::env::set_var("MY_TOKEN", "bearer_val"); }
    let config = make_bearer_config("MY_TOKEN");
    let result = bearer::resolve(&config).unwrap();
    assert_eq!(result.get("BEARER_TOKEN"), Some(&"bearer_val".to_string()));
    unsafe { std::env::remove_var("MY_TOKEN"); }
}

#[test]
fn test_bearer_resolve_missing() {
    let config = make_bearer_config("NONEXISTENT_TOKEN_XYZ");
    assert!(bearer::resolve(&config).is_err());
}

// ===========================================================================
// INTEGRATION TESTS - auth config loading
// ===========================================================================

fn setup_tool_with_auth(tool_name: &str, auth_content: &str, output_var: &str) -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    // Create .auth.json in the tools dir (flat file)
    let auth_path = temp_dir.path().join(format!("{tool_name}.auth.json"));
    std::fs::write(&auth_path, auth_content).unwrap();

    // Create the tool script
    let tool_path = temp_dir.path().join(tool_name);
    std::fs::write(
        &tool_path,
        format!("#!/bin/bash\necho \"{output_var}=${{{output_var}}}\"\n"),
    ).unwrap();
    std::fs::set_permissions(
        &tool_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    ).unwrap();

    temp_dir
}

fn setup_tool_with_auth_no_exec(tool_name: &str, auth_content: &str) -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    // Create .auth.json in the tools dir (flat file)
    let auth_path = temp_dir.path().join(format!("{tool_name}.auth.json"));
    std::fs::write(&auth_path, auth_content).unwrap();

    // Create a tool script that is NOT executable so it won't be discovered
    let tool_path = temp_dir.path().join(tool_name);
    std::fs::write(
        &tool_path,
        "#!/bin/bash\necho \"should-not-run\"\n",
    ).unwrap();
    // Explicitly set non-executable
    std::fs::set_permissions(
        &tool_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o644),
    ).unwrap();

    temp_dir
}

#[test]
fn test_auth_env_var_strategy_integration() {
    let temp_dir = setup_tool_with_auth(
        "auth-tool",
        r#"{"strategy": "env_var", "required_env_vars": ["AUTH_KEY"]}"#,
        "AUTH_KEY",
    );

    unsafe { std::env::set_var("AUTH_KEY", "secret123"); }

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let call_params = serde_json::json!({
        "name": "auth-tool",
        "arguments": {}
    });

    let output = run_request_sequence_all(
        Some(temp_dir.path().to_path_buf()), // tools_dir
        None,
        None,
        vec![("AUTH_KEY", "secret123")],
        vec![
            ("initialize", Some(&init_params)),
            ("tools/call", Some(&call_params)),
        ],
    );

    unsafe { std::env::remove_var("AUTH_KEY"); }

    if output.results.len() != 2 {
        panic!("Expected 2 results, got {}: {}\ntools dir: {}\nstderr: {}", output.results.len(), serde_json::to_string_pretty(&output.results).unwrap(), temp_dir.path().display(), output.stderr);
    }
    let call_result = &output.results[1];
    if call_result.get("error").is_some() {
        panic!("Tool call failed: {:?}\ntools dir: {}\nstderr: {}", call_result["error"], temp_dir.path().display(), output.stderr);
    }
    let call_result = &call_result["result"];
    let content = call_result["content"][0]["text"].as_str().unwrap();
    assert!(content.contains("secret123"), "Expected 'secret123' in: {}", content);
}

#[test]
fn test_auth_bearer_strategy_integration() {
    let temp_dir = setup_tool_with_auth(
        "bearer-tool",
        r#"{"strategy": "bearer_token", "required_env_vars": ["BEARER_TOKEN"]}"#,
        "BEARER_TOKEN",
    );

    unsafe { std::env::set_var("BEARER_TOKEN", "my-bearer-token"); }

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let call_params = serde_json::json!({
        "name": "bearer-tool",
        "arguments": {}
    });

    let output = run_request_sequence_all(
        Some(temp_dir.path().to_path_buf()), // tools_dir
        None,
        None,
        vec![("BEARER_TOKEN", "my-bearer-token")],
        vec![
            ("initialize", Some(&init_params)),
            ("tools/call", Some(&call_params)),
        ],
    );

    unsafe { std::env::remove_var("BEARER_TOKEN"); }

    assert_eq!(output.results.len(), 2);
    let call_result = &output.results[1];
    if call_result.get("error").is_some() {
        panic!("Tool call failed: {}\ntools dir: {}\nstderr: {}", call_result["error"], temp_dir.path().display(), output.stderr);
    }
    let call_result = &call_result["result"];
    let content = call_result["content"][0]["text"].as_str().unwrap();
    assert!(content.contains("my-bearer-token"), "Expected 'my-bearer-token' in: {}", content);
}

#[test]
fn test_auth_api_key_strategy_integration() {
    let temp_dir = setup_tool_with_auth(
        "apikey-tool",
        r#"{"strategy": "api_key_header", "required_env_vars": ["API_KEY"]}"#,
        "API_KEY",
    );

    unsafe { std::env::set_var("API_KEY", "api-key-value"); }

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let call_params = serde_json::json!({
        "name": "apikey-tool",
        "arguments": {}
    });

    let output = run_request_sequence_all(
        Some(temp_dir.path().to_path_buf()), // tools_dir
        None,
        None,
        vec![("API_KEY", "api-key-value")],
        vec![
            ("initialize", Some(&init_params)),
            ("tools/call", Some(&call_params)),
        ],
    );

    unsafe { std::env::remove_var("API_KEY"); }

    assert_eq!(output.results.len(), 2);
    let call_result = &output.results[1];
    if call_result.get("error").is_some() {
        panic!("Tool call failed: {}\ntools dir: {}\nstderr: {}", call_result["error"], temp_dir.path().display(), output.stderr);
    }
    let call_result = &call_result["result"];
    let content = call_result["content"][0]["text"].as_str().unwrap();
    assert!(content.contains("api-key-value"), "Expected 'api-key-value' in: {}", content);
}

#[test]
fn test_auth_missing_credentials_fails() {
    let temp_dir = setup_tool_with_auth_no_exec(
        "missing-auth-tool",
        r#"{"strategy": "env_var", "required_env_vars": ["UNIQUE_MISSING_AUTH_KEY_XYZ"]}"#,
    );

    // Ensure the env var is unset (it may leak from other tests)
    unsafe { std::env::remove_var("UNIQUE_MISSING_AUTH_KEY_XYZ"); }

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let call_params = serde_json::json!({
        "name": "missing-auth-tool",
        "arguments": {}
    });

    let output = run_request_sequence_all(
        Some(temp_dir.path().to_path_buf()), // tools_dir
        None,
        None,
        vec![],
        vec![
            ("initialize", Some(&init_params)),
            ("tools/call", Some(&call_params)),
        ],
    );

    assert_eq!(output.results.len(), 2);
    assert!(
        output.results[1].get("error").is_some(),
        "Expected error for missing credentials, got: {}\nstderr: {}",
        serde_json::to_string_pretty(&output.results).unwrap(),
        output.stderr
    );
}

#[test]
fn test_auth_flat_auth_file() {
    let temp_dir = TempDir::new().unwrap();

    // Create flat auth file (not in subdirectory)
    std::fs::write(
        temp_dir.path().join("flat-tool.auth.json"),
        r#"{"strategy": "env_var", "required_env_vars": ["FLAT_AUTH_KEY"]}"#,
    ).unwrap();

    // Create a simple tool
    let tool_path = temp_dir.path().join("flat-tool");
    std::fs::write(
        &tool_path,
        "#!/bin/bash\necho \"FLAT=$FLAT_AUTH_KEY\"\n",
    ).unwrap();
    std::fs::set_permissions(
        &tool_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    ).unwrap();

    unsafe { std::env::set_var("FLAT_AUTH_KEY", "flat_secret"); }

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let call_params = serde_json::json!({
        "name": "flat-tool",
        "arguments": {}
    });

    let output = run_request_sequence_all(
        Some(temp_dir.path().to_path_buf()), // tools_dir
        None,
        None,
        vec![("FLAT_AUTH_KEY", "flat_secret")],
        vec![
            ("initialize", Some(&init_params)),
            ("tools/call", Some(&call_params)),
        ],
    );

    unsafe { std::env::remove_var("FLAT_AUTH_KEY"); }

    assert_eq!(output.results.len(), 2);
    let call_result = &output.results[1]["result"];
    let content = &call_result["content"][0]["text"];
    assert!(content.as_str().unwrap().contains("flat_secret"), "Expected 'flat_secret' in: {}\nstderr: {}", content, output.stderr);
}
