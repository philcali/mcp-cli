//! Prompt endpoint tests

use std::fs;
use tempfile::TempDir;

use crate::common::run_request_sequence;

fn setup_test_prompts() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Simple text prompt
    fs::write(
        temp_dir.path().join("greeting.json"),
        r#"{"name": "greeting", "messages": [{"role": "user", "content": "Hello, {{name}}!"}]}"#,
    )
    .unwrap();

    // Prompt with multiple variables
    fs::write(
        temp_dir.path().join("intro.json"),
        r#"{"name": "intro", "messages": [{"role": "user", "content": "Name: {{name}}, Age: {{age}}, City: {{city}}"}]}"#,
    )
    .unwrap();

    // Multi-line prompt
    fs::write(
        temp_dir.path().join("email.json"),
        r#"{"name": "email", "messages": [{"role": "user", "content": "Subject: Meeting Reminder\n\nDear {{name}},\n\nThe meeting is scheduled for {{date}}.\n\nBest regards,\n{{sender}}"}]}"#,
    )
    .unwrap();

    // Simple prompt with no variables
    fs::write(
        temp_dir.path().join("simple.json"),
        r#"{"name": "simple", "messages": [{"role": "user", "content": "This is a simple message."}]}"#,
    )
    .unwrap();

    temp_dir
}

#[test]
fn test_prompts_list_with_directory() {
    let temp_dir = setup_test_prompts();

    // Run both requests in same process with prompts directory
    let results = run_request_sequence(
        None,
        Some(temp_dir.path().to_path_buf()),
        vec![
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
            ("prompts/list", None),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[0].get("result").is_some(),
        "Expected successful initialize"
    );

    let result = results[1]["result"].as_object().unwrap();
    let prompts = result["prompts"].as_array().unwrap();

    assert_eq!(prompts.len(), 4, "Should discover all 4 prompt files");

    let names: Vec<&str> = prompts
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();

    assert!(names.contains(&"greeting"), "Should include greeting");
    assert!(names.contains(&"intro"), "Should include intro");
    assert!(names.contains(&"email"), "Should include email");
}

#[test]
fn test_prompts_get_with_variables() {
    let temp_dir = setup_test_prompts();

    let params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let get_params = serde_json::json!({
        "name": "greeting",
        "arguments": {
            "name": "Alice"
        }
    });

    // Use sequence to run both requests in same process
    let results = run_request_sequence(
        None,
        Some(temp_dir.path().to_path_buf()),
        vec![
            ("initialize", Some(&params)),
            ("prompts/get", Some(&get_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("result").is_some(),
        "Expected result for prompts/get"
    );

    let result = &results[1]["result"];
    let messages = result["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);

    let content = messages[0]["content"].as_str().unwrap();
    assert_eq!(content, "Hello, Alice!");
}

#[test]
fn test_prompts_get_with_missing_argument() {
    let temp_dir = setup_test_prompts();

    let params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    // Request without required argument
    let get_params = serde_json::json!({
        "name": "greeting",
        "arguments": {}
    });

    let results = run_request_sequence(
        None,
        Some(temp_dir.path().to_path_buf()),
        vec![
            ("initialize", Some(&params)),
            ("prompts/get", Some(&get_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    // Missing argument should be kept as literal {{name}}
    let result = &results[1]["result"];
    let messages = result["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);

    let content = messages[0]["content"].as_str().unwrap();
    assert_eq!(content, "Hello, {{name}}!"); // Literal substitution when no arg provided
}

#[test]
fn test_prompts_get_with_defaults() {
    let temp_dir = setup_test_prompts();

    let params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    // Request without any arguments - should keep literals for missing args
    let get_params = serde_json::json!({
        "name": "greeting",
        "arguments": {}
    });

    let results = run_request_sequence(
        None,
        Some(temp_dir.path().to_path_buf()),
        vec![
            ("initialize", Some(&params)),
            ("prompts/get", Some(&get_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    let result = &results[1]["result"];
    let messages = result["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);

    // Missing arguments are kept as literal {{name}}
    let content = messages[0]["content"].as_str().unwrap();
    assert_eq!(content, "Hello, {{name}}!");
}

#[test]
fn test_prompts_get_not_found() {
    let params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let get_params = serde_json::json!({
        "name": "nonexistent-prompt",
        "arguments": {}
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&params)),
            ("prompts/get", Some(&get_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "Expected error for non-existent prompt"
    );
}

#[test]
fn test_prompts_list_before_initialize() {
    let response = crate::common::run_request_with_dirs("prompts/list", None, 1, None, None);

    assert!(
        response.get("error").is_some(),
        "Expected error before initialize"
    );
}

#[test]
fn test_prompts_get_before_initialize() {
    let get_params = serde_json::json!({
        "name": "some-prompt",
        "arguments": {}
    });

    let response =
        crate::common::run_request_with_dirs("prompts/get", Some(&get_params), 2, None, None);

    assert!(
        response.get("error").is_some(),
        "Expected error before initialize"
    );
}

#[test]
fn test_prompts_list_capability() {
    let temp_dir = setup_test_prompts();

    let params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    // Run initialize with prompts directory
    let results = run_request_sequence(
        None,
        Some(temp_dir.path().to_path_buf()),
        vec![("initialize", Some(&params)), ("prompts/list", None)],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[0].get("result").is_some(),
        "Expected successful initialize"
    );

    // Verify we can list prompts
    let result = &results[1]["result"];
    let prompts = result["prompts"].as_array().unwrap();
    assert!(!prompts.is_empty(), "Should have at least one prompt");
}
