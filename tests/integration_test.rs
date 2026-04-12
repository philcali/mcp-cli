//! Integration tests for mcp-cli server.

use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Spawn the MCP server with optional resources and prompts directories.
fn run_request_with_dirs(
    method: &str,
    params: Option<&serde_json::Value>,
    id: i64,
    resources_dir: Option<PathBuf>,
    prompts_dir: Option<PathBuf>,
) -> serde_json::Value {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));

    // Add directories if provided
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

/// Send a single request and read response (helper for run_request_with_dirs).
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
    prompts_dir: Option<PathBuf>,
    requests: Vec<(&str, Option<&serde_json::Value>)>,
) -> Vec<serde_json::Value> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));

    // Add directories if provided
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

/// Wrapper for prompt tests with prompts_dir only.
fn run_request_sequence_with_prompts(
    prompts_dir: PathBuf,
    requests: Vec<(&str, Option<&serde_json::Value>)>,
) -> Vec<serde_json::Value> {
    run_request_sequence(None, Some(prompts_dir), requests)
}

/// Wrapper for resources tests with resources_dir only.
fn run_request_sequence_with_resources(
    resources_dir: PathBuf,
    requests: Vec<(&str, Option<&serde_json::Value>)>,
) -> Vec<serde_json::Value> {
    run_request_sequence(Some(resources_dir), None, requests)
}

