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
    /// Supports: {{var}}, {{#include path}}, {{#env VAR}}
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
                let (directive, end_i) = self.parse_directive(&chars, i)?;
                result.push_str(&self.execute_directive(directive, args, base_dir)?);
                i = end_i;
            } else if chars[i] == '{' && i + 1 < len && chars[i + 1] == '{' {
                // Variable: {{var}}
                let (var_name, end_i) = self.parse_variable(&chars, i)?;
                let value = self.resolve_variable(&var_name, args);
                result.push_str(&value);
                i = end_i;
            } else {
                // Regular character
                result.push(chars[i]);
                i += 1;
            }
        }

        Ok(result)
    }

    /// Parse a directive ({{#include ...}} or {{#env VAR}}).
    fn parse_directive(
        &self,
        chars: &[char],
        start: usize,
    ) -> Result<(String, usize), PromptRenderError> {
        let mut i = start + 2; // Skip {{#
        while i < chars.len() && (chars[i] == '}' || !chars[i].is_whitespace()) {
            i += 1;
        }

        let _directive_end = i;
        while i < chars.len() && chars[i] != '}' {
            i += 1;
        }

        if i >= chars.len() {
            return Err(PromptRenderError::UnclosedDirective);
        }

        let content: String = chars[start + 2..i].iter().collect();
        Ok((content, i + 1))
    }

    /// Parse a variable reference ({{var}}).
    fn parse_variable(
        &self,
        chars: &[char],
        start: usize,
    ) -> Result<(String, usize), PromptRenderError> {
        let mut i = start + 2; // Skip {{
        while i < chars.len() && chars[i] != '}' {
            i += 1;
        }

        if i >= chars.len() {
            return Err(PromptRenderError::UnclosedVariable);
        }

        let name: String = chars[start + 2..i].iter().collect();

        // Skip the closing }}
        if i + 1 < chars.len() && chars[i + 1] == '}' {
            Ok((name, i + 2))
        } else {
            Ok((name, i + 1))
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
