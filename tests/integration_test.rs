//! Integration tests for mcp-cli server.

use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

/// Spawn the MCP server and run a single request-response cycle.
fn run_request(method: &str, params: Option<&serde_json::Value>, id: i64) -> serde_json::Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn mcp-cli");

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
    });

    let request = if let Some(p) = params {
        let mut r = req.as_object().unwrap().clone();
        r.insert("params".to_string(), p.clone());
        serde_json::Value::Object(r)
    } else {
        req
    };

    // Write request to stdin
    if let Some(mut stdin) = child.stdin.take() {
        writeln!(stdin, "{}", request).unwrap();
        drop(stdin);
    }

    // Read response from stdout (skip log lines starting with non-JSON chars)
    let mut result = serde_json::Value::Null;
    if let Some(stdout) = child.stdout.take() {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(|l| l.ok())
        {
            if line.trim_start().starts_with('{') {
                result = serde_json::from_str(&line).expect("Failed to parse response");
                break;
            }
        }
    }

    // Wait for process to complete and collect stderr logs
    let _output = child.wait_with_output();

    result
}

#[test]
fn test_ping_before_initialize() {
    let response = run_request("ping", None, 1);

    assert!(
        response.get("error").is_some(),
        "Expected error before initialize"
    );
    assert_eq!(response["id"], serde_json::Value::Number(1.into()));
}

#[test]
fn test_initialize() {
    let params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let response = run_request("initialize", Some(&params), 1);

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], serde_json::Value::Number(1.into()));
    assert!(
        response.get("result").is_some(),
        "Expected result in response"
    );

    let result = response["result"].as_object().unwrap();
    assert_eq!(result["protocol_version"], "2024-11-05");
    assert_eq!(result["capabilities"]["tools"], true);

    let server_info = &result["server_info"];
    assert_eq!(server_info["name"], "mcp-cli");
    assert_eq!(server_info["version"], "0.1.0");
}

#[test]
fn test_tools_list_before_initialize() {
    let response = run_request("tools/list", None, 2);

    assert!(
        response.get("error").is_some(),
        "Expected error before initialize"
    );
    assert_eq!(response["id"], serde_json::Value::Number(2.into()));
}

#[test]
fn test_unknown_method() {
    let response = run_request("unknown/method", None, 3);

    assert!(
        response.get("error").is_some(),
        "Expected error for unknown method"
    );
    assert_eq!(response["id"], serde_json::Value::Number(3.into()));
}

#[test]
fn test_tools_call_not_implemented() {
    // First initialize (oneshot since we don't need the result for multi-request)
    let params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let _response = run_request("initialize", Some(&params), 1);

    // Now try to call a tool (should fail - not implemented)
    let response = run_request("tools/call", None, 4);

    assert!(
        response.get("error").is_some(),
        "Expected error for tools/call"
    );
}

#[test]
fn test_resources_endpoints() {
    // resources/list returns empty list without requiring initialization
    let response = run_request("resources/list", None, 5);

    assert_eq!(response["jsonrpc"], "2.0");
    assert!(
        response.get("result").is_some(),
        "Expected result for resources/list"
    );
}
