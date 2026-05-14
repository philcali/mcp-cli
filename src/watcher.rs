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

/// Callback type for resource updated notifications.
/// The closure receives the file path that changed.
pub type ResourceUpdatedCallback = Box<dyn Fn(&std::path::Path) + Send + Sync>;

/// Run the common file-watching loop: set up notify watcher, watch directory,
/// and dispatch change events to callbacks.
///
/// *`label`* is used in log messages (e.g. "prompts", "tools").
/// *`on_event`* is called for every file change event. It receives the list of
///  changed paths so the caller can decide what callbacks to fire.
async fn run_watcher<F>(dir: PathBuf, config: WatchConfig, label: &str, on_event: F)
where
    F: Fn(&[PathBuf]),
{
    if !config.watch_for_changes {
        warn!("{} file watching is disabled", label);
        return;
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<notify::Result<Event>>(100);

    let mut watcher_instance =
        match notify::recommended_watcher(move |res: notify::Result<Event>| {
            let _ = tx.blocking_send(res);
        }) {
            Ok(w) => w,
            Err(e) => {
                error!("Failed to create {} file watcher: {}", label, e);
                return;
            }
        };

    if let Err(e) = watcher_instance.watch(&dir, RecursiveMode::Recursive) {
        error!("Failed to watch {} directory {:?}: {}", label, dir, e);
        return;
    }

    info!("Started watching {} directory: {:?}", label, dir);

    while let Some(res) = rx.recv().await {
        match res {
            Ok(event) => {
                if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                    for path in &event.paths {
                        info!("{} file change detected: {:?}", label, path);
                    }
                    on_event(&event.paths);
                }
            }
            Err(e) => {
                error!("Watch error: {}", e);
            }
        }
    }
}

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

    /// Notify that a specific resource file was updated.
    fn on_updated(&self, path: &std::path::Path);
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

    fn on_updated(&self, _path: &std::path::Path) {}
}

impl PromptWatcher {
    async fn watch_directory(dir: PathBuf, watcher: &PromptWatcher, config: WatchConfig) {
        run_watcher(dir, config, "prompts", |_| {
            watcher.on_change();
            watcher.on_list_changed();
        })
        .await;
    }
}

/// Watcher for tool files.
pub struct ToolWatcher;

impl Default for ToolWatcher {
    fn default() -> Self {
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

        let watcher = ToolWatcher;
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

    fn on_updated(&self, _path: &std::path::Path) {}
}

impl ToolWatcher {
    async fn watch_directory(dir: PathBuf, watcher: &ToolWatcher, config: WatchConfig) {
        run_watcher(dir, config, "tools", |_| {
            watcher.on_list_changed();
        })
        .await;
    }
}

/// Watcher for resource files.
pub struct ResourceWatcher;

impl Default for ResourceWatcher {
    fn default() -> Self {
        Self
    }
}

impl ResourceWatcher {
    pub fn start_watching(
        dir: PathBuf,
        config: WatchConfig,
        on_change: CacheInvalidateCallback,
        on_list_changed: ListChangedCallback,
        on_updated: ResourceUpdatedCallback,
    ) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>> {
        if !config.watch_for_changes {
            warn!("Resource file watching is disabled in configuration");
            return Ok(std::sync::Arc::new(tokio::task::spawn(async {})));
        }

        let watch_config = config.clone();

        let handle = tokio::task::spawn(async move {
            Self::watch_directory(dir, on_change, on_list_changed, on_updated, watch_config).await;
        });

        Ok(std::sync::Arc::new(handle))
    }

    async fn watch_directory(
        dir: PathBuf,
        on_change: CacheInvalidateCallback,
        on_list_changed: ListChangedCallback,
        on_updated: ResourceUpdatedCallback,
        config: WatchConfig,
    ) {
        run_watcher(dir, config, "resources", |paths: &[PathBuf]| {
            (on_change)();
            (on_list_changed)();
            for path in paths {
                if path.is_file() {
                    (on_updated)(path);
                }
            }
        })
        .await;
    }
}

/// Watcher for resource template files.
pub struct ResourceTemplateWatcher {
    on_change: CacheInvalidateCallback,
    on_list_changed: ListChangedCallback,
}

impl ResourceTemplateWatcher {
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
}

impl FileSystemWatcher for ResourceTemplateWatcher {
    fn start_watching(
        dir: PathBuf,
        config: WatchConfig,
        _on_change: CacheInvalidateCallback,
        _on_list_changed: ListChangedCallback,
    ) -> Result<std::sync::Arc<tokio::task::JoinHandle<()>>> {
        if !config.watch_for_changes {
            warn!("Resource template file watching is disabled in configuration");
            return Ok(std::sync::Arc::new(tokio::task::spawn(async {})));
        }

        let watcher = ResourceTemplateWatcher::new(_on_change, _on_list_changed);
        let watch_config = config.clone();

        let handle = tokio::task::spawn(async move {
            Self::watch_directory(dir, &watcher, watch_config).await;
        });

        Ok(std::sync::Arc::new(handle))
    }

    fn on_change(&self) {
        debug!("Resource template cache invalidated due to file change");
        (self.on_change)();
    }

    fn on_list_changed(&self) {
        debug!("Resource template list changed notification emitted");
        (self.on_list_changed)();
    }

    fn on_updated(&self, _path: &std::path::Path) {}
}

impl ResourceTemplateWatcher {
    async fn watch_directory(dir: PathBuf, watcher: &ResourceTemplateWatcher, config: WatchConfig) {
        run_watcher(dir, config, "resource templates", |_| {
            watcher.on_change();
            watcher.on_list_changed();
        })
        .await;
    }
}
