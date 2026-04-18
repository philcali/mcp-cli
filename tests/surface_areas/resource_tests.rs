//! Resource endpoint tests

use std::fs;
use tempfile::TempDir;

use crate::common::run_request_sequence_with_resources;
use mcp_cli::discovery::resources::discover_resources;

fn setup_test_resources() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    fs::write(temp_dir.path().join("hello.txt"), "Hello, World!").unwrap();
    fs::write(
        temp_dir.path().join("config.json"),
        r#"{"key": "value", "number": 42}"#,
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("readme.md"),
        "# Test Resource\nThis is a test.",
    )
    .unwrap();

    temp_dir
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

    let _response = run_request_sequence_with_resources(
        temp_dir.path().to_path_buf(),
        vec![("initialize", Some(&params))],
    );

    // Now list resources from the temp directory
    let response = crate::common::run_request_with_dirs(
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

    assert_eq!(resources.len(), 3, "Should discover all 3 resource files");

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
    assert!(
        results[1].get("result").is_some(),
        "Expected result for resources/read"
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
    assert!(
        results[1].get("result").is_some(),
        "Expected result for resources/read"
    );

    let result = results[1]["result"].as_object().unwrap();
    let contents = result["contents"].as_array().unwrap();
    assert_eq!(contents.len(), 1);

    let content = &contents[0];
    assert_eq!(content["mimeType"], "application/json");
}

#[test]
fn test_resources_read_not_found() {
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

    let results = crate::common::run_request_sequence(
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
fn test_resources_subscribe_valid_resource() {
    let temp_dir = setup_test_resources();

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
        "Expected successful subscribe"
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

    let results = crate::common::run_request_sequence(
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
        "Expected successful subscribe"
    );
    assert!(
        results[2].get("result").is_some(),
        "Expected successful unsubscribe"
    );
}

#[test]
fn test_resources_subscribe_and_read() {
    let temp_dir = setup_test_resources();

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
        "Expected successful subscribe"
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

// ===========================================================================
// MIME TYPE EXTENSION TESTS
// ===========================================================================

#[test]
fn test_mime_type_pdf() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("document.pdf"), "PDF content").unwrap();

    let resources = discover_resources(temp_dir.path()).unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].mime_type, Some("application/pdf".to_string()));
}

#[test]
fn test_mime_type_images() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("image.png"), "PNG content").unwrap();
    fs::write(temp_dir.path().join("photo.jpg"), "JPG content").unwrap();
    fs::write(temp_dir.path().join("animation.gif"), "GIF content").unwrap();
    fs::write(temp_dir.path().join("graphic.webp"), "WebP content").unwrap();

    let resources = discover_resources(temp_dir.path()).unwrap();

    let png_mime = resources
        .iter()
        .find(|r| r.uri.contains("image.png"))
        .unwrap()
        .mime_type
        .clone();
    let jpg_mime = resources
        .iter()
        .find(|r| r.uri.contains("photo.jpg"))
        .unwrap()
        .mime_type
        .clone();
    let gif_mime = resources
        .iter()
        .find(|r| r.uri.contains("animation.gif"))
        .unwrap()
        .mime_type
        .clone();

    assert_eq!(png_mime, Some("image/png".to_string()));
    assert_eq!(jpg_mime, Some("image/jpeg".to_string()));
    assert_eq!(gif_mime, Some("image/gif".to_string()));
}

#[test]
fn test_mime_type_fonts() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("font.woff"), "WOFF content").unwrap();
    fs::write(temp_dir.path().join("font.ttf"), "TTF content").unwrap();

    let resources = discover_resources(temp_dir.path()).unwrap();

    let woff_mime = resources
        .iter()
        .find(|r| r.uri.contains("font.woff"))
        .unwrap()
        .mime_type
        .clone();
    let ttf_mime = resources
        .iter()
        .find(|r| r.uri.contains("font.ttf"))
        .unwrap()
        .mime_type
        .clone();

    assert_eq!(woff_mime, Some("font/woff".to_string()));
    assert_eq!(ttf_mime, Some("font/ttf".to_string()));
}

#[test]
fn test_mime_type_archives() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("archive.zip"), "ZIP content").unwrap();
    fs::write(temp_dir.path().join("data.tar.gz"), "TARGZ content").unwrap();

    let resources = discover_resources(temp_dir.path()).unwrap();

    let zip_mime = resources
        .iter()
        .find(|r| r.uri.contains("archive.zip"))
        .unwrap()
        .mime_type
        .clone();

    assert_eq!(zip_mime, Some("application/zip".to_string()));
}
