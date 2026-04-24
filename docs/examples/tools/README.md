# mcp-cli Example Tools

This directory contains example tool scripts demonstrating common patterns for building tools that work with mcp-cli.

## Quick Start

```bash
mcp-cli --tools-dir ./docs/examples/tools
```

## Available Tools

| Tool | Description | Auth Strategy |
|------|-------------|---------------|
| [curl-request.sh](curl-request.sh) | HTTP client with method, headers, body support | None / bearer_token / api_key |
| [file-manager.sh](file-manager.sh) | File operations (read, write, list, delete) | None |
| [weather.sh](weather.sh) | External API call with bearer token auth | bearer_token |
| [db-query.sh](db-query.sh) | Database query tool with API key auth | api_key |
| [deploy.sh](deploy.sh) | Deployment tool with OAuth2 client credentials | oauth2 |
| [image-info.sh](image-info.sh) | Demonstrates text + image content types | None |
| [env-inspector.sh](env-inspector.sh) | Shows env var resolution and credential injection | env_var |

## Tool Patterns

### Argument Handling
All tools read JSON from stdin with the format:
```json
{"name": "tool-name", "arguments": {"arg1": "value1"}}
```

### Error Handling
- Exit non-zero for failures (stderr captured by mcp-cli)
- Return JSON error messages for structured errors

### Auth Config
Tools with auth requirements use a `.auth.json` file:
```bash
# Flat file pattern
my-tool.auth.json

# Directory pattern
my-tool/.auth.json
```

### Content Types
- **Text**: Default — return via stdout
- **Image**: Return base64-encoded data with MIME type in structured JSON
- **Multiple**: Return array of content objects

## Writing Your Own Tool

1. Create a script with a shebang (`#!/bin/bash`, `#!/usr/bin/env python3`, etc.)
2. Read JSON input from stdin
3. Process arguments from `.arguments` field
4. Write output to stdout
5. Make it executable: `chmod +x my-tool.sh`
6. (Optional) Add a `.auth.json` for authentication

See individual tool files for detailed comments and usage examples.
