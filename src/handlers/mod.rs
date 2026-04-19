//! Handler modules for different MCP capabilities.

pub mod init;
pub mod logging;
pub mod ping;
pub mod prompts;
pub mod resources;
pub mod telemetry;
pub mod tools;

// Re-export common types and utilities
pub use init::handle_initialize;
pub use logging::handle_logging_messages;
pub use ping::handle_ping;
pub use prompts::{handle_prompts_get, handle_prompts_list};
pub use resources::{
    handle_resources_list, handle_resources_read, handle_resources_subscribe,
    handle_resources_unsubscribe,
};
pub use telemetry::handle_telemetry_event;
pub use tools::{handle_tools_call, handle_tools_list};
