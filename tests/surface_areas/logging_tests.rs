//! Logging endpoint tests

use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

/// Run a single request to the mcp-cli server.
fn run_request(method: &str, params: Option<&serde_json::Value>, id: i64) -> serde_json::Value {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));

    let child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn mcp-cli");

    send_request_and_read_response(child, method, params, id)
}

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

use crate::common::run_request_sequence;

#[test]
fn test_logging_messages_before_initialize() {
    let response = run_request("logging/messages", None, 1);

    assert!(
        response.get("error").is_some(),
        "Expected error before initialize"
    );
}

#[test]
fn test_logging_messages_with_info_level() {
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let log_params = serde_json::json!({
        "level": "info",
        "logger": "test-logger",
        "message": "This is a test info message"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("logging/messages", Some(&log_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("result").is_some(),
        "Expected result for logging/messages"
    );
}

#[test]
fn test_logging_messages_with_debug_level() {
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let log_params = serde_json::json!({
        "level": "debug",
        "message": "This is a debug message without logger"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("logging/messages", Some(&log_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("result").is_some(),
        "Expected result for logging/messages with debug level"
    );
}

#[test]
fn test_logging_messages_with_error_level() {
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let log_params = serde_json::json!({
        "level": "error",
        "logger": "error-logger",
        "message": "This is an error message"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("logging/messages", Some(&log_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("result").is_some(),
        "Expected result for logging/messages with error level"
    );
}

#[test]
fn test_logging_messages_with_unknown_level() {
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let log_params = serde_json::json!({
        "level": "unknown-level",
        "message": "Message with unknown level"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("logging/messages", Some(&log_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("result").is_some(),
        "Expected result for logging/messages with unknown level"
    );
}

#[test]
fn test_logging_messages_with_capabilities() {
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
        vec![
            ("initialize", Some(&init_params)),
            (
                "logging/messages",
                Some(&serde_json::json!({
                    "level": "info",
                    "message": "Test message after init with logging capability"
                })),
            ),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[0].get("result").is_some(),
        "Expected successful initialize"
    );
    assert!(
        results[1].get("result").is_some(),
        "Expected result for logging/messages when server has logging capability"
    );
}
