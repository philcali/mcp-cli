//! Prompt template engine and argument validation logic.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use super::types::PromptArgument;

/// Simple template engine for prompt rendering.
#[derive(Debug, Default)]
pub struct PromptTemplateEngine;

impl PromptTemplateEngine {
    /// Create a new template engine.
    pub fn new() -> Self {
        Self
    }

    /// Render a template with the given arguments.
    /// Supports: {{var}}, {{var | default "fallback"}}, {{var | upper/lower/truncate N}},
    /// {{#include path}}, {{#env VAR}}, {{#if var}}...{{/if}}, {{#each items as item}}...{{/each}}
    pub fn render(
        &self,
        template: &str,
        args: &HashMap<String, Value>,
        base_dir: Option<&Path>,
    ) -> Result<String, PromptRenderError> {
        let mut result = String::new();
        let chars: Vec<char> = template.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            if chars[i] == '{' && i + 1 < len && chars[i + 1] == '#' {
                // Directive: {{#...}}
                let directive_content = self.read_until_end_tag(&chars, i)?;
                let parts: Vec<&str> = directive_content.split_whitespace().collect();

                if !parts.is_empty() && (parts[0] == "if" || parts[0] == "each") {
                    // Block directive - find matching end tag
                    let block_start = i;
                    let (block_content, end_pos) = self.find_block_end(&chars, i, parts[0])?;
                    let block_str: String = block_content.iter().collect();

                    if parts[0] == "if" {
                        let var_name = parts.get(1).unwrap_or(&"").to_string();
                        let condition = self.is_truthy(&var_name, args);
                        if condition {
                            let rendered = self.render(&block_str, args, base_dir)?;
                            result.push_str(&rendered);
                        }
                    } else if parts[0] == "each" {
                        let array_name = parts.get(1).unwrap_or(&"").to_string();
                        let item_name = parts.get(3).unwrap_or(&"item").to_string(); // "each array as item"

                        if let Some(Value::Array(items)) = args.get(&array_name) {
                            for (idx, item) in items.iter().enumerate() {
                                let mut item_args = args.clone();
                                item_args.insert(item_name.clone(), item.clone());
                                item_args.insert("index".to_string(), Value::Number(idx.into()));
                                let rendered = self.render(&block_str, &item_args, base_dir)?;
                                result.push_str(&rendered);
                            }
                        }
                    }

                    i = end_pos + block_start;
                } else {
                    // Simple directive
                    let rendered =
                        self.execute_directive(directive_content.clone(), args, base_dir)?;
                    result.push_str(&rendered);
                    // Skip past the closing }}
                    i += 2 + directive_content.len() + 2;
                    if i > len {
                        i = len;
                    }
                }
            } else if chars[i] == '{' && i + 1 < len && chars[i + 1] == '{' {
                // Variable: {{var}} or {{var | filter}}
                let var_expr = self.read_variable_expr(&chars, i)?;
                let value = self.resolve_variable_with_filters(&var_expr, args);
                result.push_str(&value);
                i += 2 + var_expr.len() + 2;
                if i > len {
                    i = len;
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }

        Ok(result)
    }

    /// Read content between {{ and }}.
    fn read_until_end_tag(
        &self,
        chars: &[char],
        start: usize,
    ) -> Result<String, PromptRenderError> {
        let mut i = start + 2;
        let mut found_close = false;
        while i < chars.len() {
            if chars[i] == '}' && i + 1 < chars.len() && chars[i + 1] == '}' {
                found_close = true;
                break;
            }
            i += 1;
        }
        if !found_close {
            return Err(PromptRenderError::UnclosedDirective);
        }
        let content: String = chars[start + 2..i].iter().collect();
        Ok(content)
    }

    /// Read variable expression between {{ and }}.
    fn read_variable_expr(
        &self,
        chars: &[char],
        start: usize,
    ) -> Result<String, PromptRenderError> {
        let mut i = start + 2;
        let mut found_close = false;
        while i < chars.len() {
            if chars[i] == '}' && i + 1 < chars.len() && chars[i + 1] == '}' {
                found_close = true;
                break;
            }
            i += 1;
        }
        if !found_close {
            return Err(PromptRenderError::UnclosedVariable);
        }
        let content: String = chars[start + 2..i].iter().collect();
        Ok(content)
    }

    /// Find the end of a block directive and return the content.
    fn find_block_end<'a>(
        &self,
        chars: &'a [char],
        start: usize,
        block_type: &str,
    ) -> Result<(&'a [char], usize), PromptRenderError> {
        let mut depth = 1;
        let mut i = start + 1;

        while i < chars.len() {
            if chars[i] == '{' && i + 1 < chars.len() && chars[i + 1] == '{' {
                let inner = self.read_until_end_tag(chars, i).unwrap_or_default();
                let trimmed = inner.trim();

                if trimmed.starts_with(&format!("#{}", block_type)) {
                    depth += 1;
                } else if trimmed == format!("/{}", block_type) {
                    depth -= 1;
                    if depth == 0 {
                        return Ok((&chars[start + 1..i], i));
                    }
                }
            }
            i += 1;
        }

        Err(PromptRenderError::UnclosedBlock(block_type.to_string()))
    }

