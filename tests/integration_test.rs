//! Integration tests for mcp-cli server.

use tempfile::TempDir;

pub mod common;
pub mod surface_areas;

use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[cfg(unix)]
use libc::SIGTERM;

/// Spawn the MCP server with optional resources and prompts directories.
fn run_request_with_dirs(
    method: &str,
    params: Option<&serde_json::Value>,
    id: i64,
    resources_dir: Option<PathBuf>,
    prompts_dir: Option<PathBuf>,
) -> serde_json::Value {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));

    if let Some(ref dir) = resources_dir {
        cmd.arg("--resources-dir").arg(dir.to_str().unwrap());
    }
    if let Some(ref dir) = prompts_dir {
        cmd.arg("--prompts-dir").arg(dir.to_str().unwrap());
    }

    let child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn mcp-cli");

    send_request_and_read_response(child, method, params, id)
}

/// Send a single request and read response.
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

    if let Some(mut stdin) = child.stdin.take() {
        writeln!(stdin, "{}", request).unwrap();
        drop(stdin);
    }

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

/// Spawn server and send multiple requests.
fn run_request_sequence(
    resources_dir: Option<PathBuf>,
    prompts_dir: Option<PathBuf>,
    requests: Vec<(&str, Option<&serde_json::Value>)>,
) -> Vec<serde_json::Value> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));

    if let Some(ref dir) = resources_dir {
        cmd.arg("--resources-dir").arg(dir.to_str().unwrap());
    }
    if let Some(ref dir) = prompts_dir {
        cmd.arg("--prompts-dir").arg(dir.to_str().unwrap());
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn mcp-cli");

    let mut results: Vec<serde_json::Value> = Vec::new();

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
            stdin.flush().unwrap();

            if let Some(ref mut stdout) = child.stdout {
                let line = std::io::BufReader::new(stdout)
                    .lines()
                    .map_while(|l| l.ok())
                    .find(|line| line.trim_start().starts_with('{'));

                if let Some(line) = line {
                    results.push(serde_json::from_str(&line).expect("Failed to parse response"));
                }
            }
        }
    }

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

/// Run requests in daemon mode.
fn run_request_sequence_daemon(
    requests: Vec<(&str, Option<&serde_json::Value>)>,
) -> Vec<serde_json::Value> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));
    cmd.arg("--daemon");

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn mcp-cli daemon");

    let mut results: Vec<serde_json::Value> = Vec::new();

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
            stdin.flush().unwrap();

            if let Some(ref mut stdout) = child.stdout {
                let line = std::io::BufReader::new(stdout)
                    .lines()
                    .map_while(|l| l.ok())
                    .find(|line| line.trim_start().starts_with('{'));

                if let Some(line) = line {
                    results.push(serde_json::from_str(&line).expect("Failed to parse response"));
                }
            }
        }
    }

    #[cfg(unix)]
    unsafe {
        libc::kill(child.id() as i32, SIGTERM);
    }

    let output = child.wait_with_output().unwrap();
    tracing::info!("Daemon process exited with status: {:?}", output.status);

    results
}

// ===========================================================================
// CORE FUNCTIONALITY TESTS
// ===========================================================================

#[test]
fn test_ping_before_initialize() {
    let response = run_request_with_dirs("ping", None, 1, None, None);

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response.get("error"), None);
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

    let response = run_request_with_dirs("initialize", Some(&params), 1, None, None);

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
    let response = run_request_with_dirs("tools/list", None, 2, None, None);

    assert!(
        response.get("error").is_some(),
        "Expected error before initialize"
    );
    assert_eq!(response["id"], serde_json::Value::Number(2.into()));
}

#[test]
fn test_unknown_method() {
    let response = run_request_with_dirs("unknown/method", None, 3, None, None);

    assert!(
        response.get("error").is_some(),
        "Expected error for unknown method"
    );
    assert_eq!(response["id"], serde_json::Value::Number(3.into()));
}

#[test]
fn test_resources_endpoints() {
    let response = run_request_with_dirs("resources/list", None, 5, None, None);

    assert_eq!(response["jsonrpc"], "2.0");
    assert!(
        response.get("result").is_some(),
        "Expected result for resources/list"
    );
}

#[test]
fn test_roots_list_before_initialize() {
    let response = run_request_with_dirs("roots/list", None, 10, None, None);

    assert!(
        response.get("error").is_some(),
        "Expected error before initialize"
    );
    assert_eq!(response["id"], serde_json::Value::Number(10.into()));
}

