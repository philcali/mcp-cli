//! Integration tests for mcp-cli server.

use tempfile::TempDir;

pub mod common;
pub mod surface_areas;

use std::fs;
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

#[cfg(unix)]
use libc::SIGTERM;

// ===========================================================================
// CORE FUNCTIONALITY TESTS
// ===========================================================================

#[test]
fn test_ping_before_initialize() {
    let response = common::run_request_with_dirs("ping", None, 1, None, None);

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response.get("error"), None);
    assert_eq!(response["id"], serde_json::Value::Number(1.into()));
    assert_eq!(response["result"]["initialized"], false);
    assert_eq!(response["result"]["server_info"]["name"], "mcp-cli");
    assert!(response["result"]["capabilities"].get("tools").is_some());
}

#[test]
fn test_ping_after_initialize() {
    let results = common::run_request_sequence_daemon(vec![
        (
            "initialize",
            Some(&serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test-client", "version": "1.0" }
            })),
        ),
        ("ping", None),
    ]);

    let ping_response = &results[1];
    assert_eq!(ping_response["jsonrpc"], "2.0");
    assert_eq!(ping_response["result"]["initialized"], true);
    assert_eq!(ping_response["result"]["server_info"]["name"], "mcp-cli");
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

    let response = common::run_request_with_dirs("initialize", Some(&params), 1, None, None);

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], serde_json::Value::Number(1.into()));
    assert!(
        response.get("result").is_some(),
        "Expected result in response"
    );

    let result = response["result"].as_object().unwrap();
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["capabilities"]["tools"]["listChanged"], true);

    let server_info = &result["serverInfo"];
    assert_eq!(server_info["name"], "mcp-cli");
    assert_eq!(server_info["version"], "0.1.0");
}

#[test]
fn test_tools_list_before_initialize() {
    let response = common::run_request_with_dirs("tools/list", None, 2, None, None);

    assert!(
        response.get("error").is_some(),
        "Expected error before initialize"
    );
    assert_eq!(response["id"], serde_json::Value::Number(2.into()));
}

#[test]
fn test_unknown_method() {
    let response = common::run_request_with_dirs("unknown/method", None, 3, None, None);

    assert!(
        response.get("error").is_some(),
        "Expected error for unknown method"
    );
    assert_eq!(response["id"], serde_json::Value::Number(3.into()));
}

#[test]
fn test_resources_endpoints() {
    let response = common::run_request_with_dirs("resources/list", None, 5, None, None);

    assert_eq!(response["jsonrpc"], "2.0");
    assert!(
        response.get("result").is_some(),
        "Expected result for resources/list"
    );
}

