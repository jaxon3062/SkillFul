# skilltrace

`skilltrace` is a Rust CLI and MCP-native tracing tool for observing how coding agents use skills and tools.

## Current Scope

This repository is initialized around the Codex-first MVP described in [project_proposal.md](/Users/jaxon3062/Documents/Projects/SkillFul/project_proposal.md).

Implemented in the initial scaffold:

- Rust CLI with proposed subcommands
- SQLite schema bootstrap for sessions and events
- Local config generation via `skilltrace init`
- Module layout for adapters, MCP, export, stats, and recommendation logic
- Agent-facing repo instructions and starter `skills.toml`

## Quick Start

```bash
cargo run -- init
cargo run -- stats
```

## Repository Layout

```text
src/
  adapters/
  export/
  mcp/
  cli.rs
  config.rs
  db.rs
  event.rs
  main.rs
  recommend.rs
  stats.rs
```