    /// Check if a value is "truthy" for conditionals.
    fn is_truthy(&self, name: &str, args: &HashMap<String, Value>) -> bool {
        match args.get(name) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Null) => false,
            Some(Value::String(s)) => !s.is_empty(),
            Some(Value::Array(a)) => !a.is_empty(),
            Some(_) => true,
            None => false,
        }
    }

    /// Resolve a variable with optional filters.
    fn resolve_variable_with_filters(&self, expr: &str, args: &HashMap<String, Value>) -> String {
        let parts: Vec<&str> = expr.splitn(2, '|').collect();
        let var_name = parts[0].trim();
        let mut value = self.resolve_variable(var_name, args);

        if parts.len() == 2 {
            let filter = parts[1].trim();
            value = self.apply_filter(&value, filter, args, var_name);
        }

        value
    }

    /// Apply a filter to a value.
    fn apply_filter(
        &self,
        value: &str,
        filter: &str,
        _args: &HashMap<String, Value>,
        var_name: &str,
    ) -> String {
        let filter_parts: Vec<&str> = filter.splitn(2, ' ').collect();
        let filter_name = filter_parts[0];

        match filter_name {
            "default" => {
                if value.starts_with("{{") && value.ends_with("}}") {
                    if filter_parts.len() > 1 {
                        let default_val = filter_parts[1].trim();
                        default_val.trim_matches('"').trim_matches('\'').to_string()
                    } else {
                        String::new()
                    }
                } else {
                    value.to_string()
                }
            }
            "upper" => value.to_uppercase(),
            "lower" => value.to_lowercase(),
            "truncate" => {
                let max_len = filter_parts
                    .get(1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(100);
                if value.len() > max_len {
                    format!("{}...", &value[..max_len.saturating_sub(3)])
                } else {
                    value.to_string()
                }
            }
            _ => {
                format!("{{{{{} | {}}}}}", var_name, filter)
            }
        }
    }

    /// Execute a directive.
    fn execute_directive(
        &self,
        directive: String,
        _args: &HashMap<String, Value>,
        base_dir: Option<&Path>,
    ) -> Result<String, PromptRenderError> {
        let parts: Vec<&str> = directive.split_whitespace().collect();
        if parts.is_empty() {
            return Err(PromptRenderError::InvalidDirective(directive));
        }

        match parts[0] {
            "include" => {
                let path_str = parts
                    .get(1)
                    .ok_or_else(|| PromptRenderError::MissingArgument("include".to_string()))?;
                let base = base_dir.unwrap_or(Path::new("."));
                let full_path = base.join(path_str);
                std::fs::read_to_string(&full_path).map_err(|e| PromptRenderError::FileReadError {
                    path: path_str.to_string(),
                    error: e.to_string(),
                })
            }
            "env" => {
                let var_name = parts
                    .get(1)
                    .ok_or_else(|| PromptRenderError::MissingArgument("env".to_string()))?;
                std::env::var(var_name)
                    .map_err(|_| PromptRenderError::EnvVarNotFound(var_name.to_string()))
            }
            _ => Err(PromptRenderError::UnknownDirective(parts[0].to_string())),
        }
    }

    /// Resolve a variable from arguments.
    fn resolve_variable(&self, name: &str, args: &HashMap<String, Value>) -> String {
        match args.get(name) {
            Some(Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => format!("{{{{{}}}}}", name), // Keep as literal if not found
        }
    }
}

/// Error type for template rendering.
#[derive(Debug, Clone)]
pub enum PromptRenderError {
    UnclosedDirective,
    UnclosedVariable,
    UnclosedBlock(String),
    InvalidDirective(String),
    UnknownDirective(String),
    MissingArgument(String),
    FileReadError { path: String, error: String },
    EnvVarNotFound(String),
}

impl std::fmt::Display for PromptRenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PromptRenderError::UnclosedDirective => write!(f, "Unclosed directive"),
            PromptRenderError::UnclosedVariable => write!(f, "Unclosed variable"),
            PromptRenderError::UnclosedBlock(block) => write!(f, "Unclosed block: {}", block),
            PromptRenderError::InvalidDirective(d) => write!(f, "Invalid directive: {}", d),
            PromptRenderError::UnknownDirective(d) => write!(f, "Unknown directive: {}", d),
            PromptRenderError::MissingArgument(d) => {
                write!(f, "Missing argument for directive: {}", d)
            }
            PromptRenderError::FileReadError { path, error } => {
                write!(f, "Failed to read file '{}': {}", path, error)
            }
            PromptRenderError::EnvVarNotFound(var) => {
                write!(f, "Environment variable '{}' not found", var)
            }
        }
    }
}

impl std::error::Error for PromptRenderError {}

/// Validate prompt arguments against required parameters.
pub fn validate_prompt_arguments(
    args: &HashMap<String, Value>,
    required_args: &[PromptArgument],
) -> Result<(), String> {
    let required_names: Vec<&str> = required_args
        .iter()
        .filter(|a| a.required == Some(true))
        .map(|a| a.name.as_str())
        .collect();

    for name in required_names {
        if !args.contains_key(name) {
            return Err(format!("Missing required argument: {}", name));
        }
    }

    Ok(())
}
