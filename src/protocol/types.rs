//! MCP protocol type definitions.
//!
//! This module contains all pure data types for the MCP protocol,
//! including JSON-RPC messages, capability structures, and domain-specific types.

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

// ===========================================================================
// JSON-RPC MESSAGE TYPES
// ===========================================================================

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

/// Base JSON-RPC 2.0 notification structure (no id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
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

// ===========================================================================
// INITIALIZATION
// ===========================================================================

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
#[serde(rename_all = "camelCase")]
pub struct InitResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: Implementation,
}

/// Client capabilities object.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ClientCapabilities {
    #[serde(default)]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub roots: Option<RootsCapability>,
    #[serde(default)]
    pub sampling: Option<SamplingCapability>,
}

/// Roots capability from client.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
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

/// Tools capability - indicates server supports tool listing and calling.
#[derive(Debug, Clone, Serialize, Default)]
pub struct ToolsCapability {
    /// Whether the server sends notifications when its tool list changes.
    #[serde(skip_serializing_if = "Option::is_none", rename = "listChanged")]
    pub list_changed: Option<bool>,
}

/// Prompts capability - indicates server supports prompt listing and getting.
#[derive(Debug, Clone, Serialize, Default)]
pub struct PromptsCapability {
    /// Whether the server sends notifications when its prompt list changes.
    #[serde(skip_serializing_if = "Option::is_none", rename = "listChanged")]
    pub list_changed: Option<bool>,
}

/// Sampling capability - indicates server supports LLM sampling.
#[derive(Debug, Clone, Serialize, Default, Deserialize)]
pub struct SamplingCapability {
    #[serde(skip_serializing_if = "Option::is_none", rename = "listChanged")]
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
    pub prompts: Option<PromptsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<RootsCapabilityServer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<SamplingCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<TasksCapability>,
}

impl ServerCapabilities {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tools(mut self) -> Self {
        self.tools = Some(ToolsCapability {
            list_changed: Some(true),
        });
        self
    }

    pub fn with_resources(mut self, list_changed: bool) -> Self {
        self.resources = Some(ResourcesCapability {
            list_changed,
            template_list_changed: None,
        });
        self
    }

    /// Enable resource templates capability (server supports resource templates).
    pub fn with_resource_templates(mut self) -> Self {
        match &mut self.resources {
            Some(cap) => {
                cap.list_changed = true;
                cap.template_list_changed = Some(true);
            }
            None => {
                self.resources = Some(ResourcesCapability {
                    list_changed: true,
                    template_list_changed: Some(true),
                });
            }
        }
        self
    }

    pub fn with_prompts(mut self) -> Self {
        self.prompts = Some(PromptsCapability {
            list_changed: Some(true),
        });
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

    /// Enable sampling capability (server can ask client to call LLM).
    pub fn with_sampling(mut self) -> Self {
        self.sampling = Some(SamplingCapability { list_changed: None });
        self
    }

    /// Enable tasks capability (server supports task-augmented requests).
    pub fn with_tasks(mut self) -> Self {
        self.tasks = Some(TasksCapability {
            list: Some(true),
            cancel: Some(true),
            requests: Some(TasksRequestsCapability {
                tools: Some(TasksToolsRequestsCapability { call: Some(true) }),
            }),
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

/// Resource capability structure.
#[derive(Debug, Clone, Serialize)]
pub struct ResourcesCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "templateListChanged"
    )]
    pub template_list_changed: Option<bool>,
}

/// Implementation info (client or server).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

// ===========================================================================
// TOOLS
// ===========================================================================

/// Tool structure.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
}

impl Tool {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: Some(description.to_string()),
            input_schema: Some(json!({ "type": "object" })),
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
    /// Opt-in flag for streaming results
    #[serde(default, rename = "stream")]
    pub stream: bool,
    /// When present, this call should be executed as a task
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskAugmentation>,
}

impl CallToolParams {
    /// Check if streaming was requested
    pub fn is_streaming(&self) -> bool {
        self.stream
    }
}

/// Tool result structure.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
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

// ===========================================================================
// STREAMING SUPPORT
// ===========================================================================

/// Stream chunk types for streaming tool/resource output.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamChunk {
    /// Initial metadata about the stream
    #[serde(rename_all = "camelCase")]
    Meta {
        #[allow(dead_code)]
        chunk_count: i64,
        total_bytes: Option<usize>,
    },
    /// A chunk of content (text output)
    #[serde(rename_all = "camelCase")]
    Content {
        data: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    /// Final marker indicating stream completion
    Done { summary: Option<String> },
}

/// Tool call result with optional streaming ID.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingCallResult {
    pub content: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    /// Stream identifier for receiving chunks
    pub stream_id: String,
}