/// Spawn the MCP server and run a single request-response cycle.
fn run_request(method: &str, params: Option<&serde_json::Value>, id: i64) -> serde_json::Value {
    run_request_with_dirs(method, params, id, None, None)
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
    let response = run_request_with_dirs(
        "resources/list",
        None,
        2,
        Some(temp_dir.path().to_path_buf()),
        None,
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

    let results = run_request_sequence_with_resources(
        temp_dir.path().to_path_buf(),
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

    let results = run_request_sequence_with_resources(
        temp_dir.path().to_path_buf(),
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

#[test]
fn test_roots_list_before_initialize() {
    let response = run_request("roots/list", None, 10);

    assert!(
        response.get("error").is_some(),
        "Expected error before initialize"
    );
    assert_eq!(response["id"], serde_json::Value::Number(10.into()));
}

#[test]
fn test_roots_list_with_client_roots() {
    // Client sends roots during initialization
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

    // Check initialization succeeded
    assert!(
        results[0].get("result").is_some(),
        "Expected successful initialize"
    );

    // Check roots/list returns the client-provided roots
    let roots_result = &results[1]["result"];
    let roots_array = roots_result["roots"].as_array().unwrap();

    assert_eq!(roots_array.len(), 2, "Should return both root directories");

    // Verify first root has name
    assert_eq!(roots_array[0]["uri"], "file:///home/user/project");
    assert_eq!(roots_array[0]["name"], "project");

    // Verify second root without name (should not have name field or be null)
    assert_eq!(roots_array[1]["uri"], "file:///tmp/data");
}

#[test]
fn test_roots_list_without_client_roots_capability() {
    // Client initializes without roots capability but server still supports it
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

    // Initialization should succeed (server has roots capability)
    assert!(
        results[0].get("result").is_some(),
        "Expected successful initialize"
    );

    // Roots list returns empty since client didn't provide any
    let roots_result = &results[1]["result"];
    let roots_array = roots_result["roots"].as_array().unwrap();
    assert_eq!(roots_array.len(), 0, "Should return empty list");
}

/// Setup test prompts directory with sample prompt files.
fn setup_test_prompts() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a code review prompt
    fs::write(
        temp_dir.path().join("code-review.json"),
        r#"{
            "name": "code-review",
            "description": "Review code for bugs and improvements",
            "arguments": [
                {"name": "language", "required": false},
                {"name": "code", "required": true}
            ],
            "messages": [
                {"role": "system", "content": "You are a code reviewer."},
                {"role": "user", "content": "Review this {{language}} code: {{code}}"}
            ]
        }"#,
    )
    .unwrap();

    // Create a debug prompt with required args only
    fs::write(
        temp_dir.path().join("debug.json"),
        r#"{
            "name": "debug",
            "description": "Debug errors",
            "arguments": [
                {"name": "error", "required": true}
            ],
            "messages": [
                {"role": "system", "content": "Help debug this error."},
                {"role": "user", "content": "{{error}}"}
            ]
        }"#,
    )
    .unwrap();

    temp_dir
}

#[test]
fn test_prompts_list_before_initialize() {
    let response = run_request("prompts/list", None, 1);

    assert!(
        response.get("error").is_some(),
        "Expected error before initialize"
    );
    assert_eq!(response["id"], serde_json::Value::Number(1.into()));
}

#[test]
fn test_prompts_list_with_directory() {
    let temp_dir = setup_test_prompts();

    // Initialize first
    let params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    // Initialize and list prompts on same server process
    let results = run_request_sequence_with_prompts(
        temp_dir.path().to_path_buf(),
        vec![("initialize", Some(&params)), ("prompts/list", None)],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[0].get("result").is_some(),
        "Expected successful initialize"
    );

    // Now check prompts list result
    let response = &results[1];

    assert_eq!(response["jsonrpc"], "2.0");
    assert!(
        response.get("result").is_some(),
        "Expected result for prompts/list"
    );

    let result = response["result"]
        .as_object()
        .expect("Expected result object");

    if !result.contains_key("prompts") {
        panic!(
            "Response does not contain 'prompts' key. Full response: {:?}",
            response
        );
    }

    let prompts = result["prompts"]
        .as_array()
        .expect("Expected prompts to be an array");

    // Should have 2 prompt files
    assert_eq!(
        prompts.len(),
        2,
        "Should discover all 2 prompt files: {:?}",
        prompts
    );

    // Check that we found the expected prompts
    let names: Vec<&str> = prompts
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();

    assert!(names.contains(&"code-review"), "Should include code-review");
    assert!(names.contains(&"debug"), "Should include debug");
}

#[test]
fn test_prompts_get_with_args() {
    let temp_dir = setup_test_prompts();

    // Initialize and get prompt in sequence
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let get_params = serde_json::json!({
        "name": "code-review",
        "arguments": {
            "language": "rust",
            "code": "fn main() { println!(\"Hello\"); }"
        }
    });

    let results = run_request_sequence_with_prompts(
        temp_dir.path().to_path_buf(),
        vec![
            ("initialize", Some(&init_params)),
            ("prompts/get", Some(&get_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("result").is_some(),
        "Expected result for prompts/get, got error: {:?}",
        results[1].get("error")
    );

    let result = results[1]["result"].as_object().unwrap();
    assert_eq!(
        result["description"],
        "Review code for bugs and improvements"
    );

    let messages = result["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);

    // Check that variables were substituted
    let user_msg = &messages[1]["content"];
    assert!(user_msg.as_str().unwrap().contains("rust"));
    assert!(user_msg.as_str().unwrap().contains("fn main()"));
}

#[test]
fn test_prompts_get_missing_required_arg() {
    let temp_dir = setup_test_prompts();

    // Initialize first
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    // Try to get debug prompt without required 'error' argument
    let get_params = serde_json::json!({
        "name": "debug",
        "arguments": {}
    });

    let results = run_request_sequence_with_prompts(
        temp_dir.path().to_path_buf(),
        vec![
            ("initialize", Some(&init_params)),
            ("prompts/get", Some(&get_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "Expected error for missing required argument"
    );
}

#[test]
fn test_prompts_get_not_found() {
    let temp_dir = setup_test_prompts();

    // Initialize first
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    // Try to get non-existent prompt
    let get_params = serde_json::json!({
        "name": "nonexistent-prompt",
        "arguments": {}
    });

    let results = run_request_sequence_with_prompts(
        temp_dir.path().to_path_buf(),
        vec![
            ("initialize", Some(&init_params)),
            ("prompts/get", Some(&get_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    eprintln!("Non-existent prompt response: {:?}", results[1]);
    assert!(
        results[1].get("error").is_some(),
        "Expected error for non-existent prompt, got: {:?}",
        results[1]
    );
}

#[test]
fn test_tools_call_with_directory() {
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

    // Use a custom helper since we need tools_dir
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
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

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

    // Use a custom helper since we need tools_dir
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
/// RESOURCE SUBSCRIPTION TESTS
// ===========================================================================

#[test]
fn test_resources_subscribe_before_initialize() {
    let subscribe_params = serde_json::json!({
        "uri": "file:///test/resource.txt"
    });

    let response = run_request("resources/subscribe", Some(&subscribe_params), 1);

    assert!(
        response.get("error").is_some(),
        "Expected error before initialize"
    );
}

#[test]
fn test_resources_subscribe_valid_resource() {
    let temp_dir = setup_test_resources();

    // Initialize first
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let subscribe_params = serde_json::json!({
        "uri": format!("file://{}/hello.txt", temp_dir.path().display())
    });

    let results = run_request_sequence_with_resources(
        temp_dir.path().to_path_buf(),
        vec![
            ("initialize", Some(&init_params)),
            ("resources/subscribe", Some(&subscribe_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[0].get("result").is_some(),
        "Expected successful initialize"
    );
    assert!(
        results[1].get("result").is_some(),
        "Expected successful subscribe, got error: {:?}",
        results[1].get("error")
    );
}

#[test]
fn test_resources_subscribe_nonexistent_resource() {
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let subscribe_params = serde_json::json!({
        "uri": "file:///nonexistent/resource.txt"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("resources/subscribe", Some(&subscribe_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "Expected error for non-existent resource"
    );
}

#[test]
fn test_resources_unsubscribe_valid_resource() {
    let temp_dir = setup_test_resources();

    // Initialize, subscribe, then unsubscribe in sequence
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let subscribe_params = serde_json::json!({
        "uri": format!("file://{}/hello.txt", temp_dir.path().display())
    });

    let unsubscribe_params = serde_json::json!({
        "uri": format!("file://{}/hello.txt", temp_dir.path().display())
    });

    let results = run_request_sequence_with_resources(
        temp_dir.path().to_path_buf(),
        vec![
            ("initialize", Some(&init_params)),
            ("resources/subscribe", Some(&subscribe_params)),
            ("resources/unsubscribe", Some(&unsubscribe_params)),
        ],
    );

    assert_eq!(results.len(), 3);
    assert!(
        results[0].get("result").is_some(),
        "Expected successful initialize"
    );
    assert!(
        results[1].get("result").is_some(),
        "Expected successful subscribe, got error: {:?}",
        results[1].get("error")
    );
    assert!(
        results[2].get("result").is_some(),
        "Expected successful unsubscribe, got error: {:?}",
        results[2].get("error")
    );
}

#[test]
fn test_resources_unsubscribe_nonexistent_resource() {
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let unsubscribe_params = serde_json::json!({
        "uri": "file:///nonexistent/resource.txt"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("resources/unsubscribe", Some(&unsubscribe_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "Expected error for non-existent resource"
    );
}

#[test]
fn test_resources_subscribe_and_read() {
    let temp_dir = setup_test_resources();

    // Initialize, subscribe, then read in sequence
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let subscribe_params = serde_json::json!({
        "uri": format!("file://{}/hello.txt", temp_dir.path().display())
    });

    let read_params = serde_json::json!({
        "uri": format!("file://{}/hello.txt", temp_dir.path().display())
    });

    let results = run_request_sequence_with_resources(
        temp_dir.path().to_path_buf(),
        vec![
            ("initialize", Some(&init_params)),
            ("resources/subscribe", Some(&subscribe_params)),
            ("resources/read", Some(&read_params)),
        ],
    );

    assert_eq!(results.len(), 3);
    assert!(
        results[0].get("result").is_some(),
        "Expected successful initialize"
    );
    assert!(
        results[1].get("result").is_some(),
        "Expected successful subscribe, got error: {:?}",
        results[1].get("error")
    );

    let result = &results[2]["result"];
    assert!(
        result.get("contents").is_some(),
        "Expected contents in read result"
    );

    let contents = result["contents"].as_array().unwrap();
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["text"], "Hello, World!");
}
