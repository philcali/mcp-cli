//! Prompt listing and retrieval handlers.

use crate::protocol::*;
use anyhow::{Context, Result};
use serde_json::json;

/// List available prompts.
pub async fn handle_prompts_list(server: &crate::server::McpServer) -> Result<serde_json::Value> {
    let mut cached = server.cached_prompts.lock().unwrap();

    if cached.is_empty() && server.prompts_dir.is_some() {
        *cached = server.load_prompts()?;
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

/// Get prompt with provided arguments.
pub async fn handle_prompts_get(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let get_params: GetPromptParams =
        serde_json::from_value(params.clone()).context("Failed to parse prompt get parameters")?;

    let entry = match server.get_prompt(&get_params.name)? {
        Some(entry) => entry,
        None => return Err(anyhow::anyhow!("Prompt '{}' not found", get_params.name)),
    };

    if let Some(ref required_args) = entry.arguments {
        validate_prompt_arguments(&Some(get_params.arguments.clone()), required_args)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    let content = std::fs::read_to_string(&entry.file_path)?;
    let prompt_file: PromptFile = serde_json::from_str(&content)?;

    let engine = crate::protocol::PromptTemplateEngine::new();
    let base_dir = entry.file_path.parent();

    let messages: Vec<PromptMessage> = match prompt_file.messages {
        Some(messages) => messages
            .into_iter()
            .map(|msg| {
                let rendered_content = match &msg.content {
                    PromptMessageContentValue::Array(items) => PromptMessageContentValue::Array(
                        items
                            .iter()
                            .cloned()
                            .map(|item| match item {
                                PromptMessageContentItem::Text { text } => {
                                    let rendered = engine
                                        .render(&text, &get_params.arguments, base_dir)
                                        .unwrap_or_else(|e| format!("[Render error: {}]", e));
                                    PromptMessageContentItem::Text { text: rendered }
                                }
                                other => other,
                            })
                            .collect(),
                    ),
                    PromptMessageContentValue::Text(text) => {
                        let rendered = engine
                            .render(text, &get_params.arguments, base_dir)
                            .unwrap_or_else(|e| format!("[Render error: {}]", e));
                        PromptMessageContentValue::Text(rendered)
                    }
                };

                PromptMessage {
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

    let result = GetPromptResult {
        description: entry.description,
        messages,
    };
    Ok(json!(result))
}

/// Validate prompt arguments against required parameters.
pub fn validate_prompt_arguments(
    provided: &Option<std::collections::HashMap<String, serde_json::Value>>,
    required: &[PromptArgument],
) -> Result<()> {
    for arg in required {
        if arg.required.unwrap_or(false)
            && !provided.as_ref().is_none_or(|p| p.contains_key(&arg.name))
        {
            return Err(anyhow::anyhow!("Missing required argument: {}", arg.name));
        }
    }

    Ok(())
}