/// Read resource request parameters with optional streaming.
#[derive(Debug, Deserialize)]
pub struct ReadResourceParams {
    pub uri: String,
    /// Opt-in flag for streaming results
    #[serde(default, rename = "stream")]
    pub stream: bool,
}

impl ReadResourceParams {
    /// Check if streaming was requested
    pub fn is_streaming(&self) -> bool {
        self.stream
    }
}

/// List tools request parameters.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct ToolListItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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

// ===========================================================================
// RESOURCES
// ===========================================================================

/// Resource structure.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplate {
    pub uri_template: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Resource content.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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

// ===========================================================================
// RESOURCE SUBSCRIPTION SUPPORT
// ===========================================================================

/// Subscribe to resource change notifications.
#[derive(Debug, Deserialize)]
pub struct SubscribeResourceParams {
    /// URI of the resource to subscribe to.
    pub uri: String,
}

/// Result of subscribing to a resource.
#[derive(Debug, Serialize, Default)]
pub struct SubscribeResourceResult {
    // MCP spec doesn't define any result fields for this method
    #[serde(skip_serializing)]
    _empty: (),
}

/// Unsubscribe from resource change notifications.
#[derive(Debug, Deserialize)]
pub struct UnsubscribeResourceParams {
    /// URI of the resource to unsubscribe from.
    pub uri: String,
}

/// Result of unsubscribing from a resource.
#[derive(Debug, Serialize, Default)]
pub struct UnsubscribeResourceResult {
    // MCP spec doesn't define any result fields for this method
    #[serde(skip_serializing)]
    _empty: (),
}

/// Resource list changed notification.
#[derive(Debug, Default)]
pub struct ResourcesListChangedNotification;

impl ResourcesListChangedNotification {
    pub const METHOD_NAME: &'static str = "resources/listChanged";

    /// Convert to JSON-RPC notification format.
    pub fn to_jsonrpc(&self) -> serde_json::Value {
        json!({
            "jsonrpc": "2.0",
            "method": Self::METHOD_NAME,
        })
    }
}

// ===========================================================================
// STREAMING NOTIFICATIONS
// ===========================================================================

/// Streaming output chunk notification from server to client.
#[derive(Debug, Serialize)]
pub struct StreamChunkNotification {
    #[serde(rename = "method")]
    pub method_value: String,
    #[serde(rename = "jsonrpc")]
    pub jsonrpc_version: String,
    pub params: StreamChunkParams,
}

/// Parameters for streaming chunk notifications.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunkParams {
    pub request_id: String,
    pub chunk: StreamChunk,
}

impl StreamChunkNotification {
    /// Create a content chunk notification
    pub fn content(request_id: &str, data: &str) -> Self {
        Self {
            method_value: "tools/stream".to_string(),
            jsonrpc_version: "2.0".to_string(),
            params: StreamChunkParams {
                request_id: request_id.to_string(),
                chunk: StreamChunk::Content {
                    data: data.to_string(),
                    is_error: None,
                },
            },
        }
    }

    /// Create a meta/chunk count notification
    pub fn meta(request_id: &str, total_bytes: Option<usize>) -> Self {
        Self {
            method_value: "tools/stream".to_string(),
            jsonrpc_version: "2.0".to_string(),
            params: StreamChunkParams {
                request_id: request_id.to_string(),
                chunk: StreamChunk::Meta {
                    chunk_count: -1, // unknown for streaming
                    total_bytes,
                },
            },
        }
    }

    /// Create a done notification
    pub fn done(request_id: &str) -> Self {
        Self {
            method_value: "tools/stream".to_string(),
            jsonrpc_version: "2.0".to_string(),
            params: StreamChunkParams {
                request_id: request_id.to_string(),
                chunk: StreamChunk::Done { summary: None },
            },
        }
    }
}

/// Resource subscription info.
#[derive(Debug, Clone)]
pub struct SubscriptionInfo {
    /// URI being subscribed to.
    pub uri: String,
}

impl From<&str> for SubscriptionInfo {
    fn from(uri: &str) -> Self {
        Self {
            uri: uri.to_string(),
        }
    }
}

/// Resource subscription management trait.
pub trait ResourceManager {
    /// Subscribe to a resource URI. Returns true if newly subscribed.
    fn subscribe(&self, uri: &str) -> bool;

    /// Unsubscribe from a resource URI. Returns true if was subscribed.
    fn unsubscribe(&self, uri: &str) -> bool;