#[test]
fn test_roots_list_before_initialize() {
    let response = common::run_request_with_dirs("roots/list", None, 10, None, None);

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

    let results = common::run_request_sequence(
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

    let results = common::run_request_sequence(
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

    let output = common::run_request_sequence_all(
        Some(temp_dir.path().to_path_buf()),
        None,
        None,
        None,
        vec![],
        vec![
            ("initialize", Some(&init_params)),
            ("tools/call", Some(&call_params)),
        ],
    );

    assert_eq!(output.results.len(), 2);

    let call_result = &output.results[1]["result"];
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

    let output = common::run_request_sequence_all(
        Some(temp_dir.path().to_path_buf()),
        None,
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
        "Expected error for non-existent tool"
    );
}

// ===========================================================================
// DAEMON MODE TESTS
// ===========================================================================

#[test]
fn test_daemon_mode_initialize_and_list() {
    let results = common::run_request_sequence_daemon(vec![
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
    let results = common::run_request_sequence_daemon(vec![
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

#[test]
fn test_resources_updated_notification_on_subscribed_resource() {
    let temp_dir = TempDir::new().unwrap();
    let resource_file = temp_dir.path().join("test-resource.txt");
    fs::write(&resource_file, "initial content").unwrap();

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let resource_uri = format!("file://{}", resource_file.display());

    let subscribe_params = serde_json::json!({ "uri": resource_uri });

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));
    cmd.arg("--daemon")
        .arg("--resources-dir")
        .arg(temp_dir.path().to_str().unwrap());

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn mcp-cli daemon");

    // Spawn a thread to collect stdout while we send requests
    let stdout = child.stdout.take().unwrap();
    let stdout_handle = std::thread::spawn(move || -> Vec<String> {
        std::io::BufReader::new(stdout)
            .lines()
            .map_while(|l| l.ok())
            .collect()
    });

    // Send requests
    if let Some(ref mut stdin) = child.stdin {
        let init_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": init_params,
        });
        writeln!(stdin, "{}", init_req).unwrap();
        stdin.flush().unwrap();

        let sub_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "resources/subscribe",
            "params": subscribe_params,
        });
        writeln!(stdin, "{}", sub_req).unwrap();
        stdin.flush().unwrap();
    }

    // Give watcher time to start
    std::thread::sleep(std::time::Duration::from_millis(300));

    // Modify the resource file
    fs::write(&resource_file, "updated content").unwrap();

    // Give watcher time to detect the change
    std::thread::sleep(std::time::Duration::from_millis(800));

    // Kill the daemon to flush stdout
    #[cfg(unix)]
    unsafe {
        libc::kill(child.id() as i32, SIGTERM);
    }

    let _output = child.wait().unwrap();

    // Collect stdout from the thread
    let all_stdout_lines = stdout_handle.join().unwrap();

    // Parse all lines and find notifications
    let mut notifications: Vec<serde_json::Value> = Vec::new();
    for line in &all_stdout_lines {
        if line.trim_start().starts_with('{') {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
            if parsed.get("method").is_some() && parsed.get("id").is_none() {
                notifications.push(parsed);
            }
        }
    }

    // Check that we received a resources/updated notification
    let updated = notifications.iter().find(|n| {
        n.get("method").and_then(|m| m.as_str()) == Some("notifications/resources/updated")
    });

    assert!(
        updated.is_some(),
        "Expected resources/updated notification, got: {:?}",
        notifications
    );

    if let Some(notification) = updated {
        assert_eq!(notification["params"]["uri"], resource_uri);
    }
}

#[test]
fn test_resources_updated_no_notification_for_unsubscribed_resource() {
    let temp_dir = TempDir::new().unwrap();
    let resource_file_a = temp_dir.path().join("resource-a.txt");
    let resource_file_b = temp_dir.path().join("resource-b.txt");
    fs::write(&resource_file_a, "content a").unwrap();
    fs::write(&resource_file_b, "content b").unwrap();

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let uri_a = format!("file://{}", resource_file_a.display());
    let subscribe_params = serde_json::json!({ "uri": uri_a });

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));
    cmd.arg("--daemon")
        .arg("--resources-dir")
        .arg(temp_dir.path().to_str().unwrap());

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn mcp-cli daemon");

    let stdout = child.stdout.take().unwrap();
    let stdout_handle = std::thread::spawn(move || -> Vec<String> {
        std::io::BufReader::new(stdout)
            .lines()
            .map_while(|l| l.ok())
            .collect()
    });

    if let Some(ref mut stdin) = child.stdin {
        let init_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": init_params,
        });
        writeln!(stdin, "{}", init_req).unwrap();
        stdin.flush().unwrap();

        let sub_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "resources/subscribe",
            "params": subscribe_params,
        });
        writeln!(stdin, "{}", sub_req).unwrap();
        stdin.flush().unwrap();
    }

    std::thread::sleep(std::time::Duration::from_millis(300));

    // Modify the unsubscribed resource
    fs::write(&resource_file_b, "updated content b").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(800));

    #[cfg(unix)]
    unsafe {
        libc::kill(child.id() as i32, SIGTERM);
    }

    let _output = child.wait().unwrap();

    let all_stdout_lines = stdout_handle.join().unwrap();

    let mut notifications: Vec<serde_json::Value> = Vec::new();
    for line in &all_stdout_lines {
        if line.trim_start().starts_with('{') {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
            if parsed.get("method").is_some() && parsed.get("id").is_none() {
                notifications.push(parsed);
            }
        }
    }

    let updated = notifications.iter().any(|n| {
        n.get("method").and_then(|m| m.as_str()) == Some("notifications/resources/updated")
    });

    assert!(
        !updated,
        "Should NOT receive resources/updated for unsubscribed resource, got: {:?}",
        notifications
    );
}

