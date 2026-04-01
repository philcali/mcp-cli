//! MCP server implementation with stdio transport.

use crate::protocol::*;
use anyhow::{Context, Result};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info};

/// Server state and configuration.
pub struct McpServer {
    name: String,
    version: String,
    capabilities: ServerCapabilities,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new("mcp-cli", "0.1.0")
    }
}

impl McpServer {
    /// Create a new MCP server with the given name and version.
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            capabilities: ServerCapabilities::new(),
        }
    }

    /// Enable tools capability.
    pub fn enable_tools(self) -> Self {
        let mut s = self;
        s.capabilities.tools = Some(true);
        s
    }

    /// Run the server loop on stdio transport.
    pub async fn run(&mut self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();

        info!("MCP server starting, waiting for messages...");

        let mut initialized = false;

        loop {
            match reader.next_line().await {
                Ok(Some(line)) => {
                    if line.trim().is_empty() {
                        continue;
                    }

                    debug!("Received message: {}", line);

                    // Parse and respond to the request
                    match self.handle_request(&line, initialized).await {
                        Ok(response) => {
                            let _ = tokio::io::stdout().write_all(format!("{}\n", response).as_bytes()).await;
                            let _ = tokio::io::stdout().flush().await;

                            // After successful initialize, mark as initialized
                            if !initialized && response.contains("capabilities") {
                                initialized = true;
                            }
                        }
                        Err(e) => {
                            error!("Error processing message: {}", e);
                            let err_resp = json!({
                                "jsonrpc": "2.0",
                                "error": JsonRpcError::internal_error(&e.to_string()),
                                "id": null,
                            });
                            let _ = tokio::io::stdout().write_all(format!("{}\n", serde_json::to_string(&err_resp).unwrap()).as_bytes()).await;
                        }
                    }

                    // Update initialized state based on response
                    if !initialized && self.capabilities.tools == Some(true) {
                        initialized = true;
                    }
                }
                Ok(None) => {
                    // EOF reached
                    break;
                }
                Err(e) => {
                    error!("Read error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle a request and return the response.
    async fn handle_request(&self, line: &str, initialized: bool) -> Result<String> {
        let request: JsonRpcRequest = serde_json::from_str(line)?;

        debug!(
            "Processing {} with id={}",
            request.method,
            match &request.id_value {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => "null".to_string(),
            }
        );

        let result = self.route_request(&request.method, &request.params, initialized).await;

        Ok(match result {
            Ok(resp) => json!({
                "jsonrpc": "2.0",
                "result": resp,
                "id": request.id_value,
            }),
            Err(e) => {
                let err_resp = JsonRpcError::internal_error(&e.to_string());
                json!({
                    "jsonrpc": "2.0",
                    "error": err_resp,
                    "id": request.id_value,
                })
            }
        }.to_string())
    }

    /// Route requests to appropriate handlers.
    async fn route_request(&self, method: &str, params: &serde_json::Value, initialized: bool) -> Result<serde_json::Value> {
        match method {
            "initialize" => self.handle_initialize(params).await,
            "resources/list" => Ok(json!({ "resources": [] })),
            _ if !initialized => Err(anyhow::anyhow!("Server not initialized")),
            "initialized" => Ok(json!({})),
            "ping" => Ok(json!({})),
            "tools/list" => self.handle_tools_list().await,
            "tools/call" => Err(anyhow::anyhow!("Tools not implemented")),
            "resources/read" => Err(anyhow::anyhow!("Resources not implemented")),
            _ => Err(anyhow::anyhow!("Unknown method: {}", method)),
        }
    }

    /// Handle initialize request.
    async fn handle_initialize(&self, params: &serde_json::Value) -> Result<serde_json::Value> {
        let init_params: InitParams = serde_json::from_value(params.clone())
            .context("Failed to parse initialize parameters")?;

        info!(
            "Received initialize request from {}: {}",
            init_params.client_info.name, init_params.protocol_version
        );

        // Validate protocol version (simplified check)
        if !init_params.protocol_version.starts_with("2024-") {
            return Err(anyhow::anyhow!("Unsupported protocol version"));
        }

        let result = InitResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: self.capabilities.clone(),
            server_info: Implementation {
                name: self.name.clone(),
                version: self.version.clone(),
            },
        };

        Ok(serde_json::to_value(result)?)
    }

    /// Handle tools/list request.
    async fn handle_tools_list(&self) -> Result<serde_json::Value> {
        Ok(json!({ "tools": [] }))
    }
}

/// Server with tools capability enabled (for builder pattern).
pub struct McpServerWithTools {
    name: String,
    version: String,
    capabilities: ServerCapabilities,
}

impl McpServerWithTools {
    pub fn run(self) -> McpServer {
        McpServer {
            name: self.name,
            version: self.version,
            capabilities: self.capabilities,
        }
    }
}