#[test]
fn test_roots_list_with_client_roots() {
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "roots": {
                "listChanged": true
            }
        },
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        },
        "roots": [
            { "uri": "file:///home/user/project", "name": "project" },
            { "uri": "file:///tmp/data" }
        ]
    });

    let results = run_request_sequence(
        None,
        None,
        vec![("initialize", Some(&init_params)), ("roots/list", None)],
    );

    assert_eq!(results.len(), 2);

    assert!(
        results[0].get("result").is_some(),
        "Expected successful initialize"
    );

    let roots_result = &results[1]["result"];
    let roots_array = roots_result["roots"].as_array().unwrap();

    assert_eq!(roots_array.len(), 2, "Should return both root directories");

    assert_eq!(roots_array[0]["uri"], "file:///home/user/project");
    assert_eq!(roots_array[0]["name"], "project");

    assert_eq!(roots_array[1]["uri"], "file:///tmp/data");
}

#[test]
fn test_roots_list_without_client_roots_capability() {
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let results = run_request_sequence(
        None,
        None,
        vec![("initialize", Some(&init_params)), ("roots/list", None)],
    );

    assert_eq!(results.len(), 2);

    assert!(
        results[0].get("result").is_some(),
        "Expected successful initialize"
    );

    let roots_result = &results[1]["result"];
    let roots_array = roots_result["roots"].as_array().unwrap();
    assert_eq!(roots_array.len(), 0, "Should return empty list");
}

// ===========================================================================
// TOOLS TESTS
// ===========================================================================

fn setup_test_tools() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    fs::write(
        temp_dir.path().join("echo-tool.sh"),
        "#!/bin/bash\ninput=$(cat)\necho \"Echo: $input\"\n",
    )
    .unwrap();

    std::fs::set_permissions(
        temp_dir.path().join("echo-tool.sh"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    temp_dir
}

#[test]
fn test_tools_call_with_directory() {
    let temp_dir = setup_test_tools();

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let call_params = serde_json::json!({
        "name": "echo-tool",
        "arguments": {"message": "Hello MCP"}
    });

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));
    cmd.arg("--tools-dir")
        .arg(temp_dir.path().to_str().unwrap());

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn mcp-cli");

    let requests = [
        ("initialize", Some(&init_params)),
        ("tools/call", Some(&call_params)),
    ];

    let mut results: Vec<serde_json::Value> = Vec::new();
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

    child.wait().unwrap();

    assert_eq!(results.len(), 2);

    let call_result = &results[1]["result"];
    assert!(
        call_result.get("content").is_some(),
        "Should have content in result"
    );

    let content_array = call_result["content"].as_array().unwrap();
    assert_eq!(content_array.len(), 1);

    let text_content = &content_array[0]["text"];
    assert!(text_content.as_str().unwrap().contains("Echo"));
}

#[test]
fn test_tools_call_not_found() {
    let temp_dir = setup_test_tools();

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let call_params = serde_json::json!({
        "name": "nonexistent-tool",
        "arguments": {}
    });

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));
    cmd.arg("--tools-dir")
        .arg(temp_dir.path().to_str().unwrap());

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn mcp-cli");

    let requests = [
        ("initialize", Some(&init_params)),
        ("tools/call", Some(&call_params)),
    ];

    let mut results: Vec<serde_json::Value> = Vec::new();
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

    child.wait().unwrap();

    assert_eq!(results.len(), 2);

    assert!(
        results[1].get("error").is_some(),
        "Expected error for non-existent tool"
    );
}

// ===========================================================================
// DAEMON MODE TESTS
// ===========================================================================

#[test]
fn test_daemon_mode_initialize_and_list() {
    let results = run_request_sequence_daemon(vec![
        (
            "initialize",
            Some(&serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0"
                }
            })),
        ),
        ("resources/list", None),
    ]);

    assert_eq!(results.len(), 2);

    assert!(
        results[0].get("result").is_some(),
        "Expected successful initialize, got: {:?}",
        results[0]
    );

    let resources_result = &results[1]["result"];
    assert!(
        resources_result.is_object(),
        "Expected resources/list to return an object"
    );
}

#[test]
fn test_daemon_mode_multiple_requests() {
    let results = run_request_sequence_daemon(vec![
        (
            "initialize",
            Some(&serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0"
                }
            })),
        ),
        ("resources/list", None),
        ("roots/list", None),
    ]);

    assert_eq!(results.len(), 3);

    for (i, result) in results.iter().enumerate() {
        assert!(
            result.get("result").is_some(),
            "Request {} should succeed, got error: {:?}",
            i + 1,
            result.get("error")
        );
    }
}
