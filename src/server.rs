//! MCP server implementation with stdio transport.

use crate::protocol::{
    JsonRpcError, JsonRpcNotification, JsonRpcRequest, PromptsCapability, ResourcesCapability,
    SamplingCapability, ServerCapabilities, ToolsCapability,
};
use crate::watcher::{FileSystemWatcher, WatchConfig};
use anyhow::Result;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

pub use crate::state::ServerState;

// Re-export discovery types for convenience
pub use crate::discovery::tools::ToolDefinition;
pub type PromptEntry = crate::discovery::prompts::PromptEntry;

/// Configuration for prompt caching.
#[derive(Debug, Clone)]
pub struct PromptCacheConfig {
    pub ttl_secs: u64,
    pub watch_for_changes: bool,
}

impl Default for PromptCacheConfig {
    fn default() -> Self {
        Self {
            ttl_secs: 300,
            watch_for_changes: true,
        }
    }
}

use tokio::sync::{broadcast, oneshot};

/// Type for a pending sampling request: (request_id, sender for client response)
type PendingSampling = (
    String,
    std::sync::Arc<tokio::sync::Mutex<Option<oneshot::Sender<serde_json::Value>>>>,
);

/// Server state and configuration.
pub struct McpServer {
    pub state: std::sync::Arc<crate::state::ServerState>,
    pub capabilities: ServerCapabilities,
    pub prompt_cache_config: PromptCacheConfig,
    /// Broadcast channel for streaming notifications to clients
    pub notification_tx: Option<std::sync::Arc<broadcast::Sender<String>>>,
    /// Cached stdout handle to avoid repeated tokio::io::stdout() calls
    /// which can cause broken pipe errors in subprocess contexts.
    pub(crate) stdout: Option<std::sync::Arc<tokio::sync::Mutex<tokio::io::Stdout>>>,
    /// Pending sampling request: (request_id, sender for client response)
    pub(crate) pending_sampling: Option<PendingSampling>,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new("mcp-cli", "0.1.0")
    }
}

impl McpServer {
    pub fn new(name: &str, version: &str) -> Self {
        let state = std::sync::Arc::new(crate::state::ServerState::new(name, version));
        // Create notification channel for streaming support
        let (notification_tx, _) = broadcast::channel(100);
        // Cache stdout handle to avoid broken pipe errors in subprocess contexts
        let stdout = Some(std::sync::Arc::new(tokio::sync::Mutex::new(
            tokio::io::stdout(),
        )));

        Self {
            state,
            capabilities: ServerCapabilities::new(),
            prompt_cache_config: PromptCacheConfig::default(),
            notification_tx: Some(std::sync::Arc::new(notification_tx)),
            stdout,
            pending_sampling: None,
        }
    }

