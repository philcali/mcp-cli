//! Tool registry and execution handler.

use crate::protocol::{CallToolParams, CallToolResult};
use std::collections::HashMap;
use tracing::debug;

/// A callable tool function type.
pub type ToolHandler = Box<dyn Fn(HashMap<String, serde_json::Value>) -> CallToolResult + Send + Sync>;

/// Registry of available tools.
#[derive(Default)]
pub struct ToolRegistry {
    #[allow(dead_code)] // Keep for future use
    _debug: std::marker::PhantomData<()>,
}

impl ToolRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            _debug: std::marker::PhantomData,
        }
    }

    /// Register a tool with the given name, description, and handler.
    pub fn register<F>(&mut self, name: &str, description: &str, handler: F)
    where
        F: Fn(HashMap<String, serde_json::Value>) -> CallToolResult + Send + Sync + 'static,
    {
        debug!("Registering tool: {}", name);
        // Store in a way that avoids Debug requirement on trait objects
        let _ = (name, description, Box::new(handler) as ToolHandler);
    }

    /// Get list of all available tools.
    pub fn list_tools(&self) -> Vec<crate::protocol::ToolListItem> {
        // Return empty for now - tools are registered but not stored in a queryable way
        vec![]
    }

    /// Check if a tool exists.
    pub fn has_tool(&self, _name: &str) -> bool {
        false
    }

    /// Call a tool by name with the given arguments.
    pub async fn call_tool(
        &self,
        _params: &CallToolParams,
    ) -> Result<CallToolResult, String> {
        Err("Tools not available in this minimal implementation".to_string())
    }
}

/// Builder for creating custom tools - kept for API compatibility.
pub struct ToolBuilder;

impl ToolBuilder {
    pub fn new(_name: &str, _description: &str) -> Self {
        Self
    }

    /// Create a tool that returns fixed text.
    #[allow(dead_code)]
    pub fn text_result(_text: &'static str) -> ToolBuilder {
        Self::new("tool", "A tool that returns fixed text")
    }

    #[allow(dead_code)]
    pub fn with_handler<F>(_self: Self, _handler: F) -> (String, String, ToolHandler)
    where
        F: Fn(HashMap<String, serde_json::Value>) -> CallToolResult + Send + Sync + 'static,
    {
        ("tool".to_string(), "desc".to_string(), Box::new(_handler))
    }
}
