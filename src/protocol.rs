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
    /// Root directories provided by the client. These are paths the server can access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roots: Option<Vec<Root>>,
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

/// Server-side roots capability - indicates server can list client root directories.
#[derive(Debug, Clone, Serialize)]
pub struct RootsCapabilityServer {
    /// Whether the server supports listing root directories provided by the client.
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
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
    pub roots: Option<RootsCapabilityServer>,
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

    /// Enable roots capability (client root directories for file access).
    pub fn with_roots(mut self) -> Self {
        self.roots = Some(RootsCapabilityServer {
            list_changed: false,
        });
        self
    }
}

/// Client-provided root directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    /// URI of the root directory (e.g., file:///path/to/root).
    #[serde(rename = "uri")]
    pub uri: String,
    /// Optional name for the root.
    #[serde(skip_serializing_if = "Option::is_none", rename = "name")]
    pub name: Option<String>,
}

impl Root {
    pub fn new(uri: &str) -> Self {
        Self {
            uri: uri.to_string(),
            name: None,
        }
    }

    pub fn with_name(uri: &str, name: &str) -> Self {
        Self {
            uri: uri.to_string(),
            name: Some(name.to_string()),
        }
    }
}

/// Resource read result.
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
    Text {
        text: String,
    },
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
#[derive(Default)]
pub struct ListToolsParams {
    #[serde(default)]
    pub tool_names: Option<Vec<String>>,
}


/// Tool list result.
#[derive(Debug, Serialize)]
pub struct ListToolsResult {
    pub tools: Vec<ToolListItem>,
}

/// Tool list update notification.
#[derive(Debug, Serialize)]
pub struct ToolsListChangedNotification {
    #[serde(rename = "method")]
    pub method_value: String,
    #[serde(rename = "jsonrpc")]
    pub jsonrpc_version: String,
}

impl Default for ToolsListChangedNotification {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolsListChangedNotification {
    pub fn new() -> Self {
        Self {
            method_value: "tools/listChanged".to_string(),
            jsonrpc_version: "2.0".to_string(),
        }
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// Authentication strategy for a tool.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStrategy {
    /// Environment variable injection (e.g., GITHUB_TOKEN)
    EnvVar,
    /// OAuth2 flow with token caching - EXPERIMENTAL
    #[serde(rename = "oauth2")]
    OAuth2,
    /// API key passed as custom header - EXPERIMENTAL
    #[serde(rename = "api_key_header")]
    ApiKeyHeader,
    /// Bearer token in Authorization header - EXPERIMENTAL
    #[serde(rename = "bearer_token")]
    BearerToken,
}

/// OAuth2 configuration for a tool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OAuthConfig {
    pub client_id_env: String,
    pub token_url: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// Authentication configuration for a tool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolAuthConfig {
    /// The authentication strategy used by this tool
    #[serde(default = "default_strategy")]
    pub strategy: AuthStrategy,
    /// Environment variables required for authentication
    #[serde(default)]
    pub required_env_vars: Vec<String>,
    /// OAuth2 configuration (only used if strategy is oauth2)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_config: Option<OAuthConfig>,
}

fn default_strategy() -> AuthStrategy {
    AuthStrategy::EnvVar // Default to simple env var injection
}

/// Load tool auth config from a file path.
pub fn load_tool_auth_config(path: &std::path::Path) -> anyhow::Result<Option<ToolAuthConfig>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(path)?;
    let config: ToolAuthConfig = serde_json::from_str(&content)?;
    Ok(Some(config))
}

/// Load tool auth config from JSON string.
pub fn parse_tool_auth_config(json: &str) -> anyhow::Result<ToolAuthConfig> {
    let config: ToolAuthConfig = serde_json::from_str(json)?;
    Ok(config)
}

// ===========================================================================
// PROMPT SUPPORT
// ===========================================================================

/// Prompt template file structure.
#[derive(Debug, Clone, Deserialize)]
pub struct PromptFile {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
    #[serde(default)]
    pub messages: Option<Vec<PromptFileMessage>>,
}

