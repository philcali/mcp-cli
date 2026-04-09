# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`mcp-cli` is a minimal Model Context Protocol (MCP) server implementation using stdio transport with JSON-RPC 2.0. It's written in Rust and follows a short-lived, synchronous process model where each invocation handles exactly one request-response cycle.

## Build & Test Commands

```bash
# Build the project
cargo build --release

# Run tests
cargo test

# Run a single test
cargo test -- test_initialize

# Check formatting
cargo fmt

# Run clippy lints
cargo clippy
```

## Architecture Overview

The codebase uses a minimal architecture with four main modules:

**src/main.rs** - Entry point that initializes the tracing subscriber and starts the MCP server. Uses builder pattern to enable tools capability via `enable_tools()`.

**src/server.rs** - Core server implementation containing:
- `McpServer` struct with name, version, and capabilities
- Request routing logic in `route_request()` method
- Handler methods for each MCP method (initialize, ping, tools/list, etc.)
- Stdio communication via tokio's async I/O

**src/protocol.rs** - All protocol types including:
- JSON-RPC 2.0 request/response structures (`JsonRpcRequest`, `JsonRpcResponse`)
- Error handling (`JsonRpcError` with standard error codes)
- MCP-specific types (capabilities, tools, resources, prompts)
- Prompt template engine and rendering utilities
- Content types for tool results and resource contents

**src/lib.rs** - Library root exposing protocol and server modules. Note: the tools module is not exposed publicly despite existing in codebase.

## Request Flow

1. Server reads complete JSON line from stdin
2. Parses as `JsonRpcRequest`
3. Routes to handler based on method name
4. Writes response to stdout
5. Exits (or continues loop for subsequent requests)

Initialization state is tracked via the `initialized` flag - some methods require initialization first.

## Key Patterns

- **Builder pattern**: Server capabilities configured during construction (`enable_tools()`, `enable_resources()`)
- **Error handling**: Uses `anyhow::Result` throughout, with internal errors reported as JSON-RPC error responses
- **Logging via tracing**: Logs to stderr only (never stdout - that's the protocol stream!)

## Tools Discovery

The server supports dynamic tool discovery from a configured tools directory:

1. Set a tools directory using `ServerBuilder.with_tools_dir(path)` or `McpServer.enable_tools_dir(path)`
2. The server scans for executable files in that directory
3. Each executable becomes an available tool, named by its filename (without extension)
4. Tools receive JSON input via stdin with this format:
   ```json
   {"name": "tool-name", "arguments": {...}}
   ```
5. Tool output is captured from stdout and returned as text content

Tools are cached after first discovery for subsequent `tools/list` calls.

## Prompt Discovery

The server supports dynamic prompt discovery from a configured prompts directory:

1. Set a prompts directory using `ServerBuilder.with_prompts_dir(path)` or CLI flag `--prompts-dir <path>`
2. The server scans for `.json` files in that directory
3. Each JSON file becomes an available prompt, identified by its filename (without extension)
4. Prompt templates support variable substitution: `{{variable}}`
5. Template directives are supported: `{{#include path}}`, `{{#env VAR}}`

Prompt files follow the MCP prompts specification with fields for name, description, arguments, and messages. Prompts are cached after first discovery.

See [PROMPTS.md](PROMPTS.md) for detailed usage documentation.

## Testing

Integration tests in `tests/integration_test.rs` spawn actual server processes and communicate via stdin/stdout pipes. Tests verify:
- Error responses before initialization (`test_ping_before_initialize`)
- Successful initialize flow (`test_initialize`)
- Method not found errors (`test_unknown_method`)
- Unimplemented endpoints return errors (`test_tools_call_not_implemented`)

Run tests with `cargo test` from the repository root.
