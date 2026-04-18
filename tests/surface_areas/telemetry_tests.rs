//! Telemetry endpoint tests

use crate::common::run_request_sequence;

#[test]
fn test_telemetry_event() {
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
                "telemetry/event",
                Some(&serde_json::json!({
                    "eventName": "test.event",
                    "data": {
                        "key": "value"
                    }
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
        "Expected result for telemetry/event"
    );
}
