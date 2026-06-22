//! `mustard-mcp` — the `mustard-memory` Model Context Protocol server binary.
//!
//! Thin entry point: all logic lives in the [`mustard_mcp`] library. This is
//! the long-lived face Claude Code spawns (`mcpServers.mustard-memory`); giving
//! it its own exe decouples it from `mustard-rt` rebuilds (a running MCP server
//! no longer holds `mustard-rt.exe` open and blocks its reinstall on Windows).
//!
//! Backward compatibility: `mustard-rt mcp` still works — that subcommand now
//! delegates to `mustard_mcp::run()`, the same entry this binary calls.

fn main() {
    mustard_mcp::run();
}
