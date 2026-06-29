# skilltrace

**See which agent skills your coding agent discovers, uses, ignores, retries, and fails.**

`skilltrace` is a local-first telemetry tool for AI coding agents. It records skill inventory and skill usage from agent hooks, plugins, Model Context Protocol (MCP) tools, and command line events so you can improve your real skill set without maintaining a duplicate catalog.

## Why it exists

Agent skills are becoming reusable infrastructure: `SKILL.md` instructions, workflow playbooks, scripts, and references that agents load when they need specialized behavior.

As that skill set grows, you need answers that agent tools do not show by default:

- which discovered skills the agent uses
- which discovered skills the agent ignores
- which skills fail or retry
- which skills appear together
- which skills look stale, redundant, or worth improving

`skilltrace` stores that signal locally and reports it back in an agent-readable format.

## Core idea

`skilltrace` observes; agent tools decide.

The agent remains responsible for skill discovery, permissions, selection, and loading. `skilltrace` receives normalized telemetry from that agent and stores it locally.

Target flow:

1. An agent starts a session
2. A hook, plugin, MCP tool, or command reports `session_start`
3. The agent or adapter reports discovered skills
4. The agent reports `skill_start`, `skill_end`, and failure events
5. `skilltrace` reports usage, failures, retries, stale skills, and unused discovered skills

## Current status

This repository has the post-Phase-1 foundation:

- local initialization with `skilltrace init`
- SQLite-backed session and event storage
- JSONL mirroring and export
- manual event recording with `skilltrace event`
- reports: `stats`, `timeline`, `failures`, `unused`, `recommend`
- wrapper-based session correlation with `skilltrace wrap <command>`
- stdio MCP server with event and skill lifecycle tools
- privacy sanitization for summary-like fields

The current implementation still uses `skills.toml` as a legacy declared-skill source for some reports. The revised design replaces that with agent-reported discovered skill inventory.

## Install and run locally

Clone the repository and run commands through Cargo:

```bash
cargo run -- init
```

Record a skill lifecycle manually:

```bash
cargo run -- event skill_start --skill writing-plans
cargo run -- event skill_end --skill writing-plans --success true
```

Inspect local traces:

```bash
cargo run -- stats
cargo run -- timeline --last
cargo run -- failures
cargo run -- recommend
```

Run the MCP server:

```bash
cargo run -- mcp
```

Use wrapper fallback correlation:

```bash
cargo run -- wrap /bin/echo hello
```

## What comes next

The reframed MVP focuses on hook-friendly skill telemetry:

- first-class skill inventory storage
- `record_session_start`, `record_session_end`, and `record_skills_discovered`
- inventory-backed `unused` and `recommend`
- `skilltrace serve` for local hook and plugin intake
- concurrent intake safety for parallel subagents
- Codex hook integration docs and config

See the [reframed local MVP plan](docs/superpowers/plans/2026-06-29-reframed-local-mvp.md) for implementation details.

## Development

Run the full local checks:

```bash
cargo fmt
cargo check
cargo test
```

Run focused suites while working on a specific area:

```bash
cargo test mcp
cargo test wrap
cargo test privacy
cargo test skills_resolution
```

## Repository layout

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
  privacy.rs
  recommend.rs
  stats.rs
```

Read [project_proposal.md](project_proposal.md) for the product proposal and [AGENTS.md](AGENTS.md) for repo-specific working rules.
