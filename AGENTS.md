# mcp-cli Agent Instructions

## Project Overview
A short-lived MCP server using stdio transport. Each invocation handles exactly one request-response cycle and exits.

## Developer Commands

**Build:**
```bash
cargo build --release
```

**Run tests:**
```bash
cargo test
```

**Lint & check (CI order):**
```bash
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
```

## Architecture Notes

- **Entry point**: `src/main.rs` - CLI argument parsing for tools-dir, resources-dir, prompts-dir
- **Core logic**: `src/server.rs` - MCP protocol handlers and capability management
- **Protocol types**: `src/protocol.rs` - JSON-RPC 2.0 + MCP message structures
- **Test helper**: `tests/integration_test.rs` - Spawn server via `env!("CARGO_BIN_EXE_mcp-cli")`, send requests via stdin

## Server Behavior Quirks

1. **Stateful across multiple requests in same process** - initialize must be called before most methods, but resources/prompts/tools discovery persists
2. **One-shot per invocation** - server exits after handling all queued requests from a single stdin stream
3. **CLI args**: `--tools-dir`, `--resources-dir`, `--prompts-dir` configure capabilities

## Testing Patterns

- Tests spawn the compiled binary via `Command::new(env!("CARGO_BIN_EXE_mcp-cli"))`
- Use `run_request_sequence()` for multi-step flows (initialize → other methods) on same process
- Resources/prompts are discovered from directories passed via CLI args in tests
- Tools require executable scripts; test with `--tools-dir`

## Existing Instruction Sources

- `.claude/settings.local.json`: Pre-approved Bash commands (`cargo build:*`, `cargo run:*`, `cargo test:*`, `cargo check:*`, `cargo clippy:*`)
