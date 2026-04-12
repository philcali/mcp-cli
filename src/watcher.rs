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

/// Unified file system watcher trait.
pub trait FileSystemWatcher: Send + Sync {
    /// Start watching a directory for changes.
    fn start_watching(
        dir: PathBuf,
        config: WatchConfig,
        on_change: CacheInvalidateCallback,
    ) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>>;

    /// Invalidate cache when file changes are detected.
    fn on_change(&self);
}

/// Watcher for prompt files.
pub struct PromptWatcher {
    on_change: CacheInvalidateCallback,
}

impl PromptWatcher {
    pub fn new<F>(on_change: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        Self {
            on_change: Box::new(on_change),
        }
    }

    /// Invalidate the prompt cache when file changes are detected.
    pub fn on_change(&self) {
        debug!("Prompt cache invalidated due to file change");
        (self.on_change)();
    }
}

impl FileSystemWatcher for PromptWatcher {
    fn start_watching(
        dir: PathBuf,
        config: WatchConfig,
        _on_change: CacheInvalidateCallback,
    ) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>> {
        if !config.watch_for_changes {
            warn!("Prompt file watching is disabled in configuration");
            return Ok(std::sync::Arc::new(tokio::task::spawn(async {})));
        }

        let watcher = PromptWatcher::new(_on_change);
        let watch_config = config.clone();

        let handle = tokio::task::spawn(async move {
            Self::watch_directory(dir, &watcher, watch_config).await;
        });

        Ok(std::sync::Arc::new(handle))
    }

    fn on_change(&self) {
        self.on_change();
    }
}

impl PromptWatcher {
    async fn watch_directory(dir: PathBuf, _watcher: &PromptWatcher, config: WatchConfig) {
        if !config.watch_for_changes {
            warn!("Prompt file watching is disabled in configuration");
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
                        _watcher.on_change();
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
}

impl ToolWatcher {
    async fn watch_directory(dir: PathBuf, _watcher: &ToolWatcher, config: WatchConfig) {
        if !config.watch_for_changes {
            warn!("Tool file watching is disabled in configuration");
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
                        _watcher.on_change();
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
}

impl EventManager {
    pub fn new() -> Self {
        Self {
            prompt_handle: None,
            tool_handle: None,
        }
    }

    /// Start watching prompts directory.
    pub fn start_prompt_watching(
        &mut self,
        dir: PathBuf,
        config: WatchConfig,
        on_change: CacheInvalidateCallback,
    ) -> Result<()> {
        if self.prompt_handle.is_some() {
            warn!("Prompt watcher already started");
            return Ok(());
        }

        let handle = PromptWatcher::start_watching(dir, config, on_change)?;
        self.prompt_handle = Some(handle);
        Ok(())
    }

    /// Start watching tools directory.
    pub fn start_tool_watching(
        &mut self,
        dir: PathBuf,
        config: WatchConfig,
        _on_change: CacheInvalidateCallback,
    ) -> Result<()> {
        if self.tool_handle.is_some() {
            warn!("Tool watcher already started");
            return Ok(());
        }

        let handle = ToolWatcher::start_watching(dir, config, _on_change)?;
        self.tool_handle = Some(handle);
        Ok(())
    }

    /// Stop all watchers.
    pub fn stop_all(&mut self) {
        self.prompt_handle = None;
        self.tool_handle = None;
    }
}

impl Default for EventManager {
    fn default() -> Self {
        Self::new()
    }
}
