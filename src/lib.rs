//! MCP CLI Library
//!
//! This crate provides a minimal implementation of the Model Context Protocol (MCP) server.
//!
//! # Example
//!
//! ```rust,ignore
//! use mcp_cli::server::McpServer;
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut server = McpServer::new("my-server", "1.0.0");
//!     // Register tools, run server...
//! }
//! ```

pub mod protocol;
pub mod server;
pub mod watcher;

// Note: tools module uses serde_json but is not exposed here
