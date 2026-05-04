//! Handler modules for different MCP capabilities.

pub mod completion;
pub mod elicitation;
pub mod init;
pub mod logging;
pub mod ping;
pub mod prompts;
pub mod resources;
pub mod roots;
pub mod sampling;
pub mod tasks;
pub mod telemetry;
pub mod tools;

// Re-export common types and utilities
pub use completion::handle_completion_complete;
pub use elicitation::handle_elicitation_create;
pub use init::handle_initialize;
pub use logging::{handle_logging_messages, handle_logging_set_level};
pub use ping::handle_ping;
pub use prompts::{handle_prompts_get, handle_prompts_list};
pub use resources::{
    handle_resource_templates_list, handle_resources_list, handle_resources_read,
    handle_resources_subscribe, handle_resources_unsubscribe,
};
pub use roots::handle_roots_list;
pub use sampling::handle_sampling_create_message;
pub use tasks::{handle_tasks_cancel, handle_tasks_get, handle_tasks_list, handle_tasks_result};
pub use telemetry::handle_telemetry_event;
pub use tools::{handle_tools_call, handle_tools_list};
