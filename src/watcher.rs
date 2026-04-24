//! Unified file system watcher for MCP CLI resources.
//!
//! Provides a shared abstraction for watching tools, prompts, and resources directories
//! for file changes using the `notify` crate.

use anyhow::Result;
use notify::{Event, RecursiveMode, Watcher};
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

/// Configuration for file watching.
#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub watch_for_changes: bool,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            watch_for_changes: true,
        }
    }
}

/// Callback type for cache invalidation.
pub type CacheInvalidateCallback = Box<dyn Fn() + Send + Sync>;

/// Callback type for list changed notifications.
pub type ListChangedCallback = Box<dyn Fn() + Send + Sync>;

/// Unified file system watcher trait.
pub trait FileSystemWatcher: Send + Sync {
    /// Start watching a directory for changes.
    fn start_watching(
        dir: PathBuf,
        config: WatchConfig,
        on_change: CacheInvalidateCallback,
        on_list_changed: ListChangedCallback,
    ) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>>;

    /// Invalidate cache when file changes are detected.
    fn on_change(&self);

    /// Notify that the list of items has changed.
    fn on_list_changed(&self);
}

/// Watcher for prompt files.
pub struct PromptWatcher {
    on_change: CacheInvalidateCallback,
    on_list_changed: ListChangedCallback,
}

impl PromptWatcher {
    pub fn new<F, G>(on_change: F, on_list_changed: G) -> Self
    where
        F: Fn() + Send + Sync + 'static,
        G: Fn() + Send + Sync + 'static,
    {
        Self {
            on_change: Box::new(on_change),
            on_list_changed: Box::new(on_list_changed),
        }
    }

    /// Invalidate the prompt cache when file changes are detected.
    pub fn on_change(&self) {
        debug!("Prompt cache invalidated due to file change");
        (self.on_change)();
    }

    /// Notify that the prompt list has changed.
    pub fn on_list_changed(&self) {
        debug!("Prompt list changed notification emitted");
        (self.on_list_changed)();
    }
}

impl FileSystemWatcher for PromptWatcher {
    fn start_watching(
        dir: PathBuf,
        config: WatchConfig,
        _on_change: CacheInvalidateCallback,
        _on_list_changed: ListChangedCallback,
    ) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>> {
        if !config.watch_for_changes {
            warn!("Prompt file watching is disabled in configuration");
            return Ok(std::sync::Arc::new(tokio::task::spawn(async {})));
        }

        let watcher = PromptWatcher::new(_on_change, _on_list_changed);
        let watch_config = config.clone();

        let handle = tokio::task::spawn(async move {
            Self::watch_directory(dir, &watcher, watch_config).await;
        });

        Ok(std::sync::Arc::new(handle))
    }

    fn on_change(&self) {
        self.on_change();
    }

    fn on_list_changed(&self) {
        self.on_list_changed();
    }
}

impl PromptWatcher {
    async fn watch_directory(dir: PathBuf, watcher: &PromptWatcher, config: WatchConfig) {
        if !config.watch_for_changes {
            warn!("Prompt file watching is disabled");
            return;
        }

        let (tx, mut rx) = tokio::sync::mpsc::channel::<notify::Result<Event>>(100);

        let mut watcher_instance =
            match notify::recommended_watcher(move |res: notify::Result<Event>| {
                let _ = tx.blocking_send(res);
            }) {
                Ok(w) => w,
                Err(e) => {
                    error!("Failed to create prompt file watcher: {}", e);
                    return;
                }
            };

        if let Err(e) = watcher_instance.watch(&dir, RecursiveMode::Recursive) {
            error!("Failed to watch prompts directory {:?}: {}", dir, e);
            return;
        }

        info!("Started watching prompts directory: {:?}", dir);

        while let Some(res) = rx.recv().await {
            match res {
                Ok(event) => {
                    if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                        for path in &event.paths {
                            info!("Prompt file change detected: {:?}", path);
                        }
                        watcher.on_change();
                        watcher.on_list_changed();
                    }
                }
                Err(e) => {
                    error!("Watch error: {}", e);
                }
            }
        }
    }
}

/// Watcher for tool files.
pub struct ToolWatcher;

impl Default for ToolWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolWatcher {
    pub fn new() -> Self {
        Self
    }
}

impl FileSystemWatcher for ToolWatcher {
    fn start_watching(
        dir: PathBuf,
        config: WatchConfig,
        _on_change: CacheInvalidateCallback,
        _on_list_changed: ListChangedCallback,
    ) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>> {
        if !config.watch_for_changes {
            warn!("Tool file watching is disabled in configuration");
            return Ok(std::sync::Arc::new(tokio::task::spawn(async {})));
        }

        let watcher = ToolWatcher::new();
        let watch_config = config.clone();

        let handle = tokio::task::spawn(async move {
            Self::watch_directory(dir, &watcher, watch_config).await;
        });

        Ok(std::sync::Arc::new(handle))
    }

    fn on_change(&self) {}

    fn on_list_changed(&self) {
        debug!("Tool list changed notification emitted");
    }
}

