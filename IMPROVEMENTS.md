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
**Pending**: Add progress notifications and result streaming for long-running operations

MCP supports:
- `progress` notifications for long-running requests
- Result streaming via SSE or similar mechanisms

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

Implemented full subscription support for MCP resources.

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
- ⏳ `resources/listChanged` - Notification when list changes (not yet needed)

**Test coverage:**
Added 5 comprehensive integration tests:
- `test_resources_subscribe_valid_resource` – Successful subscription
- `test_resources_subscribe_nonexistent_resource` – Error on missing resource
- `test_resources_unsubscribe_valid_resource` – Unsubscribe flow
- `test_resources_unsubscribe_nonexistent_resource` – Error handling
- `test_resources_subscribe_and_read` – Combined workflow (subscribe then read)

**Notes:**
The subscription manager is currently a simple in-memory store. For more complex use cases, this could be extended to support file watching or persistent subscriptions.

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

Created unified file system watcher module that provides shared infrastructure for watching tools, prompts, and resources directories:

**What was done:**
- **New `src/watcher.rs` module**: Centralized file watching abstraction using `notify` crate
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
**Status**: 🟡 In Progress (Steps 1-2 Complete)

Successfully refactored server.rs by extracting routing and handlers:

**Completed:**
- ✅ **Routing module** (`src/routing.rs`): Centralized request routing with explicit method→handler mapping
  - `route_request()` function delegates to handler modules
  - Added `KNOWN_METHODS` constant for introspection
  
- ✅ **Handler modules** (in `src/handlers/`):
  - `init.rs`: Initialize connection and capability negotiation
  - `tools.rs`: Tool listing and execution
  - `resources.rs`: Resource CRUD and subscriptions
  - `prompts.rs`: Prompt listing and retrieval
  - All handlers use consistent signature: `async fn handle_XXX(&server, params) -> Result<Value>`

- ✅ **Test coverage**: All 26 integration tests pass after refactoring

**Current structure:**
```
src/
├── lib.rs              # Module declarations
├── main.rs             # CLI entry point
├── server.rs           # McpServer, ServerBuilder (still ~600 lines)
├── routing.rs          # ✅ Request routing module
├── handlers/
│   ├── init.rs         # ✅ Initialize handler
│   ├── tools.rs        # ✅ Tools handlers
│   ├── resources.rs    # ✅ Resources handlers
│   └── prompts.rs      # ✅ Prompts handlers
├── protocol.rs         # MCP protocol types
└── watcher.rs          # File system watchers
```

**Remaining work:**
1. ⏳ Move discovery logic (`load_tools`, `load_resources`, `load_prompts`) to separate module
2. ⏳ Introduce `ServerState` struct for clean state management
3. ⏳ Create dedicated auth module (currently in server.rs)

**Quick win completed**: Extracted routing with explicit method→handler documentation.

## Architectural Improvements

### 7. Daemon Mode: Long-Running stdio Server
**Pending**: Add flag to run as persistent daemon instead of one-shot handler

Currently the server is **one-shot per invocation**—each process handles exactly one request, then exits. This works for composable CLI tools but limits performance and real-time capabilities.

**Proposal:** Add `--daemon` or `-d` flag to enable long-running stdio mode:
```bash
# One-shot (current behavior)
echo '{"jsonrpc":"2.0","id":1,"method":"initialize",...}' | mcp-cli --tools-dir ./tools

# Daemon mode (new)
mcp-cli --daemon --tools-dir ./tools
# Server stays alive, handles multiple requests from same stdin stream
```

**Benefits:**
- **Performance**: No process spawn overhead for each request
- **State persistence**: Caches remain valid across calls within session
- **Real-time capabilities**: Can send notifications to client (e.g., `resources/listChanged`)
- **Better async utilization**: Leverage tokio's async runtime fully

**Implementation approach:**
1. Add CLI flag: `clap::arg!("-d, --daemon", "Run as persistent stdio server")`
2. Refactor stdin loop into reusable function:
    ```rust
    async fn run_one_shot() -> Result<()> { /* current behavior */ }
    async fn run_daemon(server: &mut McpServer) -> Result<()> {
        while let Some(req) = read_line().await? {
            handle_and_send(req, server).await?;
        }
    }
    ```
3. Add graceful shutdown handling (SIGTERM/SIGINT for systemd compatibility)
4. Update systemd unit file example:
    ```ini
    [Unit]
    Description=MCP CLI Server
    After=network.target

    [Service]
    Type=simple
    ExecStart=/usr/local/bin/mcp-cli --daemon --tools-dir /etc/mcp/tools --resources-dir /etc/mcp/resources
    Restart=on-failure
    RestartSec=5

    [Install]
    WantedBy=multi-user.target
    ```
5. Add integration tests for multi-request sequences in daemon mode

**Tradeoffs:**
- **Pros**: Better performance, enables notifications, cleaner async model with tokio
- **Cons**: Process lifecycle management required (systemd or similar), potential resource leaks if not cleaned up properly

**Status**: Ready to implement—tokio runtime already present, minimal code churn needed.

### 8. Protocol Version Support & Initialization Flow
**Pending**: Better protocol version handling and initialization

Currently only accepts versions starting with "2024-" (hardcoded check). The init flow has issues:
- Duplicate protocol validation code (now fixed)
- Fragile `initialized` flag based on response string matching
- No proper negotiation of protocol capabilities
- Client roots handling mixed with version checking

**Proposed improvements:**
- Version negotiation with clear error messages for unsupported versions
- Separate initialization state from capabilities (use explicit `initialized: bool` field)
- Capability-based feature detection instead of hardcoded checks
- Proper lifecycle management: init → ready → handle requests

## Additional Features

### 9. Logging Messages
**Pending**: Implement `logging/messages` method

Allow clients to send log messages to server for unified logging.

### 10. Telemetry Events
**Pending**: Add `telemetry/event` support

Send server metrics and usage data to clients.

### 11. More MIME Types
**Pending**: Extend supported MIME types in resources

Add more file type detections:
- `.pdf` → application/pdf
- `.png`, `.jpg`, `.gif` → image/* (blob support)
- `.woff`, `.ttf` → font/* 
- Various archive formats

### 12. Tool Authentication
**Pending**: Improve authentication handling

Currently has basic auth config loading, but:
- Support OAuth flows
- Better credential injection into tool environment
- Environment variable validation and masking

## Testing & Documentation

### 13. Performance Benchmarks
**Pending**: Add benchmark tests for:
- Large file resource reading
- Many tools discovery
- Concurrent requests (if persistent mode added)

### 14. Example Tools Repository
**Pending**: Create example tool scripts demonstrating:
- Complex argument parsing
- Multiple output content types (text, image blobs)
- Error handling patterns
- Auth integration examples

### 15. Client SDK Examples  
**Pending**: Add client integration examples for:
- TypeScript/JavaScript clients
- Python clients
- Shell script wrappers

## Backlog / Future Work

- WebSocket transport support
- Request batching
- Tool caching (beyond first load)
- Resource content caching with ETags
- Custom error codes per tool
- Plugin system for extensibility
- Health check endpoint
- Graceful shutdown handling
