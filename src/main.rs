//! MCP CLI - Model Context Protocol server implementation.
//!
//! This is a minimal MCP server that communicates via stdio using JSON-RPC 2.0.

pub mod protocol;
pub mod server;

use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::fmt::format::FmtSpan;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr (never stdout - that's the protocol stream!)
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_span_events(FmtSpan::NONE)
        .with_target(false)
        .without_time()
        .init();

    let mut srv = server::McpServer::new("mcp-cli", "0.1.0").enable_tools();

    info!("MCP server starting...");
    srv.run().await?;

    Ok(())
}
