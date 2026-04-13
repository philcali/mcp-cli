//! MCP server implementation with stdio transport.

use crate::handlers::{
    handle_initialize, handle_prompts_get, handle_prompts_list, handle_resources_list,
    handle_tools_call, handle_tools_list,
};
use crate::protocol::{load_tool_auth_config, *};
use crate::watcher::FileSystemWatcher;
use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

/// Client-provided root directory.
#[derive(Debug, Clone)]
pub struct Root {
    uri: String,
    #[allow(dead_code)]
    _name: Option<String>,
}

/// Entry for a discovered prompt with cache metadata.
#[derive(Debug, Clone)]
pub struct PromptEntry {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Option<Vec<crate::protocol::PromptArgument>>,
    pub file_path: PathBuf,
    pub loaded_at: std::time::Instant,
}

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

/// Server state and configuration.
pub struct McpServer {
    pub name: String,
    pub version: String,
    pub capabilities: ServerCapabilities,
    pub tools_dir: Option<PathBuf>,
    pub cached_tools: Arc<Mutex<HashMap<String, ToolDefinition>>>,
    pub resources_dir: Option<PathBuf>,
    pub cached_resources: Mutex<Vec<ResourceEntry>>,
    pub prompts_dir: Option<PathBuf>,
    pub cached_prompts: Arc<Mutex<HashMap<String, PromptEntry>>>,
    pub prompt_cache_config: PromptCacheConfig,
    pub roots: Mutex<Vec<Root>>,
    pub subscription_manager: std::sync::Arc<dyn crate::protocol::ResourceManager + Send + Sync>,
}

#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub script_path: PathBuf,
    pub auth_config: Option<ToolAuthConfig>,
}

pub struct CredentialResolver;

impl Default for CredentialResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialResolver {
    pub fn new() -> Self {
        Self
    }

    pub fn resolve_for_tool(
        tools_dir: &std::path::Path,
        tool_name: &str,
    ) -> Result<Vec<(String, String)>> {
        let auth_config = Self::load_auth_config(tools_dir, tool_name)?;
        match auth_config {
            Some(config) => Self::validate_and_inject(&config),
            None => Ok(Vec::new()),
        }
    }

    fn load_auth_config(
        tools_dir: &std::path::Path,
        tool_name: &str,
    ) -> Result<Option<ToolAuthConfig>> {
        let auth_path = tools_dir.join(tool_name).join(".auth.json");
        if auth_path.exists() {
            return load_tool_auth_config(&auth_path);
        }
        let flat_auth_path = tools_dir.join(format!("{}.auth.json", tool_name));
        if flat_auth_path.exists() {
            return load_tool_auth_config(&flat_auth_path);
        }
        Ok(None)
    }

    fn validate_and_inject(config: &ToolAuthConfig) -> Result<Vec<(String, String)>> {
        let mut creds = Vec::new();
        for env_var in &config.required_env_vars {
            match std::env::var(env_var) {
                Ok(value) => {
                    if value.is_empty() {
                        return Err(anyhow::anyhow!(
                            "Environment variable '{}' is set but empty.",
                            env_var
                        ));
                    }
                    creds.push((env_var.clone(), value));
                }
                Err(_) => {
                    let all_env_vars: Vec<String> = config.required_env_vars.to_vec();
                    return Err(anyhow::anyhow!(
                        "Missing required environment variable '{}' for tool '{:?}'.\nAvailable: {}\nPlease set {}.",
                        env_var,
                        config.strategy,
                        all_env_vars.join(", "),
                        env_var
                    ));
                }
            }
        }
        Ok(creds)
    }
}

#[derive(Debug, Clone)]
pub struct ResourceEntry {
    pub uri: String,
    pub resource_type: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub file_path: PathBuf,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new("mcp-cli", "0.1.0")
    }
}

impl McpServer {
    pub fn new(name: &str, version: &str) -> Self {
        let subscription_manager: std::sync::Arc<
            dyn crate::protocol::ResourceManager + Send + Sync,
        > = std::sync::Arc::new(crate::protocol::MemorySubscriptionManager::new());

        Self {
            name: name.to_string(),
            version: version.to_string(),
            capabilities: ServerCapabilities::new(),
            tools_dir: None,
            cached_tools: Arc::new(Mutex::new(HashMap::new())),
            resources_dir: None,
            cached_resources: Mutex::new(Vec::new()),
            prompts_dir: None,
            prompt_cache_config: PromptCacheConfig::default(),
            cached_prompts: Arc::new(Mutex::new(HashMap::new())),
            roots: Mutex::new(Vec::new()),
            subscription_manager,
        }
    }

