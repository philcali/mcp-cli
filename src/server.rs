//! MCP server implementation with stdio transport.

use crate::protocol::*;
use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

/// Client-provided root directory.
#[derive(Debug, Clone)]
struct Root {
    uri: String,
    #[allow(dead_code)] // Reserved for future use when client sends roots during init
    _name: Option<String>,
}

/// Entry for a discovered prompt.
#[derive(Debug, Clone)]
struct PromptEntry {
    name: String,
    description: Option<String>,
    arguments: Option<Vec<crate::protocol::PromptArgument>>,
    file_path: PathBuf,
}

/// Server state and configuration.
pub struct McpServer {
    name: String,
    version: String,
    capabilities: ServerCapabilities,
    /// Path to tools directory (optional)
    tools_dir: Option<PathBuf>,
    /// Cached tool list from discovered scripts
    cached_tools: Mutex<HashMap<String, ToolDefinition>>,
    /// Path to resources directory (optional)
    resources_dir: Option<PathBuf>,
    /// Cached resource list
    cached_resources: Mutex<Vec<ResourceEntry>>,
    /// Path to prompts directory (optional)
    prompts_dir: Option<PathBuf>,
    /// Cached prompt list
    cached_prompts: Mutex<HashMap<String, PromptEntry>>,
    /// Client-provided root directories for file access
    roots: Mutex<Vec<Root>>,
    /// Resource subscriptions manager
    subscription_manager: std::sync::Arc<dyn crate::protocol::ResourceManager + Send + Sync>,
}

/// Definition of a discoverable tool with auth config.
#[derive(Debug, Clone)]
struct ToolDefinition {
    name: String,
    description: String,
    script_path: PathBuf,
    /// Authentication configuration for this tool (if any)
    auth_config: Option<ToolAuthConfig>,
}