impl PromptFile {
    /// Convert to internal representation with PromptMessage.
    pub fn to_messages(&self) -> Vec<crate::protocol::PromptMessage> {
        self.messages
            .iter()
            .flatten()
            .map(|msg| crate::protocol::PromptMessage {
                role: msg.role.clone(),
                content_value: msg.content.clone(),
            })
            .collect()
    }
}

/// Get prompt request parameters.
#[derive(Debug, Deserialize)]
pub struct GetPromptParams {
    pub name: String,
    #[serde(default)]
    pub arguments: HashMap<String, serde_json::Value>,
}

/// Prompt message role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// Prompt message structure.
#[derive(Debug, Clone, Serialize)]
pub struct PromptMessage {
    pub role: MessageRole,
    #[serde(rename = "content")]
    pub content_value: PromptMessageContentValue,
}

impl PromptMessage {
    pub fn new(role: MessageRole, content: PromptMessageContentValue) -> Self {
        Self {
            role,
            content_value: content,
        }
    }
}

/// Internal value type for prompt message content (text or array).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PromptMessageContentValue {
    Text(String),
    Array(Vec<PromptMessageContentItem>),
}

impl PromptMessageContentValue {
    pub fn text(s: &str) -> Self {
        Self::Text(s.to_string())
    }

    /// Convert to string (for rendering). Returns None for array content.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            PromptMessageContentValue::Text(s) => Some(s),
            PromptMessageContentValue::Array(_) => None,
        }
    }

    /// Check if content is an array.
    pub fn is_array(&self) -> bool {
        matches!(self, PromptMessageContentValue::Array(_))
    }
}

/// Prompt message structure for deserialization from files.
#[derive(Debug, Clone, Deserialize)]
pub struct PromptFileMessage {
    pub role: MessageRole,
    pub content: PromptMessageContentValue,
}

/// Content item for structured prompt messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PromptMessageContentItem {
    Text {
        text: String,
    },
    #[serde(rename = "image_url")]
    ImageUrl {
        image_url: ImageUrlData,
    },
}

/// Image URL data structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrlData {
    pub url: String,
}

/// Result of getting a prompt.
#[derive(Debug, Clone, Serialize)]
pub struct GetPromptResult {
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
}

impl GetPromptResult {
    pub fn new(description: Option<String>, messages: Vec<PromptMessage>) -> Self {
        Self {
            description,
            messages,
        }
    }
}

/// Simple template engine for prompt rendering.
#[derive(Debug, Default)]
pub struct PromptTemplateEngine;

impl PromptTemplateEngine {
    /// Create a new template engine.
    pub fn new() -> Self {
        Self
    }

    /// Render a template with the given arguments.
    /// Supports: {{var}}, {{#include path}}, {{#env VAR}}
    pub fn render(
        &self,
        template: &str,
        args: &HashMap<String, serde_json::Value>,
        base_dir: Option<&std::path::Path>,
    ) -> Result<String, PromptRenderError> {
        let mut result = String::new();
        let chars: Vec<char> = template.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            if chars[i] == '{' && i + 1 < len && chars[i + 1] == '#' {
                // Directive: {{#...}}
                let (directive, end_i) = self.parse_directive(&chars, i)?;
                result.push_str(&self.execute_directive(directive, args, base_dir)?);
                i = end_i;
            } else if chars[i] == '{' && i + 1 < len && chars[i + 1] == '{' {
                // Variable: {{var}}
                let (var_name, end_i) = self.parse_variable(&chars, i)?;
                let value = self.resolve_variable(&var_name, args);
                result.push_str(&value);
                i = end_i;
            } else {
                // Regular character
                result.push(chars[i]);
                i += 1;
            }
        }