    pub fn with_prompt_cache_config(mut self, config: PromptCacheConfig) -> Self {
        self.prompt_cache_config = config;
        self
    }

    pub fn add_root(&self, uri: String, name: Option<String>) {
        let mut roots = self.roots.lock().unwrap();
        if !roots.iter().any(|r| r.uri == uri) {
            roots.push(Root { uri, _name: name });
        }
    }

    #[allow(dead_code)]
    async fn handle_initialize(&self, params: &serde_json::Value) -> Result<serde_json::Value> {
        handle_initialize(self, params).await
    }

    pub fn enable_tools(mut self) -> Self {
        self.capabilities.tools = Some(true);
        self
    }

    pub fn enable_tools_dir(mut self, path: PathBuf) -> Self {
        self.tools_dir = Some(path);
        self
    }

    pub fn start_tool_watcher(&self) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>> {
        let dir = match &self.tools_dir {
            Some(p) => p.clone(),
            None => return Err(anyhow::anyhow!("No tools directory configured")),
        };
        let cached_tools_mutex = self.cached_tools.clone();
        crate::watcher::ToolWatcher::start_watching(
            dir,
            crate::watcher::WatchConfig {
                watch_for_changes: true,
            },
            Box::new(move || {
                cached_tools_mutex.lock().unwrap().clear();
            }),
        )
    }

