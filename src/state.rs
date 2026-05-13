//! Server state management.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tracing::info;

use crate::auth;
use crate::auth::TokenCache;
use crate::discovery::prompts::{PromptEntry, discover_prompts};
use crate::discovery::resources::{ResourceEntry, discover_resources};
use crate::discovery::tools::{ToolDefinition, discover_tools};
use crate::protocol::{ClientCapabilities, LogLevel, MemorySubscriptionManager, ResourceManager};

pub struct ServerState {
    pub name: String,
    pub version: String,
    pub tools_dir: Option<PathBuf>,
    pub cached_tools: Arc<Mutex<HashMap<String, ToolDefinition>>>,
    pub resources_dir: Option<PathBuf>,
    pub cached_resources: Arc<Mutex<Vec<ResourceEntry>>>,
    pub prompts_dir: Option<PathBuf>,
    pub cached_prompts: Arc<Mutex<HashMap<String, PromptEntry>>>,
    pub roots: Arc<Mutex<Vec<crate::protocol::Root>>>,
    pub subscription_manager: Arc<dyn ResourceManager + Send + Sync>,
    /// Whether the server has been successfully initialized
    pub initialized: AtomicBool,
    /// OAuth2 token cache shared across all requests
    pub oauth_cache: TokenCache,
    /// Client's capabilities from the initialize request (interior-mutable for HTTP transport)
    pub client_capabilities: Arc<Mutex<Option<ClientCapabilities>>>,
    /// Resource templates directory
    pub resource_templates_dir: Option<PathBuf>,
    /// Cached resource templates
    pub cached_resource_templates:
        Arc<Mutex<Vec<crate::discovery::resources::ResourceTemplateEntry>>>,
    /// Task manager for task-augmented requests
    pub task_manager: std::sync::Arc<crate::task_manager::TaskManager>,
    /// Current minimum log level for logging/messages (set by client via logging/setLevel)
    pub log_level: std::sync::Arc<std::sync::RwLock<LogLevel>>,
    /// Set of request IDs that have been cancelled via notifications/cancelled.
    /// Handlers can check this to detect cancellation.
    pub cancelled_requests: Arc<Mutex<HashMap<serde_json::Value, String>>>,
}

impl Clone for ServerState {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            version: self.version.clone(),
            tools_dir: self.tools_dir.clone(),
            cached_tools: Arc::clone(&self.cached_tools),
            resources_dir: self.resources_dir.clone(),
            cached_resources: Arc::clone(&self.cached_resources),
            prompts_dir: self.prompts_dir.clone(),
            cached_prompts: Arc::clone(&self.cached_prompts),
            roots: Arc::clone(&self.roots),
            subscription_manager: Arc::clone(&self.subscription_manager),
            initialized: AtomicBool::new(self.initialized.load(Ordering::SeqCst)),
            oauth_cache: self.oauth_cache.clone(),
            client_capabilities: Arc::clone(&self.client_capabilities),
            resource_templates_dir: self.resource_templates_dir.clone(),
            cached_resource_templates: Arc::clone(&self.cached_resource_templates),
            task_manager: Arc::clone(&self.task_manager),
            log_level: Arc::clone(&self.log_level),
            cancelled_requests: Arc::clone(&self.cancelled_requests),
        }
    }
}

impl ServerState {
    pub fn new(name: &str, version: &str) -> Self {
        let subscription_manager: Arc<dyn ResourceManager + Send + Sync> =
            Arc::new(MemorySubscriptionManager::new());

        Self {
            name: name.to_string(),
            version: version.to_string(),
            tools_dir: None,
            cached_tools: Arc::new(Mutex::new(HashMap::new())),
            resources_dir: None,
            cached_resources: Arc::new(Mutex::new(Vec::new())),
            prompts_dir: None,
            cached_prompts: Arc::new(Mutex::new(HashMap::new())),
            roots: Arc::new(Mutex::new(Vec::new())),
            subscription_manager,
            initialized: AtomicBool::new(false),
            oauth_cache: TokenCache::new(),
            client_capabilities: Arc::new(Mutex::new(None)),
            resource_templates_dir: None,
            cached_resource_templates: Arc::new(Mutex::new(Vec::new())),
            task_manager: Arc::new(crate::task_manager::TaskManager::new()),
            log_level: Arc::new(std::sync::RwLock::new(LogLevel::Debug)),
            cancelled_requests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Mark the server as initialized.
    pub fn set_initialized(&self) {
        self.initialized.store(true, Ordering::SeqCst);
    }

    /// Check if the server is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    /// Store client capabilities from the initialize request.
    pub fn set_client_capabilities(&self, caps: ClientCapabilities) {
        let mut caps_guard = self.client_capabilities.lock().unwrap();
        *caps_guard = Some(caps);
    }

    /// Get client capabilities if set during initialization.
    pub fn get_client_capabilities(&self) -> Option<ClientCapabilities> {
        self.client_capabilities.lock().unwrap().clone()
    }

    pub fn with_tools_dir(mut self, path: PathBuf) -> Self {
        self.tools_dir = Some(path);
        self
    }

    pub fn with_resources_dir(mut self, path: PathBuf) -> Self {
        self.resources_dir = Some(path);
        self
    }

    pub fn with_prompts_dir(mut self, path: PathBuf) -> Self {
        self.prompts_dir = Some(path);
        self
    }

    pub async fn load_tools(&self) -> Result<HashMap<String, ToolDefinition>, anyhow::Error> {
        let dir = match &self.tools_dir {
            Some(p) => p,
            None => return Ok(HashMap::new()),
        };
        discover_tools(dir).await
    }

    pub fn load_resources(&self) -> Result<Vec<ResourceEntry>, anyhow::Error> {
        let dir = match &self.resources_dir {
            Some(p) => p,
            None => {
                info!("No resources directory configured");
                return Ok(Vec::new());
            }
        };
        discover_resources(dir)
    }

    pub fn load_prompts(&self) -> Result<HashMap<String, PromptEntry>, anyhow::Error> {
        let dir = match &self.prompts_dir {
            Some(p) => p,
            None => return Ok(HashMap::new()),
        };
        discover_prompts(dir)
    }

    pub fn add_root(&self, uri: String, name: Option<String>) {
        let mut roots = self.roots.lock().unwrap();
        if !roots.iter().any(|r| r.uri == uri) {
            roots.push(crate::protocol::Root { uri, name });
        }
    }

    pub async fn resolve_credentials(
        &self,
        tool_name: &str,
    ) -> Result<std::collections::HashMap<String, String>, anyhow::Error> {
        match &self.tools_dir {
            Some(tools_dir) => {
                auth::resolve_credentials(&self.oauth_cache, tools_dir, tool_name).await
            }
            None => Ok(std::collections::HashMap::new()),
        }
    }

    pub fn invalidate_all_caches(&self) -> Result<(), anyhow::Error> {
        self.cached_tools.lock().unwrap().clear();
        self.cached_resources.lock().unwrap().clear();
        self.cached_prompts.lock().unwrap().clear();
        self.cached_resource_templates.lock().unwrap().clear();
        info!("All caches invalidated");
        Ok(())
    }

    pub fn load_resource_templates(
        &self,
    ) -> Result<Vec<crate::discovery::resources::ResourceTemplateEntry>, anyhow::Error> {
        let dir = match &self.resource_templates_dir {
            Some(p) => p,
            None => {
                info!("No resource templates directory configured");
                return Ok(Vec::new());
            }
        };
        crate::discovery::resources::discover_resource_templates(dir)
    }
}
