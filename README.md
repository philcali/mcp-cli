# mcp-cli

A minimal Model Context Protocol (MCP) server implementation for CLI environments using stdio transport.

## Overview

`mcp-cli` is a short-lived, synchronous MCP server that communicates via stdin/stdout using JSON-RPC 2.0. Each invocation handles exactly one request-response cycle, making it ideal for composable CLI tools and shell workflows.

## Features

- **JSON-RPC 2.0 compliant** - Follows the official specification
- **stdio transport** - Communicates via standard input/output streams
- **Short-lived process model** - Each invocation is independent
- **Tools capability** - Supports MCP tool discovery with dynamic script loading
- **Resources capability** - Supports MCP resource discovery from a configured directory

## Installation

### Build from source

```bash
cd mcp-cli
cargo build --release
```

The binary will be available at `target/release/mcp-cli`.

### Requirements

- Rust 1.75+
- Cargo

## Usage

### Basic invocation

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"my-client","version":"1.0"}}}' | ./target/release/mcp-cli
```

### Supported methods

| Method | Description | Before Initialize? |
|--------|-------------|-------------------|
| `initialize` | Initialize the MCP session | No (required first) |
| `initialized` | Notification that client is initialized | No |
| `ping` | Ping the server | No |
| `tools/list` | List available tools | No |
| `tools/call` | Call a tool script | No |
| `resources/list` | List available resources | No |
| `resources/read` | Read a resource file | No |

### Tool scripts

The server can discover and execute tool scripts from a configurable directory:

```bash
./target/release/mcp-cli /path/to/tools
```

**Tool requirements:**
- Executable files (shell scripts, binaries, etc.)
- Filename without extension becomes the tool name
- Scripts receive JSON input via stdin: `{"name": "tool-name", "arguments": {...}}`
- Output to stdout is returned as the tool result
- Exit code 0 = success, non-zero = error reported to client

**Example tool script (`aws-s3-list.sh`):**
```bash
#!/bin/bash
# AWS S3 bucket list wrapper
input=$(cat)
bucket=$(echo "$input" | jq -r '.arguments.bucket // empty')

if [ -n "$bucket" ]; then
    aws s3 ls "s3://$bucket/"
else
    aws s3 ls
fi
```

**Calling the tool:**
```json
{"name": "aws-s3-list", "arguments": {"bucket": "my-bucket"}}
```

Tools are cached after first discovery for performance.

### Resource files

The server can discover and serve resource files from a configurable directory:

```bash
./target/release/mcp-cli /path/to/tools /path/to/resources
```

Or configure via the builder pattern in code:
```rust
McpServer::default()
    .enable_tools()
    .enable_tools_dir(PathBuf::from("/path/to/tools"))
    .enable_resources(true)  // Enable resources capability with listChanged=true
    .enable_resources_dir(PathBuf::from("/path/to/resources"))
```

**Resource requirements:**
- Regular files in the configured directory
- Filename without extension becomes the resource name
- URI is constructed as `file://<absolute-path>`
- MIME type is auto-detected from file extension

**Supported MIME types:**
| Extension | MIME Type |
|-----------|-----------|
| `.txt`, `.text` | `text/plain` |
| `.md` | `text/markdown` |
| `.json` | `application/json` |
| `.xml` | `application/xml` |
| `.yaml`, `.yml` | `application/yaml` |
| `.toml` | `application/toml` |
| `.rs` | `text/x-rust` |
| `.sh` | `application/x-sh` |
| `.py` | `text/x-python` |

**Example resource file (`config.json`):**
```json
{"region": "us-east-1", "environment": "production"}
```

**Reading the resource:**
```bash
echo '{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"file:///path/to/resources/config.json"}}' | ./target/release/mcp-cli
```

Resources are cached after first discovery for performance.

### Response format

Success response:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocol_version": "2024-11-05",
    "capabilities": {
      "tools": true
    },
    "server_info": {
      "name": "mcp-cli",
      "version": "0.1.0"
    }
  }
}
```

Error response:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32603,
    "message": "Server not initialized"
  }
}
```

## Testing


### Manual testing with curl

```bash
curl -s --unix-socket /tmp/mcp.sock \
  -X POST -d '{"jsonrpc":"2.0","id":1,"method":"ping"}' \
  http://localhost/ping
```

## Protocol compliance

This server implements:
- JSON-RPC 2.0 specification
- MCP protocol version `2024-11-05`
- Server capabilities: tools and resources

## Architecture

```
┌─────────────┐     stdin      ┌──────────┐     stdout     ┌─────────┐
│   Client    │ ◄────────────► │ mcp-cli  │ ◄────────────► │ JSON-RPC│
│ (JSON-RPC)  │               │ (stdio)  │                │ Server │
└─────────────┘                └──────────┘                └─────────┘
```

Each process handles exactly one request:
1. Read complete JSON line from stdin
2. Parse and route to handler
3. Write response to stdout
4. Exit

## License

MIT