    /// Check if a resource is currently subscribed.
    fn is_subscribed(&self, uri: &str) -> bool;

    /// Get list of all subscribed URIs.
    fn get_subscriptions(&self) -> Vec<String>;
}

/// In-memory subscription manager for testing and simple use cases.
pub struct MemorySubscriptionManager {
    subscriptions: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl MemorySubscriptionManager {
    pub fn new() -> Self {
        Self {
            subscriptions: std::sync::Mutex::new(std::collections::HashSet::new()),
        }
    }
}

impl Default for MemorySubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceManager for MemorySubscriptionManager {
    fn subscribe(&self, uri: &str) -> bool {
        self.subscriptions.lock().unwrap().insert(uri.to_string())
    }

    fn unsubscribe(&self, uri: &str) -> bool {
        self.subscriptions.lock().unwrap().remove(uri)
    }

    fn is_subscribed(&self, uri: &str) -> bool {
        self.subscriptions.lock().unwrap().contains(uri)
    }

    fn get_subscriptions(&self) -> Vec<String> {
        self.subscriptions
            .lock()
            .unwrap()
            .clone()
            .into_iter()
            .collect()
    }
}

// ===========================================================================
// PROMPT SUPPORT
// ===========================================================================

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
    pub fn to_messages(&self) -> Vec<PromptMessage> {
        self.messages
            .iter()
            .flatten()
            .map(|msg| PromptMessage {
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

// ===========================================================================
// COMPLETION SUPPORT
// ===========================================================================

/// Reference to a completion location (tool, prompt, or resource).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompletionReference {
    #[serde(rename = "type")]
    pub ref_type: String,
    pub value: String,
}

/// Completion request parameters.
#[derive(Debug, Deserialize)]
pub struct CompleteParams {
    #[serde(rename = "ref")]
    pub ref_: CompletionReference,
    pub argument: CompleteArgument,
}

/// The argument being completed.
#[derive(Debug, Deserialize)]
pub struct CompleteArgument {
    pub name: String,
    pub value: String,
}

/// Completion result.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteResult {
    pub values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}

// ===========================================================================
// LOGGING SUPPORT
// ===========================================================================

/// Log message level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
    Alert,
    Emergency,
}

/// Log message request parameters from client.
#[derive(Debug, Deserialize)]
pub struct LogMessageParams {
    /// Message level
    pub level: LogLevel,
    /// Logger name (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logger: Option<String>,
    /// The log message content
    pub message: String,
}

/// Result of logging a message.
#[derive(Debug, Serialize, Default)]
pub struct LogMessageResult {
    // MCP spec doesn't define any result fields for this method
    #[serde(skip_serializing)]
    _empty: (),
}

// ===========================================================================
// SAMPLING SUPPORT
// ===========================================================================

/// Content for sampling messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SamplingContent {
    Text {
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    Image {
        data: String,
        mime_type: String,
    },
}

/// A message for sampling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingMessage {
    pub role: MessageRole,
    #[serde(rename = "content")]
    pub content_value: SamplingContent,
}

/// Parameters for createMessage sampling request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageParams {
    pub messages: Vec<SamplingMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Result of createMessage sampling.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageResult {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    pub role: MessageRole,
    #[serde(rename = "content")]
    pub content_value: SamplingContent,
}

// ===========================================================================
// AUTH CONFIGURATION
// ===========================================================================

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

// ===========================================================================
// PROTOCOL VERSION SUPPORT
// ===========================================================================

/// Supported MCP protocol versions (in order of preference).
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2024-11-05", "2024-10-07"];

/// Check if a version is compatible by comparing year prefix against supported versions.
/// Accepts any version whose year matches a supported year or is within one year ahead
/// of the latest supported year (forward compatibility).
fn is_compatible_version(version: &str) -> bool {
    let client_year = match version
        .split('-')
        .next()
        .and_then(|y| y.parse::<u16>().ok())
    {
        Some(y) => y,
        None => return false,
    };

    let max_supported_year = SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .filter_map(|s| s.split('-').next())
        .filter_map(|y| y.parse::<u16>().ok())
        .max();

    match max_supported_year {
        Some(max_year) => {
            // Match exact year or allow one year ahead for forward compat
            client_year == max_year || client_year == max_year + 1
        }
        None => false,
    }
}

