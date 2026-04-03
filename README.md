# mcp-cli

A minimal Model Context Protocol (MCP) server implementation for CLI environments using stdio transport.

## Overview

`mcp-cli` is a short-lived, synchronous MCP server that communicates via stdin/stdout using JSON-RPC 2.0. Each invocation handles exactly one request-response cycle, making it ideal for composable CLI tools and shell workflows.

## Features

- **JSON-RPC 2.0 compliant** - Follows the official specification
- **stdio transport** - Communicates via standard input/output streams
- **Short-lived process model** - Each invocation is independent
- **Tools capability** - Supports MCP tool discovery (currently returns empty list)
- **Resources capability** - Supports MCP resource discovery (currently returns empty list)

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
| `tools/call` | Call a tool | No (not implemented) |
| `resources/list` | List available resources | No |
| `resources/read` | Read a resource | No (not implemented) |

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
- Server capabilities (tools only)

Not implemented:
- `tools/call` - Returns error when called
- `resources/read` - Returns error when called (use with resources/list to discover available resources)

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