#[test]
fn test_completion_complete_tool_names() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let tools_dir = temp_dir.path();

    // Create some tool scripts
    for name in &["list-files", "delete-file", "get-status"] {
        let script_path = tools_dir.join(name);
        fs::write(&script_path, "#!/bin/sh\necho ok\n").unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(
            &script_path,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();
    }

    let output = common::run_request_sequence_all(
        Some(tools_dir.to_path_buf()),
        None,
        None,
        None,
        vec![],
        vec![
            (
                "initialize",
                Some(&serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "test-client", "version": "1.0" }
                })),
            ),
            (
                "completion/complete",
                Some(&serde_json::json!({
                    "ref": { "type": "tool", "value": "list" },
                    "argument": { "name": "name", "value": "list" }
                })),
            ),
        ],
    );

    // Parse the completion response (2nd result)
    let completion_result = &output.results[1];
    let values: Vec<&str> = completion_result["result"]["values"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    // Should match "list-files" since it starts with "list"
    assert!(
        values.contains(&"list-files"),
        "Expected 'list-files' in completions, got: {:?}",
        values
    );
}

#[test]
fn test_completion_complete_no_matches() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let tools_dir = temp_dir.path();

    // Create some tool scripts
    for name in &["list-files", "delete-file"] {
        let script_path = tools_dir.join(name);
        fs::write(&script_path, "#!/bin/sh\necho ok\n").unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(
            &script_path,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();
    }

    let output = common::run_request_sequence_all(
        Some(tools_dir.to_path_buf()),
        None,
        None,
        None,
        vec![],
        vec![
            (
                "initialize",
                Some(&serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "test-client", "version": "1.0" }
                })),
            ),
            (
                "completion/complete",
                Some(&serde_json::json!({
                    "ref": { "type": "tool", "value": "xyz" },
                    "argument": { "name": "name", "value": "xyz" }
                })),
            ),
        ],
    );

    let completion_result = &output.results[1];
    let values: Vec<&str> = completion_result["result"]["values"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    assert!(values.is_empty(), "Expected no matches, got: {:?}", values);
}

// ===========================================================================
// RESOURCE TEMPLATES TESTS
// ===========================================================================

fn setup_test_templates() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    fs::write(
        temp_dir.path().join("server.template.json"),
        serde_json::json!({
            "uriTemplate": "file://{path}",
            "name": "server",
            "description": "Server files",
            "mimeType": "text/plain"
        })
        .to_string(),
    )
    .unwrap();

    fs::write(
        temp_dir.path().join("config.template.json"),
        serde_json::json!({
            "uriTemplate": "config://{name}.yaml",
            "name": "config",
            "description": "Configuration files",
            "mimeType": "application/yaml"
        })
        .to_string(),
    )
    .unwrap();

    temp_dir
}

#[test]
fn test_resource_templates_list() {
    let temp_dir = setup_test_templates();

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let output = common::run_request_sequence_all(
        None,
        None,
        None,
        Some(temp_dir.path().to_path_buf()),
        vec![],
        vec![
            ("initialize", Some(&init_params)),
            ("resources/templates/list", None),
        ],
    );

    assert_eq!(output.results.len(), 2);

    let templates_result = &output.results[1]["result"];
    let templates_array = templates_result["templates"].as_array().unwrap();
    assert_eq!(templates_array.len(), 2);

    // Sort by name since filesystem order is non-deterministic
    let mut sorted_templates = templates_array.clone();
    sorted_templates.sort_by_key(|t| t["name"].as_str().unwrap().to_string());

    assert_eq!(sorted_templates[0]["name"], "config");
    assert_eq!(sorted_templates[0]["uriTemplate"], "config://{name}.yaml");

    assert_eq!(sorted_templates[1]["name"], "server");
    assert_eq!(sorted_templates[1]["uriTemplate"], "file://{path}");
    assert_eq!(sorted_templates[1]["mimeType"], "text/plain");
}

#[test]
fn test_resource_templates_empty_dir() {
    let temp_dir = TempDir::new().unwrap();

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let output = common::run_request_sequence_all(
        None,
        None,
        None,
        Some(temp_dir.path().to_path_buf()),
        vec![],
        vec![
            ("initialize", Some(&init_params)),
            ("resources/templates/list", None),
        ],
    );

    let templates_array = &output.results[1]["result"]["templates"];
    assert_eq!(templates_array.as_array().unwrap().len(), 0);
}

#[test]
fn test_resource_templates_in_capabilities() {
    let temp_dir = setup_test_templates();

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let output = common::run_request_sequence_all(
        None,
        None,
        None,
        Some(temp_dir.path().to_path_buf()),
        vec![],
        vec![("initialize", Some(&init_params))],
    );

    let caps = &output.results[0]["result"]["capabilities"]["resources"];
    assert!(
        caps.get("templateListChanged").is_some(),
        "Expected templateListChanged in resources capability, got: {:?}",
        caps
    );
    assert_eq!(caps["templateListChanged"], true);
}

