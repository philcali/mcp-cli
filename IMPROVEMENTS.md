# MCP CLI Improvement Ideas

This document lists potential improvements for the mcp-cli server.

## Quick Wins

### 1. Fix `tools/call` Implementation
**Status**: ✅ Completed

The `tools/call` handler existed but lacked proper testing. 

**What was done:**
- Added comprehensive integration tests verifying:
  - Successful tool execution with arguments
  - Proper error handling when tool doesn't exist
  - JSON input/output format correctness

### 2. Missing MCP Protocol Methods  
**Status**: ✅ Completed

Added support for additional MCP protocol endpoints:

**New method added:**
- `notifications/initialized` - Notification endpoint as per MCP spec

**New protocol types:**
- `ListToolsParams` with optional `tool_names` filtering
- `ListToolsResult` for the response structure  
- `ToolsListChangedNotification` for subscription-based updates

### 3. Streaming Support
**Status**: ✅ Completed

Implemented streaming support for `tools/call` and `resources/read` methods using JSON-RPC notifications.

**What was done:**
- Added `stream: bool` parameter to tool call and resource read requests
- When enabled, server immediately returns acknowledgment with `stream_id`
- Server sends output chunks as notifications: `{"jsonrpc":"2.0","method":"tools/stream","params":{...}}`
- Final `done` chunk signals stream completion

**Supported methods with streaming:**
- `tools/call?stream=true` - Tool output streamed line-by-line
- `resources/read?stream=true` - File contents streamed line-by-line

**Usage example (tool):**
```json
// Client requests streaming:
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "my-tool",
    "arguments": {"key": "value"},
    "stream": true
  },
  "id": "req-123"
}

// Server responds immediately:
{
  "jsonrpc": "2.0",
  "result": {
    "content": [],
    "is_error": false,
    "stream_id": "stream_my-tool_..."
  },
  "id": "req-123"
}

// Followed by streaming chunks (notifications from server):
{
  "jsonrpc": "2.0",
  "method": "tools/stream",
  "params": {
    "request_id": "req-123",
    "chunk": {"type": "content", "data": "first line\n"}
  }
}
```

**Implementation details:**
- Uses `tokio::sync::broadcast` for notification channel
- Background task polls channel and sends to stdout
- Line-by-line streaming avoids buffering large outputs

### 4. Prompt Caching/Invalidation  
**Status**: ✅ Completed

Implemented comprehensive prompt caching with TTL and file watching:

**What was done:**
- **TTL-based cache expiration**: Prompts cached for configurable duration (default 5 minutes)
  - Each prompt entry tracks `loaded_at` timestamp
  - Cache automatically refreshed when prompts accessed after TTL expires
  - Configurable via `PromptCacheConfig::ttl_secs`
  
- **File system watching**: Background watcher detects prompt file changes
  - Uses `notify` crate for cross-platform file monitoring
  - Automatically invalidates cache on file modify/create/remove events
  - Can be disabled via `watch_for_changes` config flag
  
- **Manual cache invalidation**: `invalidate_prompt_cache()` method available
  - Forces reload of all prompts on next access
  - Useful for triggering immediate refresh after bulk operations

**Configuration:**
```rust
let server = McpServer::new("server", "1.0.0")
    .with_prompt_cache_config(PromptCacheConfig {
        ttl_secs: 300,           // Cache duration in seconds (default: 300)
        watch_for_changes: true, // Enable file watching (default: true)
    });

// Start background watcher
let _watcher = server.start_prompt_watcher()?;

// Manual invalidation
server.invalidate_prompt_cache()?;
```

**Test coverage:**
- `test_prompt_cache_ttl` – Verifies TTL-based expiration works correctly
- `test_prompt_cache_invalidation` – Tests manual cache clearing

### 5. Resource Subscriptions
**Status**: ✅ Completed

Implemented full subscription support for MCP resources using an in-memory manager.

**What was done:**
- Added protocol types: `SubscribeResourceParams`, `UnsubscribeResourceParams`
- Implemented `MemorySubscriptionManager` with `ResourceManager` trait
- Server handlers:
  - `resources/subscribe` - Subscribe to a resource URI (validates existence)
  - `resources/unsubscribe` - Unsubscribe from a resource URI
- Proper error handling for non-existent resources
- Clean separation between subscription tracking and resource access

**MCP spec methods implemented:**
- ✅ `resources/subscribe` - Subscribe to resource change notifications
- ✅ `resources/unsubscribe` - Unsubscribe from changes