/// Negotiate protocol version with client.
/// Returns the version to use if negotiation succeeds.
pub fn negotiate_protocol_version(client_version: &str) -> VersionNegotiationResult {
    let client_version = client_version.trim();

    // Check for exact matches first (in order of preference)
    for supported in SUPPORTED_PROTOCOL_VERSIONS {
        if client_version == *supported {
            return VersionNegotiationResult::Supported(client_version.to_string());
        }
    }

    // Allow any version whose year matches a supported version
    if is_compatible_version(client_version) {
        return VersionNegotiationResult::Compatible(client_version.to_string());
    }

    // Version is not supported
    VersionNegotiationResult::Unsupported {
        received: client_version.to_string(),
        supported: SUPPORTED_PROTOCOL_VERSIONS
            .iter()
            .map(|s| s.to_string())
            .collect(),
    }
}

/// Protocol version negotiation result.
#[derive(Debug, Clone, PartialEq)]
pub enum VersionNegotiationResult {
    /// Version is supported and will be used
    Supported(String),
    /// Version year matches a supported version but is not an exact match
    Compatible(String),
    /// Version year does not match any supported version
    Unsupported {
        received: String,
        supported: Vec<String>,
    },
}

/// Error type for protocol initialization failures.
#[derive(Debug, Clone)]
pub enum InitError {
    /// Protocol version is not supported
    UnsupportedProtocol {
        received: String,
        supported: Vec<String>,
    },
    /// Failed to parse initialize parameters
    InvalidParams(String),
    /// Server configuration error
    Configuration(String),
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitError::UnsupportedProtocol {
                received,
                supported,
            } => {
                write!(
                    f,
                    "Unsupported protocol version '{}'. Supported versions: {}",
                    received,
                    supported.join(", ")
                )
            }
            InitError::InvalidParams(msg) => write!(f, "Invalid initialize parameters: {}", msg),
            InitError::Configuration(msg) => write!(f, "Server configuration error: {}", msg),
        }
    }
}

// ===========================================================================
// TASKS SUPPORT
// ===========================================================================

/// Task execution state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Working,
    InputRequired,
    Completed,
    Failed,
    Cancelled,
}

impl TaskState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        )
    }
}

/// Tool-level task support level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TaskSupportLevel {
    Forbidden,
    Optional,
    Required,
}

/// Task augmentation for request parameters.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAugmentation {
    /// TTL in milliseconds since creation after which task may be deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,
}

/// Tasks capability for server declaration.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TasksCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "requests")]
    pub requests: Option<TasksRequestsCapability>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TasksRequestsCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<TasksToolsRequestsCapability>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TasksToolsRequestsCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call: Option<bool>,
}

/// A task representing the execution state of a request.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    #[serde(rename = "taskId")]
    pub task_id: String,
    pub state: TaskState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "lastUpdatedAt")]
    pub last_updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "pollInterval")]
    pub poll_interval: Option<u64>,
}

/// Result when a task is created (returned immediately by augmented request).
#[derive(Debug, Serialize)]
pub struct CreateTaskResult {
    pub task: Task,
}

/// Parameters for tasks/get.
#[derive(Debug, Deserialize)]
pub struct GetTaskParams {
    #[serde(rename = "taskId")]
    pub task_id: String,
}

/// Result of tasks/get.
#[derive(Debug, Serialize)]
pub struct GetTaskResult {
    pub task: Task,
}

/// Parameters for tasks/list.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListTasksParams {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub states: Option<Vec<TaskState>>,
}

/// Result of tasks/list.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTasksResult {
    pub tasks: Vec<Task>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Parameters for tasks/result.
#[derive(Debug, Deserialize)]
pub struct TaskResultParams {
    #[serde(rename = "taskId")]
    pub task_id: String,
    /// Timeout in seconds for blocking on terminal state.
    #[serde(default = "default_task_result_timeout")]
    pub timeout: u64,
}

fn default_task_result_timeout() -> u64 {
    300
}

/// Parameters for tasks/cancel.
#[derive(Debug, Deserialize)]
pub struct CancelTaskParams {
    #[serde(rename = "taskId")]
    pub task_id: String,
}

/// Stored result when a task reaches terminal state.
#[derive(Debug, Clone, Serialize)]
pub struct TaskResult {
    pub result: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
}

impl std::error::Error for InitError {}

/// Create an appropriate JSON-RPC error for protocol errors.
pub fn protocol_error_to_json_rpc(error: &InitError) -> JsonRpcError {
    match error {
        InitError::UnsupportedProtocol { .. } => JsonRpcError::invalid_params(&error.to_string()),
        InitError::InvalidParams(msg) => JsonRpcError::invalid_params(msg),
        InitError::Configuration(msg) => JsonRpcError::internal_error(msg),
    }
}
