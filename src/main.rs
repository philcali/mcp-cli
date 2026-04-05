//! MCP CLI - Model Context Protocol server implementation.
//!
//! This is a minimal MCP server that communicates via stdio using JSON-RPC 2.0.

pub mod protocol;
pub mod server;

use anyhow::Result;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
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

    // Parse command line arguments for tools and resources directories
    let args: Vec<String> = std::env::args().collect();
    let mut builder = server::ServerBuilder::new("mcp-cli", "0.1.0")
        .with_tools()
        .with_resources(false);

    // Support --tools-dir flag with explicit path following it
    if args.len() > 1 && args[1] == "--tools-dir" && args.len() > 2 {
        let tools_dir = std::path::PathBuf::from(&args[2]);
        info!("Using tools directory: {:?}", tools_dir);
        builder = builder.with_tools_dir(tools_dir);
    }

    // Support --resources-dir flag with explicit path following it
    if args.len() > 1 && args[1] == "--resources-dir" && args.len() > 2 {
        let resources_dir = std::path::PathBuf::from(&args[2]);
        info!("Using resources directory: {:?}", resources_dir);
        builder = builder.with_resources_dir(resources_dir);
    }

    // If no flags, first positional arg is tools dir, second is resources dir
    if args.len() > 1 && !args[1].starts_with("--") {
        let arg1 = &args[1];
        info!("First argument: {}", arg1);

        // Check if it's executable (tools dir) or not (resources dir)
        #[cfg(unix)]
        let is_executable = std::path::Path::new(arg1).exists() && {
            match std::fs::metadata(arg1) {
                Ok(m) => m.permissions().mode() & 0o111 != 0,
                Err(_) => false,
            }
        };
        #[cfg(not(unix))]
        let is_executable = true;

        if is_executable {
            let tools_dir = std::path::PathBuf::from(arg1);
            info!("Using tools directory: {:?}", tools_dir);
            builder = builder.with_tools_dir(tools_dir);

            // Second arg is resources dir
            if args.len() > 2 {
                let resources_dir = std::path::PathBuf::from(&args[2]);
                info!("Using resources directory: {:?}", resources_dir);
                builder = builder.with_resources_dir(resources_dir);
            }
        } else {
            // Not executable, treat as resources dir (tests)
            let resources_dir = std::path::PathBuf::from(arg1);
            info!("Using resources directory: {:?}", resources_dir);
            builder = builder.with_resources_dir(resources_dir);

            // Second arg is also resources dir
            if args.len() > 2 {
                let extra_resources = std::path::PathBuf::from(&args[2]);
                info!(
                    "Using additional resources directory: {:?}",
                    extra_resources
                );
                builder = builder.with_resources_dir(extra_resources);
            }
        }
    }

    let mut srv = builder.build();
    info!("MCP server starting...");
    srv.run().await?;

    Ok(())
}
