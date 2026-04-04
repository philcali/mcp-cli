//! MCP CLI - Model Context Protocol server implementation.
//!
//! This is a minimal MCP server that communicates via stdio using JSON-RPC 2.0.

pub mod protocol;
pub mod server;

use anyhow::Result;
use tracing::{Level, info};
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

    // Parse command line arguments for tools directory
    let args: Vec<String> = std::env::args().collect();
    let mut builder = server::ServerBuilder::new("mcp-cli", "0.1.0").with_tools();

    if args.len() > 1 {
        let tools_dir = std::path::PathBuf::from(&args[1]);
        info!("Using tools directory: {:?}", tools_dir);
        builder = builder.with_tools_dir(tools_dir);
    }

    let mut srv = builder.build();
    info!("MCP server starting...");
    srv.run().await?;

    Ok(())
}
