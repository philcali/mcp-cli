//! Elicitation integration tests

use crate::common::run_request_sequence;

fn init_params_without_elicitation() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    })
}

fn init_params_with_elicitation() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "elicitation": {
                "form": true,
                "url": true
            }
        },
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    })
}

fn init_params_elicitation_form_only() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "elicitation": {
                "form": true,
                "url": false
            }
        },
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    })
}

/// Test elicitation/create fails when client doesn't support elicitation
#[test]
fn test_elicitation_without_client_capability() {
    let init_params = init_params_without_elicitation();

    let elicitation_params = serde_json::json!({
        "mode": "form",
        "message": "Please provide your name"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("elicitation/create", Some(&elicitation_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "elicitation should fail when client doesn't support it"
    );
}

/// Test elicitation/create fails before initialize
#[test]
fn test_elicitation_before_initialize() {
    let elicitation_params = serde_json::json!({
        "mode": "form",
        "message": "Please provide input"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![("elicitation/create", Some(&elicitation_params))],
    );

    assert_eq!(results.len(), 1);
    assert!(
        results[0].get("error").is_some(),
        "elicitation before initialize should error"
    );
}

/// Test elicitation/create with url mode missing URL
#[test]
fn test_elicitation_url_mode_missing_url() {
    let init_params = init_params_with_elicitation();

    let elicitation_params = serde_json::json!({
        "mode": "url",
        "message": "Please complete this form"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("elicitation/create", Some(&elicitation_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "url mode without URL should error"
    );
}

/// Test elicitation/create with url mode missing elicitation_id
#[test]
fn test_elicitation_url_mode_missing_id() {
    let init_params = init_params_with_elicitation();

    let elicitation_params = serde_json::json!({
        "mode": "url",
        "message": "Please complete this form",
        "url": "https://example.com/form"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("elicitation/create", Some(&elicitation_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "url mode without elicitationId should error"
    );
}

/// Test elicitation/create with url mode when client doesn't support url
#[test]
fn test_elicitation_url_mode_not_supported() {
    let init_params = init_params_elicitation_form_only();

    let elicitation_params = serde_json::json!({
        "mode": "url",
        "message": "Please complete this form",
        "url": "https://example.com/form",
        "elicitationId": "test-123"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("elicitation/create", Some(&elicitation_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "url mode should fail when client doesn't support it"
    );
}

/// Test elicitation/create with missing message parameter
#[test]
fn test_elicitation_missing_message() {
    let init_params = init_params_with_elicitation();

    let elicitation_params = serde_json::json!({
        "mode": "form"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("elicitation/create", Some(&elicitation_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "elicitation without message should error"
    );
}

/// Test elicitation/create with no params at all
#[test]
fn test_elicitation_no_params() {
    let init_params = init_params_with_elicitation();

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("elicitation/create", None),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "elicitation without params should error"
    );
}
