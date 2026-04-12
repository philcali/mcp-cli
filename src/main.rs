//! MCP CLI - Model Context Protocol server implementation.
//!
//! This is a minimal MCP server that communicates via stdio using JSON-RPC 2.0.

use anyhow::Result;
use clap::Parser;
use tracing::{Level, info, warn};
use tracing_subscriber::fmt::format::FmtSpan;

pub mod protocol;
pub mod server;
pub mod watcher;

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

    /// Directory path for prompts
    #[arg(long, short)]
    prompts_dir: Option<std::path::PathBuf>,
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

    let tools_dir = cli.tools_dir.clone();
    let resources_dir = cli.resources_dir.clone();
    let prompts_dir = cli.prompts_dir.clone();

    if let Some(ref td) = tools_dir {
        info!("Using tools directory: {:?}", td);
        builder = builder.with_tools_dir(td.clone());
    }

    if let Some(ref rd) = resources_dir {
        info!("Using resources directory: {:?}", rd);
        builder = builder.with_resources_dir(rd.clone());
    }

    if let Some(ref pd) = prompts_dir {
        info!("Using prompts directory: {:?}", pd);
        builder = builder.with_prompts();
        builder = builder.with_prompts_dir(pd.clone());
    }

    let mut srv = builder.build();

    // Start watchers if directories are configured
    if tools_dir.is_some() {
        match srv.start_tool_watcher() {
            Ok(handle) => {
                info!("Started tool watcher");
                std::mem::forget(handle); // Keep handle alive for lifetime of process
            }
            Err(e) => warn!("Failed to start tool watcher: {}", e),
        }
    }

    if prompts_dir.is_some() {
        match srv.start_prompt_watcher() {
            Ok(handle) => {
                info!("Started prompt watcher");
                std::mem::forget(handle); // Keep handle alive for lifetime of process
            }
            Err(e) => warn!("Failed to start prompt watcher: {}", e),
        }
    }

    info!("MCP server starting...");
    srv.run().await?;

    Ok(())
}
