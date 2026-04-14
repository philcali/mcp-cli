//! Server state management.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::info;

use crate::auth;
use crate::discovery::prompts::{PromptEntry, discover_prompts};
use crate::discovery::resources::{ResourceEntry, discover_resources};
use crate::discovery::tools::{ToolDefinition, discover_tools};
use crate::protocol::{MemorySubscriptionManager, ResourceManager};

pub struct ServerState {
    pub name: String,
    pub version: String,
    pub tools_dir: Option<PathBuf>,
    pub cached_tools: Arc<Mutex<HashMap<String, ToolDefinition>>>,
    pub resources_dir: Option<PathBuf>,
    pub cached_resources: Mutex<Vec<ResourceEntry>>,
    pub prompts_dir: Option<PathBuf>,
    pub cached_prompts: Arc<Mutex<HashMap<String, PromptEntry>>>,
    pub roots: Mutex<Vec<crate::server::Root>>,
    pub subscription_manager: Arc<dyn ResourceManager + Send + Sync>,
}

impl Clone for ServerState {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            version: self.version.clone(),
            tools_dir: self.tools_dir.clone(),
            cached_tools: Arc::clone(&self.cached_tools),
            resources_dir: self.resources_dir.clone(),
            cached_resources: Mutex::new(self.cached_resources.lock().unwrap().clone()),
            prompts_dir: self.prompts_dir.clone(),
            cached_prompts: Arc::clone(&self.cached_prompts),
            roots: Mutex::new(self.roots.lock().unwrap().clone()),
            subscription_manager: Arc::clone(&self.subscription_manager),
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
            cached_resources: Mutex::new(Vec::new()),
            prompts_dir: None,
            cached_prompts: Arc::new(Mutex::new(HashMap::new())),
            roots: Mutex::new(Vec::new()),
            subscription_manager,
        }
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

    pub fn load_tools(&self) -> Result<HashMap<String, ToolDefinition>, anyhow::Error> {
        let dir = match &self.tools_dir {
            Some(p) => p,
            None => return Ok(HashMap::new()),
        };
        discover_tools(dir)
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
            roots.push(crate::server::Root { uri, name });
        }
    }

    pub async fn resolve_credentials(
        &self,
        tool_name: &str,
    ) -> Result<std::collections::HashMap<String, String>, anyhow::Error> {
        match &self.tools_dir {
            Some(tools_dir) => auth::resolve_credentials(tools_dir, tool_name),
            None => Ok(std::collections::HashMap::new()),
        }
    }

    pub fn invalidate_all_caches(&self) -> Result<(), anyhow::Error> {
        self.cached_tools.lock().unwrap().clear();
        self.cached_resources.lock().unwrap().clear();
        self.cached_prompts.lock().unwrap().clear();
        info!("All caches invalidated");
        Ok(())
    }
}