/// Credential resolver that validates and injects environment variables.
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

    /// Resolve credentials for a tool by validating required env vars.
    pub fn resolve_for_tool(
        tools_dir: &std::path::Path,
        tool_name: &str,
    ) -> Result<Vec<(String, String)>> {
        let auth_config = Self::load_auth_config(tools_dir, tool_name)?;

        match auth_config {
            Some(config) => Self::validate_and_inject(&config),
            None => Ok(Vec::new()), // No auth required for this tool
        }
    }

    /// Load the auth config for a specific tool.
    fn load_auth_config(
        tools_dir: &std::path::Path,
        tool_name: &str,
    ) -> Result<Option<ToolAuthConfig>> {
        let auth_path = tools_dir.join(tool_name).join(".auth.json");
        if auth_path.exists() {
            return load_tool_auth_config(&auth_path);
        }

        // Also check for .auth.json directly in tools dir (non-namespaced)
        let flat_auth_path = tools_dir.join(format!("{}.auth.json", tool_name));
        if flat_auth_path.exists() {
            return load_tool_auth_config(&flat_auth_path);
        }

        Ok(None)
    }

    /// Validate required env vars and collect them for injection.
    fn validate_and_inject(config: &ToolAuthConfig) -> Result<Vec<(String, String)>> {
        let mut creds = Vec::new();

        for env_var in &config.required_env_vars {
            match std::env::var(env_var) {
                Ok(value) => {
                    if value.is_empty() {
                        return Err(anyhow::anyhow!(
                            "Environment variable '{}' is set but empty. Please provide a valid credential.",
                            env_var
                        ));
                    }
                    creds.push((env_var.clone(), value));
                }
                Err(_) => {
                    // Build helpful error message with available credentials for context
                    let all_env_vars: Vec<String> = config.required_env_vars.to_vec();
                    return Err(anyhow::anyhow!(
                        "Missing required environment variable '{}' for tool '{:?}'.\nAvailable variables: {}\nPlease set {} to continue.",
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

/// Entry for a discovered resource.
#[derive(Debug, Clone)]
struct ResourceEntry {
    uri: String,
    resource_type: String,
    name: String,
    description: Option<String>,
    mime_type: Option<String>,
    file_path: PathBuf,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new("mcp-cli", "0.1.0")
    }
}

impl McpServer {
    /// Create a new MCP server with the given name and version.
    pub fn new(name: &str, version: &str) -> Self {
        let subscription_manager: std::sync::Arc<
            dyn crate::protocol::ResourceManager + Send + Sync,
        > = std::sync::Arc::new(crate::protocol::MemorySubscriptionManager::new());

        Self {
            name: name.to_string(),
            version: version.to_string(),
            capabilities: ServerCapabilities::new(),
            tools_dir: None,
            cached_tools: Mutex::new(HashMap::new()),
            resources_dir: None,
            cached_resources: Mutex::new(Vec::new()),
            prompts_dir: None,
            cached_prompts: Mutex::new(HashMap::new()),
            roots: Mutex::new(Vec::new()),
            subscription_manager,
        }
    }

    /// Add a root directory provided by the client.
    pub fn add_root(&self, uri: String, name: Option<String>) {
        let mut roots = self.roots.lock().unwrap();
        // Avoid duplicates based on URI
        if !roots.iter().any(|r| r.uri == uri) {
            roots.push(Root { uri, _name: name });
        }
    }

    /// Handle initialize request and store client-provided roots.
    async fn handle_initialize(&self, params: &serde_json::Value) -> Result<serde_json::Value> {
        let init_params: InitParams = serde_json::from_value(params.clone())
            .context("Failed to parse initialize parameters")?;

        info!(
            "Received initialize request from {}: {}",
            init_params.client_info.name, init_params.protocol_version
        );

        // Parse and store client-provided root directories
        if let Some(roots_cap) = &init_params.capabilities.roots {
            debug!(
                "Client supports roots listing: {:?}",
                roots_cap.list_changed
            );
        }

        // Store roots provided by the client during initialization.
        // These are paths on the client's machine that the server can access via resources/tools.
        if let Some(ref roots) = init_params.roots {
            info!("Received {} root directory(ies) from client", roots.len());
            for root in roots {
                self.add_root(root.uri.clone(), root.name.clone());
            }
        }

        // Validate protocol version (simplified check)
        if !init_params.protocol_version.starts_with("2024-") {
            return Err(anyhow::anyhow!("Unsupported protocol version"));
        }

        // Validate protocol version (simplified check)
        if !init_params.protocol_version.starts_with("2024-") {
            return Err(anyhow::anyhow!("Unsupported protocol version"));
        }

        let result = InitResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: self.capabilities.clone(),
            server_info: Implementation {
                name: self.name.clone(),
                version: self.version.clone(),
            },
        };

        Ok(serde_json::to_value(result)?)
    }

    /// Enable tools capability.
    pub fn enable_tools(self) -> Self {
        let mut s = self;
        s.capabilities.tools = Some(true);
        s
    }

    /// Set the tools directory path for dynamic tool discovery.
    pub fn enable_tools_dir(mut self, path: PathBuf) -> Self {
        self.tools_dir = Some(path);
        self
    }

    /// Load tools from the tools directory.
    fn load_tools(&self) -> Result<HashMap<String, ToolDefinition>> {
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

            // Skip directories and non-executable files
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

            // Skip files without execute permission (basic check on Unix)
            #[cfg(unix)]
            {
                use std::os::unix::prelude::*;
                let mode = metadata.permissions().mode();
                if mode & 0o111 == 0 {
                    continue;
                }
            }

            // Use filename without extension as tool name
            let name = match path.file_stem() {
                Some(stem) => stem.to_string_lossy().to_string(),
                None => continue,
            };

            // Try to load auth config for this tool
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

    /// Load resources from the resources directory.
    fn load_resources(&self) -> Result<Vec<ResourceEntry>> {
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

            // Skip directories
            if !path.is_file() {
                continue;
            }

            // Read filename without extension as resource name
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

    /// Detect MIME type from file extension.
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

    /// Set the resources directory path for dynamic resource discovery.
    pub fn enable_resources_dir(mut self, path: PathBuf) -> Self {
        self.resources_dir = Some(path);
        self
    }

    /// Enable resources capability with listChanged flag.
    pub fn enable_resources(mut self, list_changed: bool) -> Self {
        self.capabilities.resources = Some(ResourcesCapability { list_changed });
        self
    }

    /// Enable prompts capability.
    pub fn enable_prompts(mut self) -> Self {
        self.capabilities.prompts = Some(true);
        self
    }

    /// Set the prompts directory path for dynamic prompt discovery.
    pub fn enable_prompts_dir(mut self, path: PathBuf) -> Self {
        self.prompts_dir = Some(path);
        self
    }

    /// Load prompts from the prompts directory.
    fn load_prompts(&self) -> Result<HashMap<String, PromptEntry>> {
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

            // Skip directories and non-JSON files
            if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            // Read and parse the prompt file
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
                },
            );
        }

        info!("Loaded {} prompts", prompts.len());
        Ok(prompts)
    }

    /// Run the server loop on stdio transport.
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

                    // Parse and respond to the request
                    match self.handle_request(&line, initialized).await {
                        Ok(response) => {
                            let _ = tokio::io::stdout()
                                .write_all(format!("{}\n", response).as_bytes())
                                .await;
                            let _ = tokio::io::stdout().flush().await;

                            // After successful initialize, mark as initialized
                            if !initialized && response.contains("capabilities") {
                                initialized = true;
                            }
                        }
                        Err(e) => {
                            error!("Error processing message: {}", e);
                            let err_resp = json!({
                                "jsonrpc": "2.0",
                                "error": JsonRpcError::internal_error(&e.to_string()),
                                "id": null,
                            });
                            let _ = tokio::io::stdout()
                                .write_all(
                                    format!("{}\n", serde_json::to_string(&err_resp).unwrap())
                                        .as_bytes(),
                                )
                                .await;
                        }
                    }

                    // Update initialized state based on response
                    if !initialized && self.capabilities.tools == Some(true) {
                        initialized = true;
                    }
                }
                Ok(None) => {
                    // EOF reached
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

    /// Handle a request and return the response.
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
            Ok(resp) => json!({
                "jsonrpc": "2.0",
                "result": resp,
                "id": request.id_value,
            }),
            Err(e) => {
                let err_resp = JsonRpcError::internal_error(&e.to_string());
                json!({
                    "jsonrpc": "2.0",
                    "error": err_resp,
                    "id": request.id_value,
                })
            }
        }
        .to_string())
    }

    /// Route requests to appropriate handlers.
    async fn route_request(
        &self,
        method: &str,
        params: &serde_json::Value,
        initialized: bool,
    ) -> Result<serde_json::Value> {
        match method {
            "initialize" => self.handle_initialize(params).await,
            "resources/list" => self.handle_resources_list().await,
            "resources/subscribe" => self.handle_resources_subscribe(params).await,
            "resources/unsubscribe" => self.handle_resources_unsubscribe(params).await,
            _ if !initialized => Err(anyhow::anyhow!("Server not initialized")),
            "initialized" => Ok(json!({})),
            "ping" => Ok(json!({})),
            "roots/list" => {
                if !initialized {
                    return Err(anyhow::anyhow!("Server not initialized"));
                }
                self.handle_roots_list().await
            }
            "tools/list" => self.handle_tools_list().await,
            "tools/call" => self.handle_tools_call(params).await,
            "resources/read" => self.handle_resources_read(params).await,
            "prompts/list" => self.handle_prompts_list().await,
            "prompts/get" => self.handle_prompts_get(params).await,
            "notifications/initialized" => Ok(json!({})),
            _ => Err(anyhow::anyhow!("Unknown method: {}", method)),
        }
    }

    /// Handle tools/list request.
    async fn handle_tools_list(&self) -> Result<serde_json::Value> {
        let mut cached = self.cached_tools.lock().unwrap();

        // Load tools from directory if not already cached and directory is configured
        if cached.is_empty() && self.tools_dir.is_some() {
            *cached = self.load_tools()?;
        }

        let tool_list: Vec<_> = cached
            .values()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                })
            })
            .collect();

        Ok(json!({ "tools": tool_list }))
    }

    /// Handle resources/list request.
    async fn handle_resources_list(&self) -> Result<serde_json::Value> {
        let mut cached = self.cached_resources.lock().unwrap();

        // Load resources from directory if not already cached and directory is configured
        if cached.is_empty() && self.resources_dir.is_some() {
            *cached = self.load_resources()?;
        }

        let resource_list: Vec<_> = cached
            .iter()
            .map(|r| {
                json!({
                    "uri": r.uri,
                    "type": r.resource_type,
                    "name": r.name,
                    "description": r.description,
                    "mimeType": r.mime_type,
                })
            })
            .collect();

        Ok(json!({ "resources": resource_list }))
    }

    /// Handle roots/list request - return client-provided root directories.
    async fn handle_roots_list(&self) -> Result<serde_json::Value> {
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

    /// Handle prompts/list request.
    async fn handle_prompts_list(&self) -> Result<serde_json::Value> {
        let mut cached = self.cached_prompts.lock().unwrap();

        // Load prompts from directory if not already cached and directory is configured
        if cached.is_empty() && self.prompts_dir.is_some() {
            *cached = self.load_prompts()?;
        }

        let prompt_list: Vec<_> = cached
            .values()
            .map(|p| {
                json!({
                    "name": p.name,
                    "description": p.description,
                    "arguments": p.arguments.as_ref().map(|args| {
                        args.iter().map(|a| json!({
                            "name": a.name,
                            "required": a.required.unwrap_or(false),
                        })).collect::<Vec<_>>()
                    }),
                })
            })
            .collect();

        Ok(json!({ "prompts": prompt_list }))
    }

    /// Handle prompts/get request.
    async fn handle_prompts_get(&self, params: &serde_json::Value) -> Result<serde_json::Value> {
        let get_params: crate::protocol::GetPromptParams =
            serde_json::from_value(params.clone())
                .context("Failed to parse prompt get parameters")?;

        // Look up the prompt in cache or try to load it
        let entry = {
            let cached = self.cached_prompts.lock().unwrap();
            if let Some(prompt) = cached.get(&get_params.name) {
                prompt.clone()
            } else {
                drop(cached);
                // Reload prompts and look again
                let mut cached = self.cached_prompts.lock().unwrap();
                *cached = self.load_prompts()?;
                match cached.get(&get_params.name) {
                    Some(prompt) => prompt.clone(),
                    None => return Err(anyhow::anyhow!("Prompt '{}' not found", get_params.name)),
                }
            }
        };

        // Validate required arguments if prompt has them
        if let Some(ref required_args) = entry.arguments {
            crate::protocol::validate_prompt_arguments(&get_params.arguments, required_args)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }

        // Read the prompt template file
        let content = std::fs::read_to_string(&entry.file_path)?;
        let prompt_file: crate::protocol::PromptFile = serde_json::from_str(&content)?;

        // Render templates with provided arguments
        let engine = crate::protocol::PromptTemplateEngine::new();
        let base_dir = entry.file_path.parent();

        let messages: Vec<crate::protocol::PromptMessage> = match prompt_file.messages {
            Some(messages) => messages
                .into_iter()
                .map(|msg| {
                    // Render each message content
                    let rendered_content = match &msg.content {
                        crate::protocol::PromptMessageContentValue::Array(items) => {
                            crate::protocol::PromptMessageContentValue::Array(
                                items
                                    .iter()
                                    .cloned()
                                    .map(|item| {
                                        match item {
                                            crate::protocol::PromptMessageContentItem::Text {
                                                text,
                                            } => {
                                                let rendered = engine
                                                    .render(&text, &get_params.arguments, base_dir)
                                                    .unwrap_or_else(|e| {
                                                        format!("[Render error: {}]", e)
                                                    });
                                                crate::protocol::PromptMessageContentItem::Text {
                                                    text: rendered,
                                                }
                                            }
                                            other => other, // Keep non-text items as-is
                                        }
                                    })
                                    .collect(),
                            )
                        }
                        crate::protocol::PromptMessageContentValue::Text(text) => {
                            let rendered = engine
                                .render(text, &get_params.arguments, base_dir)
                                .unwrap_or_else(|e| format!("[Render error: {}]", e));
                            crate::protocol::PromptMessageContentValue::Text(rendered)
                        }
                    };

                    crate::protocol::PromptMessage {
                        role: msg.role,
                        content_value: rendered_content,
                    }
                })
                .collect(),
            None => {
                return Err(anyhow::anyhow!(
                    "Prompt '{}' has no messages",
                    get_params.name
                ));
            }
        };

        let result = crate::protocol::GetPromptResult {
            description: entry.description,
            messages,
        };
        Ok(json!(result))
    }

    /// Handle tools/call request.
    async fn handle_tools_call(&self, params: &serde_json::Value) -> Result<serde_json::Value> {
        let call_params: CallToolParams = serde_json::from_value(params.clone())
            .context("Failed to parse tool call parameters")?;

        // Look up the tool in cache or try to load it
        let (script_path, auth_config) = {
            let cached = self.cached_tools.lock().unwrap();
            if let Some(tool) = cached.get(&call_params.name) {
                (tool.script_path.clone(), tool.auth_config.clone())
            } else {
                drop(cached);
                // Reload tools and look again
                let mut cached = self.cached_tools.lock().unwrap();
                *cached = self.load_tools()?;
                match cached.get(&call_params.name) {
                    Some(tool) => (tool.script_path.clone(), tool.auth_config.clone()),
                    None => return Err(anyhow::anyhow!("Tool '{}' not found", call_params.name)),
                }
            }
        };

        // Resolve credentials if auth is configured for this tool
        if let Some(ref _config) = auth_config
            && let Some(ref tools_dir) = self.tools_dir
        {
            match CredentialResolver::resolve_for_tool(tools_dir, &call_params.name) {
                Ok(creds) => {
                    debug!(
                        "Resolved {} credential(s) for tool '{}'",
                        creds.len(),
                        call_params.name
                    );

                    // Validate credentials are present before proceeding
                    if !creds.is_empty() {
                        info!(
                            "Credentials validated successfully for tool '{}'",
                            call_params.name
                        );
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Credential resolution failed for tool '{}': {}",
                        call_params.name,
                        e
                    ));
                }
            }
        }

        // Prepare the input JSON for the script
        let input = json!({
            "name": call_params.name,
            "arguments": call_params.arguments,
        });

        debug!(
            "Executing tool from {:?} with input: {}",
            script_path, input
        );

        // Spawn the script process
        let mut child = tokio::process::Command::new(&script_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Failed to spawn tool script")?;

        // Write input to stdin and capture output/error
        let result = {
            let mut stdin = child.stdin.take().context("Failed to open stdin")?;
            use tokio::io::AsyncWriteExt;
            stdin.write_all(input.to_string().as_bytes()).await?;
            drop(stdin); // Close stdin to signal EOF

            // Read stdout and stderr concurrently
            let mut stdout = child.stdout.take().context("Failed to open stdout")?;
            let mut stderr = child.stderr.take().context("Failed to open stderr")?;

            let (stdout_res, stderr_res) = tokio::join!(
                async {
                    use tokio::io::AsyncReadExt;
                    let mut output = String::new();
                    stdout.read_to_string(&mut output).await?;
                    anyhow::Result::<String>::Ok(output)
                },
                async {
                    use tokio::io::AsyncReadExt;
                    let mut error_output = String::new();
                    stderr.read_to_string(&mut error_output).await?;
                    anyhow::Result::<String>::Ok(error_output)
                }
            );

            match (stdout_res, stderr_res) {
                (Ok(out), Ok(err)) => {
                    if !err.is_empty() {
                        debug!("Tool stderr: {}", err);
                    }
                    out
                }
                (Err(e), _) | (_, Err(e)) => {
                    return Err(anyhow::anyhow!("Failed to read output: {}", e));
                }
            }
        };

        // Wait for process to complete
        let status = child.wait().await.context("Failed to wait on tool")?;

        debug!("Tool '{}' exited with status: {}", call_params.name, status);

        if !status.success() {
            return Err(anyhow::anyhow!(
                "Tool '{}' failed with exit code: {:?}",
                call_params.name,
                status.code()
            ));
        }

        Ok(json!({
            "content": [
                {
                    "type": "text",
                    "text": result.trim()
                }
            ]
        }))
    }

    /// Handle resources/read request.
    async fn handle_resources_read(&self, params: &serde_json::Value) -> Result<serde_json::Value> {
        // Extract resource URI from parameters
        let uri_value = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'uri' parameter"))?;

        info!("Reading resource: {}", uri_value);

        // First check cache, reload if empty
        let mut cached = self.cached_resources.lock().unwrap();

        if cached.is_empty() {
            *cached = self.load_resources()?;
            info!("Reloaded {} resources", cached.len());
        }

        let entry = cached.iter().find(|r| r.uri == uri_value).cloned();

        if let Some(entry) = entry {
            info!("Found resource: {:?}", entry.file_path);

            // Read the file contents
            let content = std::fs::read_to_string(&entry.file_path)?;

            Ok(json!({
                "contents": [
                    {
                        "uri": entry.uri,
                        "text": content,
                        "mimeType": entry.mime_type,
                    }
                ]
            }))
        } else {
            Err(anyhow::anyhow!("Resource '{}' is not available", uri_value))
        }
    }

    /// Handle resources/subscribe request.
    async fn handle_resources_subscribe(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let subscribe_params: crate::protocol::SubscribeResourceParams =
            serde_json::from_value(params.clone())
                .context("Failed to parse resources/subscribe parameters")?;

        info!("Subscribing to resource: {}", subscribe_params.uri);

        // Check if resource exists
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

        // Subscribe to the resource
        let was_new = self.subscription_manager.subscribe(&subscribe_params.uri);

        if was_new {
            info!("Successfully subscribed to: {}", subscribe_params.uri);
        } else {
            debug!("Already subscribed to: {}", subscribe_params.uri);
        }

        Ok(json!({}))
    }

    /// Handle resources/unsubscribe request.
    async fn handle_resources_unsubscribe(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let unsubscribe_params: crate::protocol::UnsubscribeResourceParams =
            serde_json::from_value(params.clone())
                .context("Failed to parse resources/unsubscribe parameters")?;

        info!("Unsubscribing from resource: {}", unsubscribe_params.uri);

        // Check if resource exists first
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

        // Unsubscribe from the resource
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

/// Server with tools capability enabled (for builder pattern).
pub struct McpServerWithTools {
    inner: McpServer,
}

impl McpServerWithTools {
    pub fn run(self) -> McpServer {
        self.inner
    }
}

/// Builder for configuring the MCP server.
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