**Test coverage:**
Added 5 comprehensive integration tests:
- `test_resources_subscribe_valid_resource` – Successful subscription
- `test_resources_subscribe_nonexistent_resource` – Error on missing resource
- `test_resources_unsubscribe_valid_resource` – Unsubscribe flow
- `test_resources_unsubscribe_nonexistent_resource` – Error handling
- `test_resources_subscribe_and_read` – Combined workflow (subscribe then read)

### 6. Tool Execution Improvements
**Status**: ✅ Completed

Implemented enhanced tool execution capabilities:

**What was done:**
- **Timeout support**: Tools now have a default 30-second timeout to prevent hanging
  - Process is killed if it exceeds the timeout
  - Clear error message indicating which tool timed out
  
- **Separated stdout/stderr**: Tool output now properly separates standard output from errors
  - `stdout` returned in main result content
  - `stderr` captured and included separately (when non-empty)
  - Failure messages include stderr for better debugging

**Example response with stderr:**
```json
{
  "content": [{"type": "text", "text": "success"}],
  "stderr": "warning: deprecated function used"
}
```

**Timeout behavior:**
- Default timeout: 30 seconds (configurable via `TOOL_TIMEOUT_SECS` constant)
- On timeout: process killed, error returned with clear message
- Error messages now include stderr output for failed tools

**Test coverage:**
All existing tests pass. Timeout and stderr separation verified through integration testing.

### 7. Unified File System Watcher
**Status**: ✅ Completed

Created unified file system watcher module that provides shared infrastructure for watching tools, prompts, and resources directories.

**What was done:**
- **`src/watcher.rs` module**: Centralized file watching abstraction using `notify` crate
  - `FileSystemWatcher` trait with consistent interface across all resource types
  - `WatchConfig` configuration struct for enabling/disabling watchers
  - Separate `PromptWatcher` and `ToolWatcher` implementations
  
- **Unified event handling**: Both tools and prompts now use the same watching infrastructure
  - Automatic cache invalidation when files are modified/created/deleted
  - Cross-platform support via `notify`'s recommended watcher
  - Background async tasks for non-blocking file monitoring
  
- **Shared configuration**: Watchers can be enabled/disabled independently per directory type

**Integration:**
```rust
// Start watching tools directory
let tool_handle = server.start_tool_watcher()?;

// Start watching prompts directory  
let prompt_handle = server.start_prompt_watcher()?;
```

**Test coverage:**
Watchers verified through integration testing. Cache invalidation triggers on file changes.

### 8. Server Monolith Refactor & Modularization
**Status**: ✅ Completed

Refactored server.rs into a modular architecture with clear separation of concerns:

**What was done:**
- **Routing module** (`src/routing.rs`): Centralized request routing with explicit method→handler mapping
  - `route_request()` function delegates to handler modules
  - `KNOWN_METHODS` constant for introspection and documentation
  
- **Handler modules** (in `src/handlers/`):
  - `init.rs`: Initialize connection and capability negotiation
  - `tools.rs`: Tool listing and execution
  - `resources.rs`: Resource CRUD and subscriptions
  - `prompts.rs`: Prompt listing and retrieval
  - `logging.rs`: Logging message handling
  - `telemetry.rs`: Telemetry event handling
  
- **ServerState** (`src/state.rs`): Clean state management struct with:
  - Thread-safe cached collections (tools, resources, prompts)
  - Roots list management
  - Subscription manager reference
  - Initialization flag
  
- **Discovery module** (`src/discovery/`): Extracted discovery logic for tools, resources, and prompts
  - `discover_tools()`, `discover_resources()`, `discover_prompts()` functions
  
- **Auth module** (`src/auth/`): Credential resolution and validation
  - `.auth.json` loading from tool directories
  - Environment variable validation and injection

**Current structure:**
```
src/
├── lib.rs              # Module declarations
├── main.rs             # CLI entry point
├── server.rs           # McpServer, ServerBuilder (~400 lines)
├── state.rs            # ✅ ServerState struct
├── routing.rs          # ✅ Request routing module
├── handlers/           # ✅ All handler modules
│   ├── init.rs
│   ├── tools.rs
│   ├── resources.rs
│   ├── prompts.rs
│   ├── logging.rs
│   └── telemetry.rs
├── discovery/          # ✅ Discovery logic
│   ├── mod.rs
│   ├── tools.rs
│   ├── resources.rs
│   └── prompts.rs
├── auth/               # ✅ Auth module
│   └── mod.rs
├── protocol.rs         # MCP protocol types
└── watcher.rs          # File system watchers
```

## Architectural Improvements

### 13. Daemon Mode: Long-Running stdio Server
**Status**: ✅ Completed

Implemented long-running daemon mode for persistent stdio server:

**What was done:**
- **CLI flag**: Added `--daemon`/`-d` flag to enable persistent mode
- **Stdin loop**: Dedicated `stdin_loop_daemon()` processes requests continuously
- **Graceful shutdown**: SIGTERM/SIGINT handling via tokio signal listeners
- **Client disconnection recovery**: When stdin closes, server continues waiting instead of exiting

**Differences from one-shot mode:**
| Aspect | One-shot | Daemon |
|--------|----------|--------|
| Stdin EOF behavior | Exits gracefully | Keeps listening for new connections |
| Signal handling | None | SIGTERM/SIGINT handled |
| Use case | Composable tools | LSP-style persistent server |

**Usage:**
```bash
# One-shot (current default)
echo '{"jsonrpc":"2.0","id":1,"method":"initialize",...}' | mcp-cli --tools-dir ./tools

# Daemon mode
mcp-cli --daemon --tools-dir ./tools --resources-dir ./resources
```

**Integration tests**: Tests verify multi-request sequences and graceful shutdown.

### 14. Protocol Version Support & Initialization Flow
**Status**: ✅ Completed

Implemented improved protocol version handling and initialization:

**What was done:**
- **Protocol version negotiation**: `negotiate_protocol_version()` function supports:
  - Exact matches for known versions (`2024-11-05`, `2024-10-07`)
  - Forward compatibility with any `2024-*` version
  - Clear error messages listing supported versions

- **Explicit initialized state**: Added `ServerState.initialized: AtomicBool`
  - Thread-safe atomic flag for initialization status
  - Set on successful `initialize` response
  - Checked before handling requests (except `ping`)

- **Cleaned routing logic**: Removed duplicate initialized checks, unified through server state

**Error message example:**
```json
{
  "error": {
    "code": -32602,
    "message": "Unsupported protocol version '2025-01-01'. Supported versions: 2024-11-05, 2024-10-07"
  }
}
```

**Health check before init**: `ping` method now works without initialization for health monitoring.

## Additional Features

### 15. Enhanced Health Check (ping)
**Status**: ✅ Completed

Enhanced the `ping` method from a simple connectivity check to a proper health check endpoint.

**What was done:**
- Added `src/handlers/ping.rs` with `handle_ping()` function
- Returns `{ initialized, server_info: { name, version }, capabilities: { tools, resources, prompts, logging, roots } }`
- Allows clients to verify server is alive and understand its configuration
- `ping` still works before initialization (for health monitoring)

### 17. Logging Messages
**Status**: ✅ Completed

Implemented `logging/messages` method to allow clients to send log messages to the server.

**What was done:**
- Added protocol types: `LogLevel` enum, `LogMessageParams`, and `LogMessageResult`
- Implemented handler in `src/handlers/logging.rs` with tracing integration
- Supports all standard log levels: debug, info, notice, warning, error, critical, alert, emergency
- Logs are sent to the tracing crate with optional logger names
- Fallback behavior for unknown log levels

**MCP spec method implemented:**
- ✅ `logging/messages` - Send log message to server

**Usage example:**
```json
{
  "jsonrpc": "2.0",
  "method": "logging/messages",
  "params": {
    "level": "info",
    "logger": "my-client",
    "message": "Client started successfully"
  }
}
```

**Test coverage:**
- `test_logging_messages_before_initialize` - Verifies method requires initialization
- `test_logging_messages_with_info_level` - Tests info level logging
- `test_logging_messages_with_debug_level` - Tests debug level logging
- `test_logging_messages_with_error_level` - Tests error level logging
- `test_logging_messages_with_unknown_level` - Tests fallback for unknown levels
- `test_logging_messages_with_capabilities` - Tests with server capability enabled

### 16. Telemetry Events
**Status**: ✅ Completed

Implemented `telemetry/event` method for sending telemetry from clients to the server.

**What was done:**
- Added handler in `src/handlers/telemetry.rs`
- Telemetry events logged at debug level via tracing
- Server capability enabled via `enable_telemetry()`

**MCP spec method implemented:**
- ✅ `telemetry/event` - Send telemetry event to server

**Usage example:**
```json
{
  "jsonrpc": "2.0",
  "method": "telemetry/event",
  "params": {
    "eventName": "client.action",
    "data": {"key": "value"}
  }
}
```

### 19. More MIME Types
**Status**: ✅ Completed

Extended `mime_from_extension()` in `src/discovery/resources.rs` to cover:
- PDF (`application/pdf`)
- Images (`image/png`, `image/jpeg`, `image/gif`, `image/webp`, `image/svg+xml`, `image/x-icon`)
- Fonts (`font/woff`, `font/woff2`, `font/ttf`, `font/otf`, `application/vnd.ms-fontobject`)
- Archives (`application/zip`, `application/x-tar`, `application/gzip`, `application/x-bzip2`, `application/x-xz`, `application/x-7z-compressed`, `application/vnd.rar`)