    pub fn load_tools(&self) -> Result<HashMap<String, ToolDefinition>> {
        let dir = match &self.tools_dir {
            Some(p) => p,
            None => return Ok(HashMap::new()),
        };
        if !dir.exists() {
            warn!("Tools directory does not exist: {:?}", dir);
            return Ok(HashMap::new());
        }
        let mut tools = HashMap::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let metadata = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    warn!("Failed to read metadata for {:?}: {}", path, e);
                    continue;
                }
            };
            #[cfg(unix)]
            {
                use std::os::unix::prelude::*;
                let mode = metadata.permissions().mode();
                if mode & 0o111 == 0 {
                    continue;
                }
            }
            let name = match path.file_stem() {
                Some(stem) => stem.to_string_lossy().to_string(),
                None => continue,
            };
            let auth_config = match load_tool_auth_config(&path.with_extension("")) {
                Ok(Some(cfg)) => Some(cfg),
                Err(e) => {
                    warn!("Failed to load auth config for {}: {}", name, e);
                    None
                }
                Ok(None) => None,
            };
            tools.insert(
                name.clone(),
                ToolDefinition {
                    name: name.clone(),
                    description: format!("Tool script: {}", path.display()),
                    script_path: path,
                    auth_config,
                },
            );
        }
        Ok(tools)
    }

    pub fn load_resources(&self) -> Result<Vec<ResourceEntry>> {
        let dir = match &self.resources_dir {
            Some(p) => p,
            None => {
                info!("No resources directory configured");
                return Ok(Vec::new());
            }
        };
        if !dir.exists() {
            warn!("Resources directory does not exist: {:?}", dir);
            return Ok(Vec::new());
        }
        debug!("Loading resources from: {:?}", dir);
        let mut resources = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let (name, mime_type) = match (path.file_stem(), path.extension()) {
                (Some(stem), Some(ext)) => (
                    stem.to_string_lossy().to_string(),
                    Some(Self::mime_from_extension(ext.to_str().unwrap_or(""))),
                ),
                (Some(stem), None) => (stem.to_string_lossy().to_string(), None),
                _ => continue,
            };
            let uri = format!("file://{}", path.display());
            debug!("Found resource: {} -> {}", name, uri);
            resources.push(ResourceEntry {
                uri: uri.clone(),
                resource_type: "text".to_string(),
                name: name.clone(),
                description: Some(format!("Resource file: {}", path.display())),
                mime_type,
                file_path: path,
            });
        }
        debug!("Loaded {} resources", resources.len());
        Ok(resources)
    }

    fn mime_from_extension(ext: &str) -> String {
        match ext {
            "txt" | "text" => "text/plain".to_string(),
            "md" => "text/markdown".to_string(),
            "json" => "application/json".to_string(),
            "xml" => "application/xml".to_string(),
            "yaml" | "yml" => "application/yaml".to_string(),
            "toml" => "application/toml".to_string(),
            "rs" => "text/x-rust".to_string(),
            "sh" => "application/x-sh".to_string(),
            "py" => "text/x-python".to_string(),
            "js" => "application/javascript".to_string(),
            "html" | "htm" => "text/html".to_string(),
            "css" => "text/css".to_string(),
            "csv" => "text/csv".to_string(),
            _ => "application/octet-stream".to_string(),
        }
    }

    pub fn enable_resources_dir(mut self, path: PathBuf) -> Self {
        self.resources_dir = Some(path);
        self
    }

    pub fn enable_resources(mut self, list_changed: bool) -> Self {
        self.capabilities.resources = Some(ResourcesCapability { list_changed });
        self
    }

    pub fn enable_prompts(mut self) -> Self {
        self.capabilities.prompts = Some(true);
        self
    }

    pub fn enable_prompts_dir(mut self, path: PathBuf) -> Self {
        self.prompts_dir = Some(path);
        self
    }

    pub fn load_prompts(&self) -> Result<HashMap<String, PromptEntry>> {
        let dir = match &self.prompts_dir {
            Some(p) => p,
            None => return Ok(HashMap::new()),
        };
        if !dir.exists() {
            warn!("Prompts directory does not exist: {:?}", dir);
            return Ok(HashMap::new());
        }
        let mut prompts = HashMap::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to read prompt file {:?}: {}", path, e);
                    continue;
                }
            };
            let prompt_file: crate::protocol::PromptFile = match serde_json::from_str(&content) {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to parse prompt file {:?}: {}", path, e);
                    continue;
                }
            };
            prompts.insert(
                prompt_file.name.clone(),
                PromptEntry {
                    name: prompt_file.name,
                    description: prompt_file.description,
                    arguments: prompt_file.arguments,
                    file_path: path,
                    loaded_at: std::time::Instant::now(),
                },
            );
        }
        info!("Loaded {} prompts", prompts.len());
        Ok(prompts)
    }

    fn is_prompt_expired(&self, entry: &PromptEntry) -> bool {
        let ttl = std::time::Duration::from_secs(self.prompt_cache_config.ttl_secs);
        entry.loaded_at.elapsed() > ttl
    }

    pub fn get_prompt(&self, name: &str) -> Result<Option<PromptEntry>> {
        let cached = self.cached_prompts.lock().unwrap();
        if let Some(entry) = cached.get(name)
            && !self.is_prompt_expired(entry)
        {
            return Ok(Some(entry.clone()));
        }
        drop(cached);
        let mut cached = self.cached_prompts.lock().unwrap();
        *cached = self.load_prompts()?;
        Ok(cached.get(name).cloned())
    }

    pub fn invalidate_prompt_cache(&self) -> Result<()> {
        info!("Invalidating prompt cache");
        self.cached_prompts.lock().unwrap().clear();
        Ok(())
    }

    pub fn start_prompt_watcher(&self) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>> {
        let dir = match &self.prompts_dir {
            Some(p) => p.clone(),
            None => return Err(anyhow::anyhow!("No prompts directory configured")),
        };
        if !self.prompt_cache_config.watch_for_changes {
            warn!("Prompt file watching is disabled");
            return Ok(std::sync::Arc::new(tokio::task::spawn(async {})));
        }
        let cached_prompts_mutex = self.cached_prompts.clone();
        crate::watcher::PromptWatcher::start_watching(
            dir,
            crate::watcher::WatchConfig {
                watch_for_changes: true,
            },
            Box::new(move || {
                cached_prompts_mutex.lock().unwrap().clear();
            }),
        )
    }

    pub async fn run(&mut self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();
        info!("MCP server starting, waiting for messages...");
        let mut initialized = false;
        loop {
            match reader.next_line().await {
                Ok(Some(line)) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    debug!("Received message: {}", line);
                    let is_initialize = serde_json::from_str::<serde_json::Value>(&line)
                        .ok()
                        .map(|v| v.get("method").and_then(|m| m.as_str()) == Some("initialize"))
                        .unwrap_or(false);
                    match self.handle_request(&line, initialized).await {
                        Ok(response) => {
                            let _ = tokio::io::stdout()
                                .write_all(format!("{}\n", response).as_bytes())
                                .await;
                            let _ = tokio::io::stdout().flush().await;
                            if !initialized && is_initialize {
                                initialized = true;
                            }
                        }
                        Err(e) => {
                            error!("Error processing message: {}", e);
                            let err_resp = json!({ "jsonrpc": "2.0", "error": JsonRpcError::internal_error(&e.to_string()), "id": null });
                            let _ = tokio::io::stdout()
                                .write_all(
                                    format!("{}\n", serde_json::to_string(&err_resp).unwrap())
                                        .as_bytes(),
                                )
                                .await;
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    error!("Read error: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    async fn handle_request(&self, line: &str, initialized: bool) -> Result<String> {
        let request: JsonRpcRequest = serde_json::from_str(line)?;
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

    #[allow(dead_code)]
    async fn route_request(
        &self,
        method: &str,
        params: &serde_json::Value,
        initialized: bool,
    ) -> Result<serde_json::Value> {
        crate::routing::route_request(method, params, initialized, self).await
    }

    #[allow(dead_code)]
    async fn handle_tools_list(&self) -> Result<serde_json::Value> {
        handle_tools_list(self).await
    }

    #[allow(dead_code)]
    async fn handle_resources_list(&self) -> Result<serde_json::Value> {
        handle_resources_list(self).await
    }

    #[allow(dead_code)]
    pub async fn handle_roots_list(&self) -> Result<serde_json::Value> {
        info!("Handling roots list request");
        let roots = self.roots.lock().unwrap();
        let roots_list: Vec<_> = roots
            .iter()
            .map(|root| {
                if let Some(ref _name) = root._name {
                    json!({ "uri": root.uri, "name": _name })
                } else {
                    json!({ "uri": root.uri })
                }
            })
            .collect();
        Ok(json!({ "roots": roots_list }))
    }

    #[allow(dead_code)]
    async fn handle_prompts_list(&self) -> Result<serde_json::Value> {
        handle_prompts_list(self).await
    }

    #[allow(dead_code)]
    async fn handle_prompts_get(&self, params: &serde_json::Value) -> Result<serde_json::Value> {
        handle_prompts_get(self, params).await
    }

    #[allow(dead_code)]
    async fn handle_tools_call(&self, params: &serde_json::Value) -> Result<serde_json::Value> {
        handle_tools_call(self, params).await
    }

    #[allow(dead_code)]
    pub async fn handle_resources_read(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let uri_value = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'uri' parameter"))?;
        info!("Reading resource: {}", uri_value);
        let mut cached = self.cached_resources.lock().unwrap();
        if cached.is_empty() {
            *cached = self.load_resources()?;
            info!("Reloaded {} resources", cached.len());
        }
        if let Some(entry) = cached.iter().find(|r| r.uri == uri_value).cloned() {
            info!("Found resource: {:?}", entry.file_path);
            let content = std::fs::read_to_string(&entry.file_path)?;
            Ok(
                json!({ "contents": [{ "uri": entry.uri, "text": content, "mimeType": entry.mime_type }] }),
            )
        } else {
            Err(anyhow::anyhow!("Resource '{}' is not available", uri_value))
        }
    }

    #[allow(dead_code)]
    async fn handle_resources_subscribe(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let subscribe_params: crate::protocol::SubscribeResourceParams =
            serde_json::from_value(params.clone())
                .context("Failed to parse resources/subscribe parameters")?;
        info!("Subscribing to resource: {}", subscribe_params.uri);
        let mut cached = self.cached_resources.lock().unwrap();
        if cached.is_empty() {
            *cached = self.load_resources()?;
        }
        if !cached.iter().any(|r| r.uri == subscribe_params.uri) {
            return Err(anyhow::anyhow!(
                "Resource '{}' does not exist",
                subscribe_params.uri
            ));
        }
        let was_new = self.subscription_manager.subscribe(&subscribe_params.uri);
        if was_new {
            info!("Successfully subscribed to: {}", subscribe_params.uri);
        } else {
            debug!("Already subscribed to: {}", subscribe_params.uri);
        }
        Ok(json!({}))
    }

    #[allow(dead_code)]
    async fn handle_resources_unsubscribe(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let unsubscribe_params: crate::protocol::UnsubscribeResourceParams =
            serde_json::from_value(params.clone())
                .context("Failed to parse resources/unsubscribe parameters")?;
        info!("Unsubscribing from resource: {}", unsubscribe_params.uri);
        let mut cached = self.cached_resources.lock().unwrap();
        if cached.is_empty() {
            *cached = self.load_resources()?;
        }
        if !cached.iter().any(|r| r.uri == unsubscribe_params.uri) {
            return Err(anyhow::anyhow!(
                "Resource '{}' does not exist",
                unsubscribe_params.uri
            ));
        }
        let was_subscribed = self
            .subscription_manager
            .unsubscribe(&unsubscribe_params.uri);
        if !was_subscribed {
            debug!("Not subscribed to: {}", unsubscribe_params.uri);
        } else {
            info!("Successfully unsubscribed from: {}", unsubscribe_params.uri);
        }
        Ok(json!({}))
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
        assert!(server.cached_prompts.lock().unwrap().contains_key("test"));
        std::thread::sleep(std::time::Duration::from_secs(2));
        if let Some(entry) = server.cached_prompts.lock().unwrap().get("test") {
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
        let _result: serde_json::Value = server.handle_prompts_list().await.unwrap();
        assert!(server.cached_prompts.lock().unwrap().contains_key("test"));
        server.invalidate_prompt_cache().unwrap();
        assert!(server.cached_prompts.lock().unwrap().is_empty());
    }
}
