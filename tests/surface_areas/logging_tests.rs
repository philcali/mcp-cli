//! Logging endpoint tests

use crate::common::run_request_sequence;

#[test]
fn test_logging_messages_rejected_as_incoming() {
    // Per MCP spec, logging/messages is a server-to-client notification, not an incoming request.
    // Sending it as a request should return an "Unknown method" error.
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
        results[1].get("error").is_some(),
        "logging/messages should be rejected as an unknown method (it is server-to-client only)"
    );
}

#[test]
fn test_logging_set_level() {
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let set_level_params = serde_json::json!({
        "level": "debug"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("logging/setLevel", Some(&set_level_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("result").is_some(),
        "setLevel should succeed after initialize"
    );
}

#[test]
fn test_logging_set_level_with_error_level() {
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let set_level_params = serde_json::json!({
        "level": "error"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("logging/setLevel", Some(&set_level_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("result").is_some(),
        "setLevel with error level should succeed"
    );
}

#[test]
fn test_logging_set_level_missing_level_field() {
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let set_level_params = serde_json::json!({});

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("logging/setLevel", Some(&set_level_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "setLevel without level field should return an error"
    );
}

#[test]
fn test_logging_set_level_invalid_level() {
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });

    let set_level_params = serde_json::json!({
        "level": "invalid-level"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("logging/setLevel", Some(&set_level_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "setLevel with invalid level should return an error"
    );
}
