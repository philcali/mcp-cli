//! Common test utilities

use std::io::{BufRead, Write};
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
        .stderr(Stdio::inherit())
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
            stdin.flush().unwrap();

            if let Some(ref mut stdout) = child.stdout {
                let line = std::io::BufReader::new(stdout)
                    .lines()
                    .map_while(|l| l.ok())
                    .find(|line| line.trim_start().starts_with('{'));

                if let Some(line) = line {
                    results.push(serde_json::from_str(&line).expect("Failed to parse response"));
                }
            }
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

    let _output = child.wait_with_output();
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
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn mcp-cli daemon");

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
            stdin.flush().unwrap();

            if let Some(ref mut stdout) = child.stdout {
                let line = std::io::BufReader::new(stdout)
                    .lines()
                    .map_while(|l| l.ok())
                    .find(|line| line.trim_start().starts_with('{'));

                if let Some(line) = line {
                    results.push(serde_json::from_str(&line).expect("Failed to parse response"));
                }
            }
        }
    }

    #[cfg(unix)]
    unsafe {
        libc::kill(child.id() as i32, SIGTERM);
    }

    let output = child.wait_with_output().unwrap();
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
