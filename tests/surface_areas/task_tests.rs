//! Task lifecycle integration tests

use crate::common::run_request_sequence;

fn init_params_with_tasks() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tasks": {
                "list": true,
                "cancel": true,
                "requests": {
                    "tools": {
                        "call": true
                    }
                }
            }
        },
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    })
}

/// Test that tasks/list returns empty before any tasks are created
#[test]
fn test_tasks_list_empty() {
    let init_params = init_params_with_tasks();

    let results = run_request_sequence(
        None,
        None,
        vec![("initialize", Some(&init_params)), ("tasks/list", None)],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[0].get("result").is_some(),
        "initialize should succeed"
    );

    let list_result = &results[1];
    assert!(
        list_result.get("result").is_some(),
        "tasks/list should succeed"
    );
    assert!(
        list_result["result"]["tasks"].is_array(),
        "tasks should be an array"
    );
    assert_eq!(
        list_result["result"]["tasks"].as_array().unwrap().len(),
        0,
        "no tasks should exist yet"
    );
}

/// Test that tasks/get returns error for nonexistent task
#[test]
fn test_tasks_get_nonexistent() {
    let init_params = init_params_with_tasks();

    let get_params = serde_json::json!({
        "task_id": "nonexistent_task"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("tasks/get", Some(&get_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "tasks/get for nonexistent task should error"
    );
}

/// Test that tasks/cancel returns error for nonexistent task
#[test]
fn test_tasks_cancel_nonexistent() {
    let init_params = init_params_with_tasks();

    let cancel_params = serde_json::json!({
        "task_id": "nonexistent_task"
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("tasks/cancel", Some(&cancel_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "tasks/cancel for nonexistent task should error"
    );
}

/// Test that tasks/result returns error for nonexistent task
#[test]
fn test_tasks_result_nonexistent() {
    let init_params = init_params_with_tasks();

    let result_params = serde_json::json!({
        "task_id": "nonexistent_task",
        "timeout": 5
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("tasks/result", Some(&result_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "tasks/result for nonexistent task should error"
    );
}

/// Test that unknown method returns error
#[test]
fn test_unknown_task_method() {
    let init_params = init_params_with_tasks();

    let results = run_request_sequence(
        None,
        None,
        vec![("initialize", Some(&init_params)), ("tasks/unknown", None)],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "unknown task method should error"
    );
}

/// Test tasks/list with empty params object
#[test]
fn test_tasks_list_with_empty_params() {
    let init_params = init_params_with_tasks();

    let list_params = serde_json::json!({});

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("tasks/list", Some(&list_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("result").is_some(),
        "tasks/list with empty params should succeed"
    );
}

/// Test tasks/list with state filter
#[test]
fn test_tasks_list_with_state_filter() {
    let init_params = init_params_with_tasks();

    let list_params = serde_json::json!({
        "states": ["completed", "failed"]
    });

    let results = run_request_sequence(
        None,
        None,
        vec![
            ("initialize", Some(&init_params)),
            ("tasks/list", Some(&list_params)),
        ],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("result").is_some(),
        "tasks/list with state filter should succeed"
    );
}

/// Test that tasks endpoints require initialization
#[test]
fn test_tasks_before_initialize() {
    let results = run_request_sequence(None, None, vec![("tasks/list", None)]);

    assert_eq!(results.len(), 1);
    assert!(
        results[0].get("error").is_some(),
        "tasks/list before initialize should error"
    );
}

/// Test that tasks/get requires valid task_id parameter
#[test]
fn test_tasks_get_missing_params() {
    let init_params = init_params_with_tasks();

    let results = run_request_sequence(
        None,
        None,
        vec![("initialize", Some(&init_params)), ("tasks/get", None)],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "tasks/get without params should error"
    );
}

/// Test that tasks/cancel requires valid task_id parameter
#[test]
fn test_tasks_cancel_missing_params() {
    let init_params = init_params_with_tasks();

    let results = run_request_sequence(
        None,
        None,
        vec![("initialize", Some(&init_params)), ("tasks/cancel", None)],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "tasks/cancel without params should error"
    );
}

/// Test that tasks/result requires valid parameters
#[test]
fn test_tasks_result_missing_params() {
    let init_params = init_params_with_tasks();

    let results = run_request_sequence(
        None,
        None,
        vec![("initialize", Some(&init_params)), ("tasks/result", None)],
    );

    assert_eq!(results.len(), 2);
    assert!(
        results[1].get("error").is_some(),
        "tasks/result without params should error"
    );
}
