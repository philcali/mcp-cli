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
**Pending**: Improve prompt caching with proper invalidation

Currently prompts are reloaded on every request when cache is empty.

**Suggestions:**
- Add TTL-based cache expiration
- Watch file changes and invalidate cache on modification
- Support cache refresh via `prompts/listChanged` notification

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

## Architectural Improvements

### 7. Server Monolith Refactor & Modularization
**Pending**: Break up server.rs into focused modules

The current `server.rs` is a monolithic file (~1000 lines) mixing concerns:
- Initialization logic intertwined with request routing
- Tool/resource/prompt discovery in same file as handlers
- No clear separation between protocol handling and business logic
- Duplicate code patterns across handler methods
- Routing logic scattered throughout

**Proposed structure:**
```
src/
├── server.rs          # Entry point, stdio transport loop
├── routing.rs         # Request routing, method dispatch
├── handlers/
│   ├── init.rs        # Initialize/initialized handling
│   ├── tools.rs       # Tool list/call operations
│   ├── resources.rs   # Resource CRUD and subscriptions
│   └── prompts.rs     # Prompt listing/retrieval
├── discovery.rs       # Tool/resource/prompt file discovery
├── auth/              # Authentication module
│   ├── config.rs      # Auth configuration loading
│   └── resolver.rs    # Credential resolution
└── state.rs           # Shared server state management
```

**Benefits:**
- Easier to test individual components in isolation
- Clearer ownership of features (tools team vs resources team)
- Reduced merge conflicts when multiple people working
- Better code navigation and discoverability
- Can progressively refactor without breaking changes

**Implementation approach:**
1. Extract routing logic into dedicated module with clear method→handler mapping
2. Create handler modules with consistent signatures (`async fn handle_XXX(&self, params) -> Result<Value>`)
3. Move discovery logic to separate module with shared caching interface
4. Introduce `ServerState` struct for clean state management (replacing scattered fields)
5. Add integration tests after each extraction to ensure behavior preserved

**Quick win first:** Extract routing (`route_request`) into its own file and add explicit method→handler documentation.

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