        Ok(result)
    }

    /// Parse a directive ({{#include ...}} or {{#env VAR}}).
    fn parse_directive(
        &self,
        chars: &[char],
        start: usize,
    ) -> Result<(String, usize), PromptRenderError> {
        let mut i = start + 2; // Skip {{#
        while i < chars.len() && (chars[i] == '}' || !chars[i].is_whitespace()) {
            i += 1;
        }

        let _directive_end = i;
        while i < chars.len() && chars[i] != '}' {
            i += 1;
        }

        if i >= chars.len() {
            return Err(PromptRenderError::UnclosedDirective);
        }

        let content: String = chars[start + 2..i].iter().collect();
        Ok((content, i + 1))
    }

    /// Parse a variable reference ({{var}}).
    fn parse_variable(
        &self,
        chars: &[char],
        start: usize,
    ) -> Result<(String, usize), PromptRenderError> {
        let mut i = start + 2; // Skip {{
        while i < chars.len() && chars[i] != '}' {
            i += 1;
        }

        if i >= chars.len() {
            return Err(PromptRenderError::UnclosedVariable);
        }

        let name: String = chars[start + 2..i].iter().collect();
        Ok((name, i + 1))
    }

    /// Execute a directive.
    fn execute_directive(
        &self,
        directive: String,
        _args: &HashMap<String, serde_json::Value>,
        base_dir: Option<&std::path::Path>,
    ) -> Result<String, PromptRenderError> {
        let parts: Vec<&str> = directive.split_whitespace().collect();
        if parts.is_empty() {
            return Err(PromptRenderError::InvalidDirective(directive));
        }

        match parts[0] {
            "include" => {
                let path_str = parts
                    .get(1)
                    .ok_or_else(|| PromptRenderError::MissingArgument("include".to_string()))?;
                let base = base_dir.unwrap_or(std::path::Path::new("."));
                let full_path = base.join(path_str);
                std::fs::read_to_string(&full_path).map_err(|e| PromptRenderError::FileReadError {
                    path: path_str.to_string(),
                    error: e.to_string(),
                })
            }
            "env" => {
                let var_name = parts
                    .get(1)
                    .ok_or_else(|| PromptRenderError::MissingArgument("env".to_string()))?;
                std::env::var(var_name)
                    .map_err(|_| PromptRenderError::EnvVarNotFound(var_name.to_string()))
            }
            _ => Err(PromptRenderError::UnknownDirective(parts[0].to_string())),
        }
    }

    /// Resolve a variable from arguments.
    fn resolve_variable(&self, name: &str, args: &HashMap<String, serde_json::Value>) -> String {
        match args.get(name) {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => format!("{{{{{}}}}}", name), // Keep as literal if not found
        }
    }
}

/// Error type for template rendering.
#[derive(Debug, Clone)]
pub enum PromptRenderError {
    UnclosedDirective,
    UnclosedVariable,
    InvalidDirective(String),
    UnknownDirective(String),
    MissingArgument(String),
    FileReadError { path: String, error: String },
    EnvVarNotFound(String),
}

impl std::fmt::Display for PromptRenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PromptRenderError::UnclosedDirective => write!(f, "Unclosed directive"),
            PromptRenderError::UnclosedVariable => write!(f, "Unclosed variable"),
            PromptRenderError::InvalidDirective(d) => write!(f, "Invalid directive: {}", d),
            PromptRenderError::UnknownDirective(d) => write!(f, "Unknown directive: {}", d),
            PromptRenderError::MissingArgument(d) => {
                write!(f, "Missing argument for directive: {}", d)
            }
            PromptRenderError::FileReadError { path, error } => {
                write!(f, "Failed to read file '{}': {}", path, error)
            }
            PromptRenderError::EnvVarNotFound(var) => {
                write!(f, "Environment variable '{}' not found", var)
            }
        }
    }
}

impl std::error::Error for PromptRenderError {}

/// Validate prompt arguments against required parameters.
pub fn validate_prompt_arguments(
    args: &HashMap<String, serde_json::Value>,
    required_args: &[PromptArgument],
) -> Result<(), String> {
    let required_names: Vec<&str> = required_args
        .iter()
        .filter(|a| a.required == Some(true))
        .map(|a| a.name.as_str())
        .collect();

    for name in required_names {
        if !args.contains_key(name) {
            return Err(format!("Missing required argument: {}", name));
        }
    }

    Ok(())
}
