# MCP CLI Prompt Support

This document describes how to use the prompt feature in mcp-cli.

## Overview

Prompts are reusable message templates for LLMs. The MCP protocol provides two methods:
- `prompts/list` - Discover available prompt templates
- `prompts/get` - Retrieve a specific prompt with optional arguments

## Configuration

Enable prompts support by providing the `--prompts-dir` flag:

```bash
mcp-cli --prompts-dir ~/.mcp/prompts
```

Or via builder API:

```rust
let server = ServerBuilder::new("my-server", "1.0")
    .with_prompts()
    .with_prompts_dir(PathBuf::from("~/.mcp/prompts"))
    .build();
```

## Prompt File Format

Prompts are defined as JSON files with the following structure:

```json
{
  "name": "prompt-name",
  "description": "What this prompt does",
  "arguments": [
    {
      "name": "arg_name",
      "required": true,
      "description": "Argument description"
    }
  ],
  "messages": [
    {
      "role": "system",
      "content": "System message template"
    },
    {
      "role": "user",
      "content": "User message template with {{variable}} substitution"
    }
  ]
}
```

### Required Fields

- `name` - The prompt identifier (used in `prompts/get`)
- `messages` - Array of messages defining the conversation structure

### Optional Fields

- `description` - Human-readable description shown in `prompts/list`
- `arguments` - List of arguments with validation metadata

## Message Roles

Messages support three roles:
- `system` - System/instruction messages (highest priority)
- `user` - User input messages
- `assistant` - Assistant response examples (few-shot prompting)

## Template Substitution

Prompt content supports variable substitution using `{{variable}}` syntax:

```json
{
  "messages": [
    {
      "role": "user",
      "content": "Review this {{language}} code:\n\n{{code}}"
    }
  ]
}
```

When calling the prompt, provide arguments matching these variable names:

```json
{
  "name": "code-review",
  "arguments": {
    "language": "rust",
    "code": "fn main() { ... }"
  }
}
```

## Template Directives

Advanced templates support directives for dynamic content:

### Include File Contents

`{{#include path}}` - Reads and includes file contents at runtime:

```json
{
  "messages": [
    {
      "role": "user",
      "content": "Here is the code to review:\n\n{{#include /path/to/code.txt}}"
    }
  ]
}
```

### Environment Variables

`{{#env VAR_NAME}}` - Injects environment variable values:

```json
{
  "messages": [
    {
      "role": "system",
      "content": "You are reviewing code in the {{#env PROJECT_NAME}} project."
    }
  ]
}
```

## Example Prompts

See `examples/prompts/` for working examples:

- **code-review** - Code review assistant with language and code arguments
- **debug-error** - Error debugging helper with error message and context
- **write-commit-msg** - Semantic commit message generator

## API Usage

### Listing Available Prompts

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "prompts/list"
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "prompts": [
      {
        "name": "code-review",
        "description": "Review code for bugs and improvements",
        "arguments": [
          {"name": "language", "required": false},
          {"name": "code", "required": true}
        ]
      }
    ]
  }
}
```

### Getting a Prompt with Arguments

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "prompts/get",
  "params": {
    "name": "code-review",
    "arguments": {
      "language": "rust",
      "code": "fn main() { println!(\"Hello\"); }"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "description": "Review code for bugs and improvements",
    "messages": [
      {
        "role": "system",
        "content": "You are an expert code reviewer..."
      },
      {
        "role": "user",
        "content": "Please review this rust code:\n\nfn main() { println!(\"Hello\"); }"
      }
    ]
  }
}
```

## Error Handling

- **Missing required arguments**: Returns error with message about which argument is missing
- **Unknown prompt name**: Returns "Prompt not found" error
- **Template render errors**: Failed substitutions are replaced with `[Render error: ...]` placeholder text
