//! MCP CLI - Model Context Protocol server implementation.
//!
//! This is a minimal MCP server that communicates via stdio using JSON-RPC 2.0.

use anyhow::Result;
use clap::Parser;
use tracing::{Level, info};
use tracing_subscriber::fmt::format::FmtSpan;

pub mod protocol;
pub mod server;

/// Model Context Protocol CLI server
#[derive(Parser, Debug)]
#[command(name = "mcp-cli", about = "MCP server with stdio transport")]
struct Cli {
    /// Directory path for tools (executable files)
    #[arg(long, short)]
    tools_dir: Option<std::path::PathBuf>,

    /// Directory path for resources
    #[arg(long, short)]
    resources_dir: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr (never stdout - that's the protocol stream!)
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_span_events(FmtSpan::NONE)
        .with_target(false)
        .without_time()
        .init();

    let cli = Cli::parse();
    let mut builder = server::ServerBuilder::new("mcp-cli", "0.1.0")
        .with_tools()
        .with_resources(false);

    if let Some(tools_dir) = cli.tools_dir {
        info!("Using tools directory: {:?}", tools_dir);
        builder = builder.with_tools_dir(tools_dir);
    }

    if let Some(resources_dir) = cli.resources_dir {
        info!("Using resources directory: {:?}", resources_dir);
        builder = builder.with_resources_dir(resources_dir);
    }

    let mut srv = builder.build();
    info!("MCP server starting...");
    srv.run().await?;

    Ok(())
}
