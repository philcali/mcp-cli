//! MCP CLI - Model Context Protocol server implementation.
//!
//! This is a minimal MCP server that communicates via stdio using JSON-RPC 2.0.

use anyhow::Result;
use clap::Parser;
use mcp_cli::server::ServerBuilder;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

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

    /// Directory path for resource templates (.template.json files)
    #[arg(long)]
    resource_templates_dir: Option<std::path::PathBuf>,

    /// Enable logging capability for the server
    #[arg(long)]
    with_logging: bool,

    /// Enable sampling capability (server can ask client to call LLM)
    #[arg(long)]
    with_sampling: bool,

    /// Enable tasks capability (task-augmented requests)
    #[arg(long)]
    with_tasks: bool,

    /// Enable telemetry capability
    #[arg(long)]
    with_telemetry: bool,

    /// Enable elicitation capability (request user input via client)
    #[arg(long)]
    with_elicitation: bool,

    /// Run as persistent stdio server (daemon mode)
    #[arg(long, short)]
    daemon: bool,

    /// Run as HTTP server (MCP Streamable HTTP transport)
    #[cfg(feature = "http")]
    #[arg(long, conflicts_with = "daemon")]
    http: bool,

    /// HTTP bind address (only with --http)
    #[cfg(feature = "http")]
    #[arg(long, default_value = "127.0.0.1:3000", requires = "http")]
    http_addr: Option<String>,

    /// Log level (TRACE, DEBUG, INFO, WARN, ERROR)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Log file path (default: ~/.cache/mcp-cli/mcp-cli.log). Set to "-" for stderr.
    #[arg(long)]
    log_file: Option<String>,
}

/// Resolve the default log file path: ~/.cache/mcp-cli/mcp-cli.log
fn default_log_file() -> PathBuf {
    let cache_dir = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(format!("{}/.cache", home))
        });
    cache_dir.join("mcp-cli").join("mcp-cli.log")
}

/// Parse log level from string (case-insensitive)
fn parse_log_level(level: &str) -> Option<tracing::Level> {
    match level.to_lowercase().as_str() {
        "trace" => Some(tracing::Level::TRACE),
        "debug" => Some(tracing::Level::DEBUG),
        "info" => Some(tracing::Level::INFO),
        "warn" => Some(tracing::Level::WARN),
        "error" => Some(tracing::Level::ERROR),
        _ => None,
    }
}

/// Initialize tracing with configurable output and level
fn init_logging(log_file: &Option<String>, log_level: &str) -> Result<()> {
    let level = parse_log_level(log_level).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid log level '{}'. Use: trace, debug, info, warn, error",
            log_level
        )
    })?;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::default()
            .add_directive(format!("mcp_cli={}", level).parse().unwrap())
            .add_directive(format!("mcp-cli={}", level).parse().unwrap())
    });

    let log_target = match log_file {
        Some(path) if path == "-" => "stderr",
        Some(path) => path,
        None => "default",
    };

    match log_target {
        "stderr" => {
            // Log to stderr (useful when stdout must remain clean for MCP stdio)
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_span_events(FmtSpan::NONE)
                .with_target(false)
                .with_file(true)
                .with_line_number(true)
                .with_writer(std::io::stderr)
                .init();
        }
        "default" => {
            // Write to log file
            let log_path = default_log_file();
            fs::create_dir_all(log_path.parent().unwrap())?;

            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)?;

            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_span_events(FmtSpan::NONE)
                .with_target(false)
                .with_file(true)
                .with_line_number(true)
                .with_writer(file)
                .init();

            info!("Logging to {}", log_path.display());
        }
        custom_path => {
            // Custom file path
            let path = PathBuf::from(custom_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            let file = OpenOptions::new().create(true).append(true).open(&path)?;

            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_span_events(FmtSpan::NONE)
                .with_target(false)
                .with_file(true)
                .with_line_number(true)
                .with_writer(file)
                .init();

            info!("Logging to {}", path.display());
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    init_logging(&cli.log_file, &cli.log_level)?;

    let mut builder = ServerBuilder::new("mcp-cli", "0.1.0")
        .with_tools()
        .with_resources(true);

    let tools_dir = cli.tools_dir.clone();
    let resources_dir = cli.resources_dir.clone();
    let prompts_dir = cli.prompts_dir.clone();
    let resource_templates_dir = cli.resource_templates_dir.clone();

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

    if let Some(ref td) = resource_templates_dir {
        info!("Using resource templates directory: {:?}", td);
        builder = builder
            .with_resource_templates()
            .with_resource_templates_dir(td.clone());
    }

    if cli.with_logging {
        info!("Enabling logging capability");
        builder = builder.with_logging();
    }

    if cli.with_sampling {
        info!("Enabling sampling capability");
        builder = builder.with_sampling();
    }

    if cli.with_tasks {
        info!("Enabling tasks capability");
        builder = builder.with_tasks();
    }

    if cli.with_telemetry {
        info!("Enabling telemetry capability");
        builder = builder.with_telemetry();
    }

    if cli.with_elicitation {
        info!("Enabling elicitation capability");
        builder = builder.with_elicitation();
    }

    let srv = builder.build();

    // Start watchers if directories are configured
    if tools_dir.is_some() {
        match srv.start_tool_watcher() {
            Ok(_) => info!("Started tool watcher"),
            Err(e) => warn!("Failed to start tool watcher: {}", e),
        }
    }

    if prompts_dir.is_some() {
        match srv.start_prompt_watcher() {
            Ok(_) => info!("Started prompt watcher"),
            Err(e) => warn!("Failed to start prompt watcher: {}", e),
        }
    }

    if resources_dir.is_some() {
        match srv.start_resource_watcher() {
            Ok(_) => info!("Started resource watcher"),
            Err(e) => warn!("Failed to start resource watcher: {}", e),
        }
    }

    if resource_templates_dir.is_some() {
        match srv.start_resource_templates_watcher() {
            Ok(_) => info!("Started resource templates watcher"),
            Err(e) => warn!("Failed to start resource templates watcher: {}", e),
        }
    }

    info!("MCP server starting...");

    let result = async {
        #[cfg(feature = "http")]
        if cli.http {
            let addr: std::net::SocketAddr = cli
                .http_addr
                .as_deref()
                .unwrap_or("127.0.0.1:3000")
                .parse()
                .expect("Invalid HTTP address");
            info!("Running in HTTP mode on {}", addr);
            return srv.run_http(addr).await;
        }

        #[cfg(not(feature = "http"))]
        if cli.daemon {
            info!("Running in daemon mode (persistent server)");
            return srv.run_daemon().await;
        }

        srv.run().await
    }
    .await;

    // Gracefully shut down all file system watchers
    srv.shutdown_watchers();

    result
}