/// Test subscribing to a resource via template pattern.
#[test]
fn test_resource_subscribe_via_template() {
    let resources_dir = TempDir::new().unwrap();
    let templates_dir = TempDir::new().unwrap();

    // Create a resource file
    let resource_file = resources_dir.path().join("test.txt");
    fs::write(&resource_file, "content").unwrap();

    // Create a template that matches this resource
    let template_content = serde_json::json!({
        "uriTemplate": "file://{path}",
        "name": "any-file",
        "description": "Matches any file"
    });
    std::fs::write(
        templates_dir.path().join("any-file.template.json"),
        serde_json::to_string_pretty(&template_content).unwrap(),
    )
    .unwrap();

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let resource_uri = format!("file://{}", resource_file.display());
    let subscribe_params = serde_json::json!({ "uri": resource_uri });

    let output = common::run_request_sequence_all(
        None,
        Some(resources_dir.path().to_path_buf()),
        None,
        Some(templates_dir.path().to_path_buf()),
        vec![],
        vec![
            ("initialize", Some(&init_params)),
            ("resources/subscribe", Some(&subscribe_params)),
        ],
    );

    assert_eq!(output.results.len(), 2);
    assert!(
        output.results[1].get("error").is_none(),
        "Subscribe via template should succeed, got: {}\nstderr: {}",
        serde_json::to_string_pretty(&output.results).unwrap(),
        output.stderr
    );
}

/// Test subscribing to a non-matching URI fails.
#[test]
fn test_resource_subscribe_non_matching_template() {
    let templates_dir = TempDir::new().unwrap();

    // Create a template that only matches file:// URIs
    let template_content = serde_json::json!({
        "uriTemplate": "file://{path}",
        "name": "file-templates",
        "description": "Only matches file:// URIs"
    });
    std::fs::write(
        templates_dir.path().join("file-templates.template.json"),
        serde_json::to_string_pretty(&template_content).unwrap(),
    )
    .unwrap();

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let subscribe_params = serde_json::json!({ "uri": "https://example.com/doesnt-match" });

    let output = common::run_request_sequence_all(
        None,
        None,
        None,
        Some(templates_dir.path().to_path_buf()),
        vec![],
        vec![
            ("initialize", Some(&init_params)),
            ("resources/subscribe", Some(&subscribe_params)),
        ],
    );

    assert_eq!(output.results.len(), 2);
    assert!(
        output.results[1].get("error").is_some(),
        "Subscribe should fail for non-matching URI"
    );
}

/// Test unsubscribe works for template-matched resources.
#[test]
fn test_resource_unsubscribe_via_template() {
    let resources_dir = TempDir::new().unwrap();
    let templates_dir = TempDir::new().unwrap();

    // Create a resource file
    let resource_file = resources_dir.path().join("test.txt");
    fs::write(&resource_file, "content").unwrap();

    // Create a matching template
    let template_content = serde_json::json!({
        "uriTemplate": "file://{path}",
        "name": "any-file",
        "description": "Matches any file"
    });
    std::fs::write(
        templates_dir.path().join("any-file.template.json"),
        serde_json::to_string_pretty(&template_content).unwrap(),
    )
    .unwrap();

    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test-client", "version": "1.0" }
    });

    let resource_uri = format!("file://{}", resource_file.display());

    let output = common::run_request_sequence_all(
        None,
        Some(resources_dir.path().to_path_buf()),
        None,
        Some(templates_dir.path().to_path_buf()),
        vec![],
        vec![
            ("initialize", Some(&init_params)),
            (
                "resources/subscribe",
                Some(&serde_json::json!({ "uri": resource_uri })),
            ),
            (
                "resources/unsubscribe",
                Some(&serde_json::json!({ "uri": resource_uri })),
            ),
        ],
    );

    assert_eq!(output.results.len(), 3);
    // Subscribe should succeed
    assert!(
        output.results[1].get("error").is_none(),
        "Subscribe should succeed for template-matched resource"
    );
    // Unsubscribe should succeed
    assert!(
        output.results[2].get("error").is_none(),
        "Unsubscribe should succeed for template-matched resource"
    );
}
