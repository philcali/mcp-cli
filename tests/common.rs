//! Common test utilities

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[cfg(unix)]
use libc::SIGTERM;

/// Spawn the MCP server with optional resources and prompts directories.
pub fn run_request_with_dirs(
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
        .stderr(Stdio::piped())
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

    // Wait for the process to exit, then read stdout.
    // This avoids the race where we read stdout while the server is still
    // running and pick up logging lines before the actual response.
    let output = child.wait_with_output().expect("Failed to wait on child");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    for line in stdout.lines() {
        if line.trim_start().starts_with('{')
            && let Ok(result) = serde_json::from_str::<serde_json::Value>(line)
        {
            return result;
        }
    }

    serde_json::Value::Null
}

/// Spawn server and send multiple requests.
pub fn run_request_sequence(
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
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn mcp-cli");

    // Send all requests
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
        }
        // Close stdin -> EOF -> server exits
    }

    // Wait for the process to exit, then parse all stdout at once.
    // This avoids the race where we read stdout while the server is still
    // running and pick up logging lines before the actual response.
    let output = child.wait_with_output().expect("Failed to wait on child");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    let mut results: Vec<serde_json::Value> = Vec::new();
    for line in stdout.lines() {
        if line.trim_start().starts_with('{')
            && let Ok(result) = serde_json::from_str::<serde_json::Value>(line)
        {
            results.push(result);
        }
    }

    results
}

/// Run requests in daemon mode (persistent server).
pub fn run_request_sequence_daemon(
    requests: Vec<(&str, Option<&serde_json::Value>)>,
) -> Vec<serde_json::Value> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));
    cmd.arg("--daemon");

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn mcp-cli daemon");

    // Send all requests
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
        }
    }

    #[cfg(unix)]
    unsafe {
        libc::kill(child.id() as i32, SIGTERM);
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    let mut results: Vec<serde_json::Value> = Vec::new();
    for line in stdout.lines() {
        if line.trim_start().starts_with('{')
            && let Ok(result) = serde_json::from_str::<serde_json::Value>(line)
        {
            results.push(result);
        }
    }

    tracing::info!("Daemon process exited with status: {:?}", output.status);

    results
}

/// Wrapper for prompts tests with prompts_dir only.
pub fn run_request_sequence_with_prompts(
    prompts_dir: PathBuf,
    requests: Vec<(&str, Option<&serde_json::Value>)>,
) -> Vec<serde_json::Value> {
    run_request_sequence(None, Some(prompts_dir), requests)
}

/// Wrapper for resources tests with resources_dir only.
pub fn run_request_sequence_with_resources(
    resources_dir: PathBuf,
    requests: Vec<(&str, Option<&serde_json::Value>)>,
) -> Vec<serde_json::Value> {
    run_request_sequence(Some(resources_dir), None, requests)
}

/// Result of running requests with stderr capture.
pub struct RequestOutput {
    pub results: Vec<serde_json::Value>,
    pub stderr: String,
}

/// Spawn the MCP server with tools/resources/prompts dirs, set env vars, send requests,
/// and capture both stdout results and stderr.
pub fn run_request_sequence_all(
    tools_dir: Option<PathBuf>,
    resources_dir: Option<PathBuf>,
    prompts_dir: Option<PathBuf>,
    resource_templates_dir: Option<PathBuf>,
    env_vars: Vec<(&str, &str)>,
    requests: Vec<(&str, Option<&serde_json::Value>)>,
) -> RequestOutput {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mcp-cli"));

    if let Some(ref dir) = tools_dir {
        cmd.arg("--tools-dir").arg(dir.to_str().unwrap());
    }
    if let Some(ref dir) = resources_dir {
        cmd.arg("--resources-dir").arg(dir.to_str().unwrap());
    }
    if let Some(ref dir) = prompts_dir {
        cmd.arg("--prompts-dir").arg(dir.to_str().unwrap());
    }
    if let Some(ref dir) = resource_templates_dir {
        cmd.arg("--resource-templates-dir")
            .arg(dir.to_str().unwrap());
    }

    // Set environment variables
    for (k, v) in env_vars {
        cmd.env(k, v);
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn mcp-cli");

    // Send all requests
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
        }
        // Close stdin -> EOF -> server exits (one-shot mode)
    }

    // Wait for the process to exit, then parse all stdout at once.
    // This avoids the race where we read stdout while the server is still
    // running and pick up logging lines before the actual response.
    let output = child.wait_with_output().expect("Failed to wait on child");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let mut results: Vec<serde_json::Value> = Vec::new();
    for line in stdout.lines() {
        if line.trim_start().starts_with('{')
            && let Ok(result) = serde_json::from_str::<serde_json::Value>(line)
        {
            results.push(result);
        }
    }

    RequestOutput { results, stderr }
}