### 21. Tool Authentication
**Status**: ✅ Completed

Implemented full authentication support:
- **OAuth2 token flow**: Client credentials grant with automatic token refresh
- **Credential injection**: Auth tokens injected into tool process environment variables
- **Environment variable validation**: Proper masking and validation of sensitive values
- **Multiple auth strategies**: `none`, `bearer_token`, `api_key`, `oauth2` support

## Testing & Documentation

### 22. Performance Benchmarks
**Status**: ⏳ Pending

Add benchmark tests for:
- Large file resource reading
- Many tools discovery
- Concurrent requests (if persistent mode added)

### 23. Example Tools Repository
**Status**: ✅ Completed

Created `docs/examples/tools/` with example tool scripts demonstrating:

**Tools:**
- `curl-request.sh` — HTTP client with complex argument parsing (method, headers, body) and flexible auth (none, bearer_token, api_key)
- `file-manager.sh` — CRUD file operations (read, write, list, delete, exists) with path safety checks
- `weather.sh` — External API call with `bearer_token` auth strategy
- `db-query.sh` — Database query tool with `api_key` auth strategy
- `deploy.sh` — Deployment tool with `oauth2` client credentials flow
- `image-info.sh` — Multiple content output types (text metadata + base64 image)
- `env-inspector.sh` — Debugging tool for verifying auth config and credential injection

**Auth configs:**
- `weather.auth.json` — bearer_token
- `db-query.auth.json` — api_key
- `deploy.auth.json` — oauth2 with client credentials

Each tool includes inline documentation explaining the pattern and usage examples.

### 24. Client SDK Examples
**Status**: ⏳ Pending

Add client integration examples for:
- TypeScript/JavaScript clients
- Python clients
- Shell script wrappers

## MCP Specification Compliance

### 25. ListChanged Notifications
**Status**: 🔄 In Progress

The MCP spec defines `notifications/tools/list_changed`, `notifications/resources/list_changed`, and `notifications/prompts/list_changed` for clients to stay in sync with server state changes.

**Tasks:**
- Emit `notifications/tools/list_changed` when tools directory changes (types exist in protocol.rs:378-399, never emitted)
- Emit `notifications/resources/list_changed` when resources directory changes (types exist in protocol.rs:519-533, never emitted)
- Emit `notifications/prompts/list_changed` when prompts directory changes
- Emit `notifications/resources/updated` to subscribers when resource files change

### 26. Sampling / createMessage
**Status**: ⏳ Pending

The MCP spec defines `sampling/createMessage` which lets the server ask the client to call an LLM on its behalf. Major capability gap.

**Tasks:**
- Protocol types: `CreateMessageRequest`, `SamplingMessage`, `CreateMessageResult`
- Client capabilities detection
- Handler that sends request to client and awaits response
- Server capability flag
- Integration test

### 27. Completions / complete
**Status**: ✅ Completed

Implemented `completion/complete` method for argument autocompletion.

**What was done:**
- **Protocol types**: `CompletionReference`, `CompleteParams`, `CompleteArgument`, `CompleteResult`
- **Handler** in `src/handlers/completion.rs`: `handle_completion_complete()` with tool name completion
- **Routing**: Added `completion/complete` to `KNOWN_METHODS` and `route_request()`
- **Lazy loading**: Tool discovery triggered on first completion request if not yet loaded
- **Prefix matching**: Case-insensitive substring matching for tool name completion
- **Integration tests**: `test_completion_complete_tool_names` and `test_completion_complete_no_matches`

**Usage example:**
```json
{
  "jsonrpc": "2.0",
  "method": "completion/complete",
  "params": {
    "ref": { "type": "tool", "value": "list" },
    "argument": { "name": "name", "value": "list" }
  }
}
// Returns: {"values": ["list-files"], "total": 1, "has_more": false}
```

### 28. Resource Templates
**Status**: ⏳ Pending

`ResourceTemplate` type exists in protocol.rs but is never used.

**Tasks:**
- Implement `resources/templates/list` method
- Discover resource templates from resources directory
- Advertise `templates` in resources capability
- Integration test

## Backlog / Future Work

- WebSocket transport support
- Request batching
- Tool caching (beyond first load)
- Resource content caching with ETags
- Custom error codes per tool
- Plugin system for extensibility
- Health check endpoint
  - Implemented in `src/handlers/ping.rs`: returns `initialized`, `server_info`, and `capabilities`
- Graceful shutdown handling