impl ToolWatcher {
    async fn watch_directory(dir: PathBuf, watcher: &ToolWatcher, config: WatchConfig) {
        if !config.watch_for_changes {
            warn!("Tool file watching is disabled");
            return;
        }

        let (tx, mut rx) = tokio::sync::mpsc::channel::<notify::Result<Event>>(100);

        let mut watcher_instance =
            match notify::recommended_watcher(move |res: notify::Result<Event>| {
                let _ = tx.blocking_send(res);
            }) {
                Ok(w) => w,
                Err(e) => {
                    error!("Failed to create tool file watcher: {}", e);
                    return;
                }
            };

        if let Err(e) = watcher_instance.watch(&dir, RecursiveMode::Recursive) {
            error!("Failed to watch tools directory {:?}: {}", dir, e);
            return;
        }

        info!("Started watching tools directory: {:?}", dir);

        while let Some(res) = rx.recv().await {
            match res {
                Ok(event) => {
                    if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                        for path in &event.paths {
                            info!("Tool file change detected: {:?}", path);
                        }
                        watcher.on_list_changed();
                    }
                }
                Err(e) => {
                    error!("Watch error: {}", e);
                }
            }
        }
    }
}

/// Watcher for resource files.
pub struct ResourceWatcher;

impl Default for ResourceWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceWatcher {
    pub fn new() -> Self {
        Self
    }
}

impl FileSystemWatcher for ResourceWatcher {
    fn start_watching(
        dir: PathBuf,
        config: WatchConfig,
        _on_change: CacheInvalidateCallback,
        _on_list_changed: ListChangedCallback,
    ) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>> {
        if !config.watch_for_changes {
            warn!("Resource file watching is disabled in configuration");
            return Ok(std::sync::Arc::new(tokio::task::spawn(async {})));
        }

        let watcher = ResourceWatcher::new();
        let watch_config = config.clone();

        let handle = tokio::task::spawn(async move {
            Self::watch_directory(dir, &watcher, watch_config).await;
        });

        Ok(std::sync::Arc::new(handle))
    }

    fn on_change(&self) {}

    fn on_list_changed(&self) {
        debug!("Resource list changed notification emitted");
    }
}

impl ResourceWatcher {
    async fn watch_directory(dir: PathBuf, watcher: &ResourceWatcher, config: WatchConfig) {
        if !config.watch_for_changes {
            warn!("Resource file watching is disabled");
            return;
        }

        let (tx, mut rx) = tokio::sync::mpsc::channel::<notify::Result<Event>>(100);

        let mut watcher_instance =
            match notify::recommended_watcher(move |res: notify::Result<Event>| {
                let _ = tx.blocking_send(res);
            }) {
                Ok(w) => w,
                Err(e) => {
                    error!("Failed to create resource file watcher: {}", e);
                    return;
                }
            };

        if let Err(e) = watcher_instance.watch(&dir, RecursiveMode::Recursive) {
            error!("Failed to watch resources directory {:?}: {}", dir, e);
            return;
        }

        info!("Started watching resources directory: {:?}", dir);

        while let Some(res) = rx.recv().await {
            match res {
                Ok(event) => {
                    if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                        for path in &event.paths {
                            info!("Resource file change detected: {:?}", path);
                        }
                        watcher.on_list_changed();
                    }
                }
                Err(e) => {
                    error!("Watch error: {}", e);
                }
            }
        }
    }
}

/// Unified event manager that coordinates all watchers.
pub struct EventManager {
    prompt_handle: Option<std::sync::Arc<tokio::task::JoinHandle<()>>>,
    tool_handle: Option<std::sync::Arc<tokio::task::JoinHandle<()>>>,
    resource_handle: Option<std::sync::Arc<tokio::task::JoinHandle<()>>>,
}

impl EventManager {
    pub fn new() -> Self {
        Self {
            prompt_handle: None,
            tool_handle: None,
            resource_handle: None,
        }
    }

    /// Start watching prompts directory.
    pub fn start_prompt_watching(
        &mut self,
        dir: PathBuf,
        config: WatchConfig,
        on_change: CacheInvalidateCallback,
        on_list_changed: ListChangedCallback,
    ) -> Result<()> {
        if self.prompt_handle.is_some() {
            warn!("Prompt watcher already started");
            return Ok(());
        }

        let handle = PromptWatcher::start_watching(dir, config, on_change, on_list_changed)?;
        self.prompt_handle = Some(handle);
        Ok(())
    }

    /// Start watching tools directory.
    pub fn start_tool_watching(
        &mut self,
        dir: PathBuf,
        config: WatchConfig,
        on_change: CacheInvalidateCallback,
        on_list_changed: ListChangedCallback,
    ) -> Result<()> {
        if self.tool_handle.is_some() {
            warn!("Tool watcher already started");
            return Ok(());
        }

        let handle = ToolWatcher::start_watching(dir, config, on_change, on_list_changed)?;
        self.tool_handle = Some(handle);
        Ok(())
    }

    /// Start watching resources directory.
    pub fn start_resource_watching(
        &mut self,
        dir: PathBuf,
        config: WatchConfig,
        on_change: CacheInvalidateCallback,
        on_list_changed: ListChangedCallback,
    ) -> Result<()> {
        if self.resource_handle.is_some() {
            warn!("Resource watcher already started");
            return Ok(());
        }

        let handle = ResourceWatcher::start_watching(dir, config, on_change, on_list_changed)?;
        self.resource_handle = Some(handle);
        Ok(())
    }

    /// Stop all watchers.
    pub fn stop_all(&mut self) {
        self.prompt_handle = None;
        self.tool_handle = None;
        self.resource_handle = None;
    }
}

impl Default for EventManager {
    fn default() -> Self {
        Self::new()
    }
}
