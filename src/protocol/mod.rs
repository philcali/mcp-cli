//! MCP protocol types and JSON-RPC message definitions.
//!
//! This module is organized into:
//! - `types`: Pure data type definitions (structs, enums, traits)
//! - `prompt_engine`: Prompt template engine and argument validation
//! - `auth`: Authentication configuration helpers (business logic)

pub mod auth;
pub mod prompt_engine;
pub mod types;

// Re-export types
pub use types::*;

// Re-export business logic items
pub use prompt_engine::{PromptRenderError, PromptTemplateEngine, validate_prompt_arguments};

// Re-export auth helpers
pub use auth::{load_tool_auth_config, parse_tool_auth_config};

// Protocol version negotiation
pub use types::{InitError, JsonRpcError, negotiate_protocol_version, protocol_error_to_json_rpc};
