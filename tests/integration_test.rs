//! Integration tests for mcp-cli server.

use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Spawn the MCP server with optional resources directory.
fn run_request_with_resources(
    method: &str,
    params: Option<&serde_json::Value>,
    id: i64,
    resources_dir: Option<PathBuf>,
) -> serde_json::Value {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));

    // Add resources directory argument if provided (use --resources-dir flag for tests)
    if let Some(ref dir) = resources_dir {
        cmd.arg("--resources-dir").arg(dir.to_str().unwrap());
    }

    let child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn mcp-cli");

    send_request_and_read_response(child, method, params, id)
}

/// Send a single request and read response (helper for run_request_with_resources).
fn send_request_and_read_response(
    mut child: std::process::Child,
    method: &str,
    params: Option<&serde_json::Value>,
    id: i64,
) -> serde_json::Value {
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
    });

    let request = if let Some(p) = params {
        let mut r = req.as_object().unwrap().clone();
        r.insert("params".to_string(), p.to_owned());
        serde_json::Value::Object(r)
    } else {
        req
    };

    // Write request to stdin
    if let Some(mut stdin) = child.stdin.take() {
        writeln!(stdin, "{}", request).unwrap();
        drop(stdin);
    }

    // Read response from stdout
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

    let _output = child.wait_with_output();
    result
}

/// Spawn server and send multiple requests (for multi-step test flows).
fn run_request_sequence(
    resources_dir: Option<PathBuf>,
    requests: Vec<(&str, Option<&serde_json::Value>)>,
) -> Vec<serde_json::Value> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));

    // Add resources directory argument if provided (use --resources-dir flag for tests)
    if let Some(ref dir) = resources_dir {
        cmd.arg("--resources-dir").arg(dir.to_str().unwrap());
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn mcp-cli");

    let mut results = Vec::new();

    if let Some(mut stdin) = child.stdin.take() {
        for (i, (method, params)) in requests.iter().enumerate() {
            let id = i as i64 + 1;
            let req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
            });

            let request = if let Some(p) = params {
                let mut r = req.as_object().unwrap().clone();
                r.insert("params".to_string(), (*p).clone());
                serde_json::Value::Object(r)
            } else {
                req
            };

            writeln!(stdin, "{}", request).unwrap();
        }
    }

    // Read all responses
    if let Some(stdout) = child.stdout.take() {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(|l| l.ok())
        {
            if line.trim_start().starts_with('{') {
                results.push(serde_json::from_str(&line).expect("Failed to parse response"));
            }
        }
    }

    let _output = child.wait_with_output();
    results
}

/// Spawn the MCP server and run a single request-response cycle.
fn run_request(method: &str, params: Option<&serde_json::Value>, id: i64) -> serde_json::Value {
    run_request_with_resources(method, params, id, None)
}

/// Setup test resources directory with sample files.
fn setup_test_resources() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a text file
    fs::write(temp_dir.path().join("hello.txt"), "Hello, World!").unwrap();

    // Create a JSON file
    fs::write(
        temp_dir.path().join("config.json"),
        r#"{"key": "value", "number": 42}"#,
    )
    .unwrap();

    // Create a markdown file
    fs::write(
        temp_dir.path().join("readme.md"),
        "# Test Resource\nThis is a test.",
    )
    .unwrap();

    temp_dir
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

#[test]
fn test_resources_list_with_directory() {
    let temp_dir = setup_test_resources();

    let params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let _response = run_request("initialize", Some(&params), 1);

    // Now list resources from the temp directory
    let response = run_request_with_resources(
        "resources/list",
        None,
        2,
        Some(temp_dir.path().to_path_buf()),
    );

    assert_eq!(response["jsonrpc"], "2.0");
    assert!(
        response.get("result").is_some(),
        "Expected result for resources/list"
    );

    let result = response["result"].as_object().unwrap();
    let resources = result["resources"].as_array().unwrap();

    // Should have 3 resource files
    assert_eq!(resources.len(), 3, "Should discover all 3 resource files");

    // Check that we found the expected files
    let uris: Vec<&str> = resources
        .iter()
        .map(|r| r["uri"].as_str().unwrap())
        .collect();

    assert!(
        uris.iter().any(|u| u.contains("hello.txt")),
        "Should include hello.txt"
    );
    assert!(
        uris.iter().any(|u| u.contains("config.json")),
        "Should include config.json"
    );
    assert!(
        uris.iter().any(|u| u.contains("readme.md")),
        "Should include readme.md"
    );
}

#[test]
fn test_resources_read_text_file() {
    let temp_dir = setup_test_resources();

    // Initialize and read in sequence on same server process
    let params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let read_params = serde_json::json!({
        "uri": format!("file://{}/hello.txt", temp_dir.path().display())
    });

    let results = run_request_sequence(
        Some(temp_dir.path().to_path_buf()),
        vec![
            ("initialize", Some(&params)),
            ("resources/read", Some(&read_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    eprintln!("Response: {:?}", results[1]);
    assert!(
        results[1].get("result").is_some(),
        "Expected result for resources/read, got error: {:?}",
        results[1].get("error")
    );

    let result = results[1]["result"].as_object().unwrap();
    let contents = result["contents"].as_array().unwrap();
    assert_eq!(contents.len(), 1);

    let content = &contents[0];
    assert_eq!(content["text"], "Hello, World!");
}

#[test]
fn test_resources_read_json_file() {
    let temp_dir = setup_test_resources();

    // Initialize and read in sequence on same server process
    let params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let read_params = serde_json::json!({
        "uri": format!("file://{}/config.json", temp_dir.path().display())
    });

    let results = run_request_sequence(
        Some(temp_dir.path().to_path_buf()),
        vec![
            ("initialize", Some(&params)),
            ("resources/read", Some(&read_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    eprintln!("Response: {:?}", results[1]);
    assert!(
        results[1].get("result").is_some(),
        "Expected result for resources/read, got error: {:?}",
        results[1].get("error")
    );

    let result = results[1]["result"].as_object().unwrap();
    let contents = result["contents"].as_array().unwrap();
    assert_eq!(contents.len(), 1);

    let content = &contents[0];
    assert_eq!(content["mimeType"], "application/json");
}

#[test]
fn test_resources_read_not_found() {
    // Initialize and try to read in sequence on same server process
    let params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let read_params = serde_json::json!({
        "uri": "file:///nonexistent/resource.txt"
    });

    let results = run_request_sequence(
        None,
        vec![
            ("initialize", Some(&params)),
            ("resources/read", Some(&read_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "Expected error for non-existent resource"
    );
}