    /// Send a notification to all subscribed clients (for streaming)
    pub async fn send_notification(&self, method: &str, params: serde_json::Value) {
        if let Some(ref tx) = self.notification_tx {
            let msg = json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params
            });
            let _ = tx.send(msg.to_string());
        }
    }

    /// Set up a pending sampling request and return the oneshot sender
    /// so the main loop can deliver the client's response back to the handler.
    pub fn setup_pending_sampling(
        &mut self,
        request_id: String,
    ) -> std::sync::Arc<tokio::sync::Mutex<Option<oneshot::Sender<serde_json::Value>>>> {
        let sender = std::sync::Arc::new(tokio::sync::Mutex::new(None));
        self.pending_sampling = Some((request_id, sender.clone()));
        sender
    }

    /// Clear the pending sampling request.
    pub fn clear_pending_sampling(&mut self) {
        self.pending_sampling = None;
    }

    pub fn with_prompt_cache_config(mut self, config: PromptCacheConfig) -> Self {
        self.prompt_cache_config = config;
        self
    }

    pub fn add_root(&self, uri: String, name: Option<String>) {
        self.state.add_root(uri, name);
    }

    pub fn enable_tools(mut self) -> Self {
        self.capabilities.tools = Some(ToolsCapability {
            list_changed: Some(true),
        });
        self
    }

    pub fn enable_tools_dir(mut self, path: PathBuf) -> Self {
        let tools_dir_exists = std::path::Path::new(&path).exists();
        // Clone the content to get a mutable copy
        let mut new_state = (*self.state).clone();
        new_state.tools_dir = Some(path);
        self.state = std::sync::Arc::new(new_state);
        if !tools_dir_exists {
            warn!("Tools directory does not exist");
        }
        self
    }

    pub fn start_tool_watcher(&self) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>> {
        let dir = match &self.state.tools_dir {
            Some(p) => p.clone(),
            None => return Err(anyhow::anyhow!("No tools directory configured")),
        };
        let state_clone = self.state.clone();
        let notification_tx = self.notification_tx.clone();
        crate::watcher::ToolWatcher::start_watching(
            dir,
            WatchConfig {
                watch_for_changes: true,
            },
            Box::new(move || {
                state_clone.cached_tools.lock().unwrap().clear();
            }),
            Box::new(move || {
                if let Some(ref tx) = notification_tx {
                    let msg = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/tools/listChanged",
                    });
                    let _ = tx.send(msg.to_string());
                }
            }),
        )
    }

    pub fn load_tools(&self) -> Result<std::collections::HashMap<String, ToolDefinition>> {
        match &self.state.tools_dir {
            Some(dir) => crate::discovery::tools::discover_tools(dir),
            None => Ok(std::collections::HashMap::new()),
        }
    }

    pub fn load_resources(&self) -> Result<Vec<crate::discovery::resources::ResourceEntry>> {
        self.state.load_resources()
    }

    pub fn enable_resources_dir(mut self, path: PathBuf) -> Self {
        // Clone the content to get a mutable copy
        let mut new_state = (*self.state).clone();
        new_state.resources_dir = Some(path);
        self.state = std::sync::Arc::new(new_state);
        self
    }

    pub fn enable_resources(mut self, list_changed: bool) -> Self {
        self.capabilities.resources = Some(ResourcesCapability {
            list_changed,
            template_list_changed: None,
        });
        self
    }

    pub fn enable_prompts(mut self) -> Self {
        self.capabilities.prompts = Some(PromptsCapability {
            list_changed: Some(true),
        });
        self
    }

    pub fn enable_logging(mut self) -> Self {
        self.capabilities.logging = Some(true);
        self
    }

    pub fn enable_telemetry(mut self) -> Self {
        // Add telemetry capability to experimental features
        let mut experimental = self.capabilities.experimental.clone().unwrap_or_default();
        experimental.insert("telemetry".to_string(), json!(true));
        self.capabilities.experimental = Some(experimental);
        self
    }

    pub fn enable_sampling(mut self) -> Self {
        self.capabilities.sampling = Some(SamplingCapability { list_changed: None });
        self
    }

    pub fn enable_resource_templates(mut self) -> Self {
        match &mut self.capabilities.resources {
            Some(cap) => {
                cap.list_changed = true;
                cap.template_list_changed = Some(true);
            }
            None => {
                self.capabilities.resources = Some(ResourcesCapability {
                    list_changed: true,
                    template_list_changed: Some(true),
                });
            }
        }
        self
    }

    pub fn enable_resource_templates_dir(mut self, path: PathBuf) -> Self {
        let mut new_state = (*self.state).clone();
        new_state.resource_templates_dir = Some(path);
        self.state = std::sync::Arc::new(new_state);
        self
    }

    pub fn load_resource_templates(
        &self,
    ) -> Result<Vec<crate::discovery::resources::ResourceTemplateEntry>, anyhow::Error> {
        self.state.load_resource_templates()
    }

    pub fn start_resource_templates_watcher(
        &self,
    ) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>> {
        let dir = match &self.state.resource_templates_dir {
            Some(p) => p.clone(),
            None => {
                return Err(anyhow::anyhow!(
                    "No resource templates directory configured"
                ));
            }
        };
        let state_clone = self.state.clone();
        let notification_tx = self.notification_tx.clone();
        crate::watcher::ResourceTemplateWatcher::start_watching(
            dir,
            WatchConfig {
                watch_for_changes: true,
            },
            Box::new(move || {
                state_clone
                    .cached_resource_templates
                    .lock()
                    .unwrap()
                    .clear();
            }),
            Box::new(move || {
                if let Some(ref tx) = notification_tx {
                    let msg = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/resources/templates/listChanged",
                    });
                    let _ = tx.send(msg.to_string());
                }
            }),
        )
    }

    pub fn enable_prompts_dir(mut self, path: PathBuf) -> Self {
        // Clone the content to get a mutable copy
        let mut new_state = (*self.state).clone();
        new_state.prompts_dir = Some(path);
        self.state = std::sync::Arc::new(new_state);
        self
    }

    pub fn load_prompts(
        &self,
    ) -> Result<std::collections::HashMap<String, crate::discovery::prompts::PromptEntry>> {
        self.state.load_prompts()
    }

    fn is_prompt_expired(&self, entry: &crate::discovery::prompts::PromptEntry) -> bool {
        let ttl = std::time::Duration::from_secs(self.prompt_cache_config.ttl_secs);
        entry.loaded_at.elapsed() > ttl
    }

    pub fn get_prompt(&self, name: &str) -> Result<Option<crate::discovery::prompts::PromptEntry>> {
        // Check if cached entry exists and is not expired
        let (cached_entry, is_expired) = {
            let cached = self.state.cached_prompts.lock().unwrap();
            if let Some(entry) = cached.get(name) {
                let is_expired = self.is_prompt_expired(entry);
                (Some(entry.clone()), is_expired)
            } else {
                (None, true)
            }
        };

        if !is_expired {
            return Ok(cached_entry);
        }

        // Cache miss or expired - load fresh prompts and cache them
        let mut cached = self.state.cached_prompts.lock().unwrap();
        *cached = self.load_prompts()?;
        Ok(cached.get(name).cloned())
    }

    pub fn invalidate_prompt_cache(&self) -> Result<()> {
        info!("Invalidating prompt cache");
        self.state.cached_prompts.lock().unwrap().clear();
        Ok(())
    }

    pub fn start_prompt_watcher(&self) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>> {
        let dir = match &self.state.prompts_dir {
            Some(p) => p.clone(),
            None => return Err(anyhow::anyhow!("No prompts directory configured")),
        };
        if !self.prompt_cache_config.watch_for_changes {
            warn!("Prompt file watching is disabled");
            return Ok(std::sync::Arc::new(tokio::task::spawn(async {})));
        }
        let state_clone = self.state.clone();
        let notification_tx = self.notification_tx.clone();
        crate::watcher::PromptWatcher::start_watching(
            dir,
            WatchConfig {
                watch_for_changes: true,
            },
            Box::new(move || {
                state_clone.cached_prompts.lock().unwrap().clear();
            }),
            Box::new(move || {
                if let Some(ref tx) = notification_tx {
                    let msg = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/prompts/listChanged",
                    });
                    let _ = tx.send(msg.to_string());
                }
            }),
        )
    }

    pub fn start_resource_watcher(&self) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>> {
        let dir = match &self.state.resources_dir {
            Some(p) => p.clone(),
            None => return Err(anyhow::anyhow!("No resources directory configured")),
        };
        let state_clone = self.state.clone();
        let notification_tx = self.notification_tx.clone();
        crate::watcher::ResourceWatcher::start_watching(
            dir.clone(),
            WatchConfig {
                watch_for_changes: true,
            },
            Box::new({
                let state = state_clone.clone();
                move || {
                    state.cached_resources.lock().unwrap().clear();
                }
            }),
            Box::new({
                let tx = notification_tx.clone();
                move || {
                    if let Some(ref tx) = tx {
                        let msg = json!({
                            "jsonrpc": "2.0",
                            "method": "notifications/resources/listChanged",
                        });
                        let _ = tx.send(msg.to_string());
                    }
                }
            }),
            Box::new({
                let state = state_clone.clone();
                let tx = notification_tx;
                move |path: &std::path::Path| {
                    let resource_uri = format!("file://{}", path.display());
                    let subscriptions: Vec<String> = state.subscription_manager.get_subscriptions();
                    // Check direct match or template match
                    let is_subscribed = subscriptions.contains(&resource_uri) || {
                        // Check if any subscription is a template that matches this URI
                        subscriptions.iter().any(|sub_uri| {
                            if !sub_uri.contains("{path}") {
                                return false;
                            }
                            // Simple template matching: split on {path} and check prefix/suffix
                            let parts: Vec<&str> = sub_uri.split("{path}").collect();
                            if parts.len() != 2 {
                                return false;
                            }
                            resource_uri.starts_with(parts[0]) && resource_uri.ends_with(parts[1])
                        })
                    };
                    if is_subscribed && let Some(ref tx) = tx {
                        let msg = json!({
                            "jsonrpc": "2.0",
                            "method": "notifications/resources/updated",
                            "params": {
                                "uri": resource_uri,
                            },
                        });
                        let _ = tx.send(msg.to_string());
                    }
                }
            }),
        )
    }

    /// Start background task that sends notifications from the broadcast channel to stdout.
    fn start_notification_sender(
        notification_tx: std::sync::Arc<broadcast::Sender<String>>,
        stdout: std::sync::Arc<tokio::sync::Mutex<tokio::io::Stdout>>,
    ) -> tokio::task::JoinHandle<()> {
        let mut rx = notification_tx.subscribe();

        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                let mut out = stdout.lock().await;
                if let Err(e) = out.write_all(format!("{}\n", msg).as_bytes()).await {
                    error!("Failed to send notification: {}", e);
                }
            }
        })
    }

    /// Run in one-shot mode: process stdin until EOF, then exit.
    pub async fn run(&mut self) -> Result<()> {
        // Start notification sender background task if we have a channel
        let _notification_handle = self.notification_tx.as_ref().and_then(|tx| {
            self.stdout
                .as_ref()
                .map(|stdout| Self::start_notification_sender(Arc::clone(tx), Arc::clone(stdout)))
        });

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();
        info!("MCP server starting, waiting for messages...");

        let stdout = self.stdout.as_ref().expect("stdout should be set").clone();

        loop {
            match reader.next_line().await {
                Ok(Some(line)) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    debug!("Received message: {}", line);
                    match self.process_message(&line, &stdout).await {
                        Ok(is_init) => {
                            if !self.state.is_initialized() && is_init {
                                self.state.set_initialized();
                            }
                        }
                        Err(e) => {
                            error!("Error processing message: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    info!("EOF received, exiting");
                    break;
                }
                Err(e) => {
                    error!("Read error: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    /// Run in daemon mode with graceful shutdown support.
    /// Blocks until SIGINT or SIGTERM is received.
    pub async fn run_daemon(&mut self) -> Result<()> {
        // Start notification sender background task if we have a channel
        let _notification_handle = self.notification_tx.as_ref().and_then(|tx| {
            self.stdout
                .as_ref()
                .map(|stdout| Self::start_notification_sender(Arc::clone(tx), Arc::clone(stdout)))
        });

        // Set up signal handlers for Unix platforms
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};

            let mut term_signal = signal(SignalKind::terminate())?;
            let mut int_signal = signal(SignalKind::interrupt())?;

            let stdout = self.stdout.as_ref().expect("stdout should be set").clone();

            info!("Daemon mode: waiting for SIGINT or SIGTERM...");

            loop {
                tokio::select! {
                    result = self.stdin_loop(&stdout) => {
                        if let Err(e) = result {
                            error!("stdin loop error: {}", e);
                        }
                    }
                    _ = term_signal.recv() => {
                        info!("Received SIGTERM, exiting gracefully...");
                        break;
                    }
                    _ = int_signal.recv() => {
                        info!("Received SIGINT (Ctrl-C), exiting gracefully...");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Process a single line from stdin: check for sampling responses, then handle as a request.
    async fn process_message(
        &mut self,
        line: &str,
        stdout: &std::sync::Arc<tokio::sync::Mutex<tokio::io::Stdout>>,
    ) -> Result<bool> {
        // Check if this is a response to a pending sampling request
        if let Some((pending_id, sender)) = &self.pending_sampling
            && let Ok(response) = serde_json::from_str::<serde_json::Value>(line)
            && response.get("id").and_then(|v| v.as_str()) == Some(pending_id)
        {
            info!("Received sampling response from client");
            let mut guard = sender.lock().await;
            if let Some(tx) = guard.take() {
                let _ = tx.send(response);
            }
            return Ok(false);
        }

        let is_initialize = serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .map(|v| v.get("method").and_then(|m| m.as_str()) == Some("initialize"))
            .unwrap_or(false);

        match self.handle_request(line, self.state.is_initialized()).await {
            Ok(response) => {
                // Empty response means notification — no reply needed
                if !response.is_empty() {
                    let mut out = stdout.lock().await;
                    let _ = out.write_all(format!("{}\n", response).as_bytes()).await;
                    let _ = out.flush().await;
                }
            }
            Err(e) => {
                error!("Error processing message: {}", e);
                let err_resp = json!({ "jsonrpc": "2.0", "error": JsonRpcError::internal_error(&e.to_string()), "id": null });
                let mut out = stdout.lock().await;
                let _ = out
                    .write_all(
                        format!(
                            "{}\n",
                            serde_json::to_string(&err_resp)
                                .expect("error response should serialize")
                        )
                        .as_bytes(),
                    )
                    .await;
            }
        }
        Ok(is_initialize)
    }

    /// Internal stdin loop - processes lines until EOF, then returns.
    async fn stdin_loop(
        &mut self,
        stdout: &std::sync::Arc<tokio::sync::Mutex<tokio::io::Stdout>>,
    ) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();
        info!("Daemon mode: waiting for messages...");

        loop {
            match reader.next_line().await {
                Ok(Some(line)) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    debug!("Received message: {}", line);
                    self.process_message(&line, stdout).await?;
                }
                Ok(None) => {
                    // In daemon mode, when stdin closes (client disconnected),
                    // we keep waiting for more input
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                Err(e) => {
                    error!("Read error: {}", e);
                    return Err(anyhow::anyhow!(e));
                }
            }
        }
    }

    async fn handle_request(&mut self, line: &str, initialized: bool) -> Result<String> {
        let raw: serde_json::Value = serde_json::from_str(line)?;

        // If there's no id, it's a notification — handle but don't send a response
        if raw.get("id").is_none() {
            let notification: JsonRpcNotification = serde_json::from_value(raw)?;
            debug!("Processing notification: {}", notification.method);
            let _ = self
                .route_request(&notification.method, &notification.params, initialized)
                .await;
            return Ok(String::new());
        }

        let request: JsonRpcRequest = serde_json::from_value(raw)?;
        debug!(
            "Processing {} with id={}",
            request.method,
            match &request.id_value {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => "null".to_string(),
            }
        );
        let result = self
            .route_request(&request.method, &request.params, initialized)
            .await;
        Ok(match result {
            Ok(resp) => {
                json!({ "jsonrpc": "2.0", "result": resp, "id": request.id_value }).to_string()
            }
            Err(e) => {
                let err_resp = JsonRpcError::internal_error(&e.to_string());
                json!({ "jsonrpc": "2.0", "error": err_resp, "id": request.id_value }).to_string()
            }
        })
    }

    async fn route_request(
        &mut self,
        method: &str,
        params: &serde_json::Value,
        initialized: bool,
    ) -> Result<serde_json::Value> {
        crate::routing::route_request(method, params, initialized, self).await
    }
}

pub struct McpServerWithTools {
    inner: McpServer,
}
impl McpServerWithTools {
    pub fn run(self) -> McpServer {
        self.inner
    }
}

pub struct ServerBuilder {
    name: String,
    version: String,
    enable_tools: bool,
    tools_dir: Option<std::path::PathBuf>,
    enable_resources: bool,
    resources_list_changed: bool,
    resources_dir: Option<std::path::PathBuf>,
    enable_prompts: bool,
    prompts_dir: Option<std::path::PathBuf>,
    enable_logging: bool,
    enable_telemetry: bool,
    enable_sampling: bool,
    enable_resource_templates: bool,
    resource_templates_dir: Option<std::path::PathBuf>,
}

impl ServerBuilder {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            enable_tools: false,
            tools_dir: None,
            enable_resources: false,
            resources_list_changed: false,
            resources_dir: None,
            enable_prompts: false,
            prompts_dir: None,
            enable_logging: false,
            enable_telemetry: false,
            enable_sampling: false,
            enable_resource_templates: false,
            resource_templates_dir: None,
        }
    }
    pub fn with_tools(mut self) -> Self {
        self.enable_tools = true;
        self
    }
    pub fn with_tools_dir<P: Into<std::path::PathBuf>>(mut self, path: P) -> Self {
        self.tools_dir = Some(path.into());
        self
    }
    pub fn with_resources(mut self, list_changed: bool) -> Self {
        self.enable_resources = true;
        self.resources_list_changed = list_changed;
        self
    }
    pub fn with_resources_dir<P: Into<std::path::PathBuf>>(mut self, path: P) -> Self {
        self.resources_dir = Some(path.into());
        self
    }
    pub fn with_prompts(mut self) -> Self {
        self.enable_prompts = true;
        self
    }
    pub fn with_logging(mut self) -> Self {
        self.enable_logging = true;
        self
    }
    pub fn with_telemetry(mut self) -> Self {
        self.enable_telemetry = true;
        self
    }
    pub fn with_sampling(mut self) -> Self {
        self.enable_sampling = true;
        self
    }
    pub fn with_resource_templates(mut self) -> Self {
        self.enable_resource_templates = true;
        self
    }
    pub fn with_resource_templates_dir<P: Into<std::path::PathBuf>>(mut self, path: P) -> Self {
        self.resource_templates_dir = Some(path.into());
        self
    }
    pub fn with_prompts_dir<P: Into<std::path::PathBuf>>(mut self, path: P) -> Self {
        self.prompts_dir = Some(path.into());
        self
    }

    pub fn build(self) -> McpServer {
        let mut server = McpServer::new(&self.name, &self.version);
        if self.enable_tools {
            server = server.enable_tools();
        }
        if let Some(ref path) = self.tools_dir {
            server = server.enable_tools_dir(path.clone());
        }
        if self.enable_resources {
            server = server.enable_resources(self.resources_list_changed);
        }
        if let Some(ref path) = self.resources_dir {
            server = server.enable_resources_dir(path.clone());
        }
        if self.enable_prompts {
            server = server.enable_prompts();
        }
        if let Some(ref path) = self.prompts_dir {
            server = server.enable_prompts_dir(path.clone());
        }
        if self.enable_logging {
            server = server.enable_logging();
        }
        if self.enable_telemetry {
            server = server.enable_telemetry();
        }
        if self.enable_sampling {
            server = server.enable_sampling();
        }
        if self.enable_resource_templates {
            server = server.enable_resource_templates();
        }
        if let Some(ref path) = self.resource_templates_dir {
            server = server.enable_resource_templates_dir(path.clone());
        }
        server
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new("mcp-cli", "0.1.0")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_prompt_cache_ttl() {
        let temp_dir = TempDir::new().unwrap();
        let prompt_path = temp_dir.path().join("test.prompt.json");
        std::fs::write(
            &prompt_path,
            r#"{"name": "test", "description": "Test prompt", "messages": []}"#,
        )
        .unwrap();
        let server = McpServer::new("test-server", "1.0.0")
            .enable_prompts_dir(temp_dir.path().to_path_buf())
            .with_prompt_cache_config(PromptCacheConfig {
                ttl_secs: 1,
                watch_for_changes: false,
            });
        server.invalidate_prompt_cache().unwrap();
        let _entry = server.get_prompt("test").unwrap();
        assert!(
            server
                .state
                .cached_prompts
                .lock()
                .unwrap()
                .contains_key("test")
        );
        std::thread::sleep(std::time::Duration::from_secs(2));
        if let Some(entry) = server.state.cached_prompts.lock().unwrap().get("test") {
            assert!(server.is_prompt_expired(entry));
        }
    }

    #[tokio::test]
    async fn test_prompt_cache_invalidation() {
        let temp_dir = TempDir::new().unwrap();
        let prompt_path = temp_dir.path().join("test.prompt.json");
        std::fs::write(
            &prompt_path,
            r#"{"name": "test", "description": "Test prompt", "messages": []}"#,
        )
        .unwrap();
        let server = McpServer::new("test-server", "1.0.0")
            .enable_prompts_dir(temp_dir.path().to_path_buf());
        let _result: serde_json::Value =
            crate::handlers::handle_prompts_list(&server).await.unwrap();
        assert!(
            server
                .state
                .cached_prompts
                .lock()
                .unwrap()
                .contains_key("test")
        );
        server.invalidate_prompt_cache().unwrap();
        assert!(server.state.cached_prompts.lock().unwrap().is_empty());
    }
}
