# AGENTS

This repository is for `skilltrace`, a local-first Rust CLI and MCP server for tracing coding-agent skill usage.

## Working Rules

- Keep the CLI agent-readable and table-first.
- Prefer explicit event recording over fragile terminal parsing.
- Preserve privacy defaults: summaries only, no raw prompt or output capture by default.
- Treat Codex as the primary adapter until the MVP is stable.

## Useful Commands

```bash
cargo fmt
cargo check
cargo test
cargo run -- init
```

## Key Files

- `project_proposal.md`: product requirements and architecture target
- `skills.toml`: starter declared skill catalog
- `src/db.rs`: SQLite schema bootstrap
- `src/mcp/`: MCP server stubs and tool surface

