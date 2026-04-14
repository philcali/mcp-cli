//! Prompt discovery logic.

use crate::protocol::PromptFile;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct PromptEntry {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Option<Vec<crate::protocol::PromptArgument>>,
    pub file_path: PathBuf,
    pub loaded_at: std::time::Instant,
}

/// Discover prompts from a directory.
pub fn discover_prompts(prompts_dir: &PathBuf) -> Result<HashMap<String, PromptEntry>> {
    if !prompts_dir.exists() {
        warn!("Prompts directory does not exist: {:?}", prompts_dir);
        return Ok(HashMap::new());
    }

    let mut prompts = HashMap::new();

    for entry in std::fs::read_dir(prompts_dir)? {
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

        let prompt_file: PromptFile = match serde_json::from_str(&content) {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to parse prompt file {:?}: {}", path, e);
                continue;
            }
        };

        let name = prompt_file.name.clone();
        prompts.insert(
            name.clone(),
            PromptEntry {
                name: name.clone(),
                description: prompt_file.description,
                arguments: prompt_file.arguments,
                file_path: path.clone(),
                loaded_at: std::time::Instant::now(),
            },
        );

        debug!("Discovered prompt: {}", name);
    }

    info!("Discovered {} prompts", prompts.len());
    Ok(prompts)
}
