//! MCP protocol types and JSON-RPC message definitions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Base JSON-RPC 2.0 request structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(rename = "id")]
    pub id_value: serde_json::Value,
}

/// Base JSON-RPC 2.0 response structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<serde_json::Value>,
    #[serde(rename = "error")]
    pub error_value: Option<JsonRpcError>,
    #[serde(rename = "id")]
    pub id_value: serde_json::Value,
}

/// JSON-RPC 2.0 error structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    pub fn parse_error(msg: &str) -> Self {
        Self {
            code: -32700,
            message: msg.to_string(),
            data: None,
        }
    }

    pub fn invalid_params(msg: &str) -> Self {
        Self {
            code: -32602,
            message: msg.to_string(),
            data: None,
        }
    }

    pub fn method_not_found() -> Self {
        Self {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        }
    }

    pub fn internal_error(msg: &str) -> Self {
        Self {
            code: -32603,
            message: msg.to_string(),
            data: None,
        }
    }
}

/// Initialize request parameters from client.
#[derive(Debug, Deserialize, Default)]
pub struct InitParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(default)]
    pub capabilities: ClientCapabilities,
    #[serde(default, rename = "clientInfo")]
    pub client_info: Implementation,
}

/// Initialize result sent to client.
#[derive(Debug, Serialize)]
pub struct InitResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: Implementation,
}

/// Client capabilities object.
#[derive(Debug, Deserialize, Default)]
pub struct ClientCapabilities {
    #[serde(default)]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub roots: Option<RootsCapability>,
    #[serde(default)]
    pub sampling: Option<HashMap<String, serde_json::Value>>,
}

/// Roots capability from client.
#[derive(Debug, Deserialize)]
pub struct RootsCapability {
    pub list_changed: Option<bool>,
}

/// Server capabilities object.
#[derive(Debug, Clone, Serialize, Default)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<bool>,
}

impl ServerCapabilities {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tools(mut self) -> Self {
        self.tools = Some(true);
        self
    }

    pub fn with_resources(mut self, list_changed: bool) -> Self {
        self.resources = Some(ResourcesCapability { list_changed });
        self
    }

    pub fn with_prompts(mut self) -> Self {
        self.prompts = Some(true);
        self
    }

    pub fn with_logging(mut self) -> Self {
        self.logging = Some(true);
        self
    }
}

/// Resources capability.
#[derive(Debug, Clone, Serialize)]
pub struct ResourcesCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

/// Implementation info (client or server).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

/// Tool structure.
#[derive(Debug, Clone, Serialize)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}

impl Tool {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: Some(description.to_string()),
            input_schema: None,
        }
    }

    pub fn with_schema(mut self, schema: serde_json::Value) -> Self {
        self.input_schema = Some(schema);
        self
    }
}

/// Tool call request parameters.
#[derive(Debug, Deserialize)]
pub struct CallToolParams {
    pub name: String,
    #[serde(default)]
    pub arguments: HashMap<String, serde_json::Value>,
}

/// Tool result structure.
#[derive(Debug, Serialize)]
pub struct CallToolResult {
    pub content: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl CallToolResult {
    pub fn success(text: &str) -> Self {
        Self {
            content: vec![Content::text(text)],
            is_error: Some(false),
        }
    }

    pub fn error(text: &str) -> Self {
        Self {
            content: vec![Content::text(text)],
            is_error: Some(true),
        }
    }
}

/// Content type for tool results.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Text { text: String },
    #[serde(rename_all = "camelCase")]
    Image {
        data: String,
        mime_type: String,
    },
}

impl Content {
    pub fn text(text: &str) -> Self {
        Self::Text {
            text: text.to_string(),
        }
    }

    pub fn image(data: &str, mime_type: &str) -> Self {
        Self::Image {
            data: data.to_string(),
            mime_type: mime_type.to_string(),
        }
    }
}

/// List tools request parameters.
#[derive(Debug, Deserialize)]
pub struct ListToolsParams;

impl Default for ListToolsParams {
    fn default() -> Self {
        Self
    }
}

/// Tool list item.
#[derive(Debug, Clone, Serialize)]
pub struct ToolListItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}

impl From<Tool> for ToolListItem {
    fn from(tool: Tool) -> Self {
        Self {
            name: tool.name,
            description: tool.description,
            input_schema: tool.input_schema,
        }
    }
}

/// Resource structure.
#[derive(Debug, Clone, Serialize)]
pub struct Resource {
    pub uri: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Resource template structure.
#[derive(Debug, Clone, Serialize)]
pub struct ResourceTemplate {
    pub uri_template: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Resource content.
#[derive(Debug, Clone, Serialize)]
pub struct TextResourceContents {
    #[serde(rename = "uri")]
    pub uri_value: String,
    #[serde(rename = "text")]
    pub text_value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Resource read result.
#[derive(Debug, Serialize)]
pub struct ReadResourceResult {
    pub contents: Vec<ResourceContents>,
}

impl ReadResourceResult {
    pub fn text(uri: &str, text: &str) -> Self {
        Self {
            contents: vec![ResourceContents::text(uri, text)],
        }
    }
}

/// Resource content item.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResourceContents {
    Text(TextResourceContents),
    #[serde(rename_all = "camelCase")]
    Blob {
        uri: String,
        data: String,
        mime_type: String,
    },
}

impl ResourceContents {
    pub fn text(uri: &str, text: &str) -> Self {
        Self::Text(TextResourceContents {
            uri_value: uri.to_string(),
            text_value: text.to_string(),
            mime_type: None,
        })
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn blob(uri: &str, data: String, mime_type: &str) -> Self {
        Self::Blob {
            uri: uri.to_string(),
            data,
            mime_type: mime_type.to_string(),
        }
    }
}

/// Prompt structure.
#[derive(Debug, Clone, Serialize)]
pub struct Prompt {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub arguments: Option<Vec<PromptArgument>>,
}

/// Prompt argument.
#[derive(Debug, Clone, Serialize)]
pub struct PromptArgument {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}
