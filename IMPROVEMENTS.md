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
**Pending**: Implement `resources/subscribe` method

MCP spec includes:
- `resources/subscribe` - Subscribe to resource change notifications
- `resources/unsubscribe` - Unsubscribe from changes
- `resources/listChanged` - Notification when list changes

### 6. Tool Execution Improvements
**Pending**: Enhance tool execution capabilities

**Suggestions:**
- Add timeout support for long-running tools
- Better environment variable injection for credentials
- Support for concurrent tool execution
- Better error output handling (separate stdout/stderr in result)

## Architectural Improvements

### 7. Persistent Server Mode
**Pending**: Option to keep server process alive

Currently the server is short-lived (one request per invocation). Consider adding:

**Options:**
- Environment variable flag for persistent mode
- HTTP/Unix socket transport option
- Request queuing for better throughput

### 8. Protocol Version Support
**Pending**: Better protocol version handling

Currently only accepts versions starting with "2024-". Could improve:
- Support multiple MCP protocol versions
- Graceful downgrade on version mismatch
- Feature detection per protocol version

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
