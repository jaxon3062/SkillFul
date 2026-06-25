# AGENTS

This repository is for `skilltrace`, a local-first Rust CLI and MCP-native tracing tool for observing how coding agents use skills and tools.

## Mission

`skilltrace` exists to help a personal user answer:

- Which skills are used most or least?
- Which skills succeed, fail, or retry most often?
- Which decisions and skill chains correlate with success or failure?
- Which parts of an agent workflow should be promoted, fixed, merged, or removed?

The product direction is:

- Rust-based
- CLI-first
- local-first
- append-only by default
- agent-readable
- MCP-compatible
- Codex-first for the initial MVP

## Current Stage

The project is in the **Codex MVP implementation stage**.

Implemented today:

- local initialization via `skilltrace init`
- SQLite-backed sessions/events storage
- JSONL mirroring
- manual event recording via `skilltrace event`
- reporting via `stats`, `timeline`, `failures`, `unused`, and `recommend`
- process wrapper session tracing via `skilltrace wrap <command>`
- stdio MCP server with working tool handlers
- privacy/default fixes from repository review
- test coverage for wrapper privacy/exit behavior, skill-definition resolution, and MCP request handling

Not implemented yet:

- rich wrapper boundary detection for commands/tool calls inside wrapped processes
- stronger secret hashing/redaction beyond the current summary-only defaults
- advanced recommendation heuristics such as co-occurrence/overlap analysis and time-windowed inactivity
- non-stdio MCP transports
- adapters beyond the current Codex-first baseline
- OpenTelemetry export/serve functionality

## Roadmap

### Phase 1: Codex MVP

Completed:

- CLI scaffold
- SQLite schema bootstrap
- JSONL mirror
- process wrapper session lifecycle
- MCP stdio server
- data-backed stats/failures/timeline/recommend
- review-driven correctness/privacy fixes

Remaining in Phase 1:

- richer wrapper event capture inside wrapped sessions
- more capable recommendations
- better session correlation across concurrent/long-lived agent workflows
- stronger privacy controls for any future raw capture

### Phase 2: Claude Code

Planned:

- adapter for Claude Code MCP workflows
- event normalization around tool calls and session metadata

### Phase 3: OpenCode

Planned:

- wrapper or config-based integration
- multi-session tracing support

### Phase 4: OpenClaw

Planned:

- OTLP ingestion
- normalization into `skilltrace` events

### Phase 5: Hermes

Planned:

- wrapper-first integration
- MCP mode later if Hermes supports it cleanly

### Phase 6: OpenTelemetry Export

Planned:

- `skilltrace export otel`
- `skilltrace serve-otel`

## Working Rules

- Keep the CLI output table-first and easy for agents to parse.
- Prefer explicit event recording over fragile terminal parsing.
- Preserve privacy defaults:
  - summaries by default
  - no raw prompts by default
  - no raw outputs by default
- Preserve append-only semantics unless there is a very strong reason not to.
- Treat Codex as the primary adapter until the MVP is clearly stable.
- When implementing storage/reporting changes, keep SQLite and JSONL behavior aligned.
- When implementing MCP changes, return protocol-shaped error responses instead of crashing the server.
- Commit at each finished feature or milestone stage. Do not batch unrelated work into a single commit.

## Current Repo Structure

```text
.
├── AGENTS.md
├── Cargo.toml
├── project_proposal.md
├── skills.toml
├── src
│   ├── adapters
│   │   ├── claude_code.rs
│   │   ├── codex.rs
│   │   ├── hermes.rs
│   │   ├── mod.rs
│   │   ├── openclaw.rs
│   │   └── opencode.rs
│   ├── cli.rs
│   ├── config.rs
│   ├── db.rs
│   ├── event.rs
│   ├── export
│   │   ├── jsonl.rs
│   │   ├── mod.rs
│   │   └── otel.rs
│   ├── main.rs
│   ├── mcp
│   │   ├── mod.rs
│   │   ├── server.rs
│   │   └── tools.rs
│   ├── recommend.rs
│   └── stats.rs
├── tests
│   ├── mcp.rs
│   ├── skills_resolution.rs
│   └── wrap.rs
└── tmp
```

## Module Notes

- `src/main.rs`
  - process exit behavior
  - wrapper exit-code passthrough

- `src/cli.rs`
  - subcommand routing
  - wrapper execution
  - manual event recording
  - reporting command integration

- `src/config.rs`
  - `~/.skilltrace` path management
  - config loading/default creation
  - runtime session state

- `src/db.rs`
  - SQLite connection setup
  - schema bootstrap
  - event/session queries
  - streaming JSONL export source

- `src/event.rs`
  - `EventRecord` model

- `src/mcp/server.rs`
  - stdio JSON-RPC loop
  - MCP request parsing and error framing
  - tool-call handlers backed by local storage

- `src/mcp/tools.rs`
  - MCP tool metadata / schema surface

- `src/stats.rs`
  - rendering and filter parsing

- `src/recommend.rs`
  - current heuristic recommendation logic

- `src/export/jsonl.rs`
  - streaming JSONL export path

- `src/export/otel.rs`
  - still a stub / future work

## Useful Commands

Core:

```bash
cargo fmt
cargo check
cargo test
```

CLI:

```bash
cargo run -- init
cargo run -- event skill_start --skill run_tests
cargo run -- stats
cargo run -- timeline --last
cargo run -- failures
cargo run -- recommend
cargo run -- export jsonl
```

Wrapper:

```bash
cargo run -- wrap /bin/echo hello
cargo run -- wrap codex
```

MCP:

```bash
cargo run -- mcp
```

## Testing Expectations

Before finishing a meaningful change, prefer to run:

- `cargo fmt`
- `cargo test`

When touching a focused area, run the narrowest useful suite first:

- `cargo test wrap`
- `cargo test mcp`
- `cargo test skills_resolution`

## Known Gaps

These are active backlog items, not accidental omissions:

- recommendation logic is still heuristic and shallow
- wrapper tracing only captures session-level behavior, not internal tool boundaries
- privacy config fields like `hash_sensitive_values` are not fully implemented end-to-end
- `otel` export is still placeholder-only
- concurrent multi-process session semantics are improved but not fully modeled as first-class session leases/locks

## Useful Skills And Tools

When working in Codex with available skills/tooling, these are the most relevant:

- `vercel-react-best-practices`
  - only relevant if a future UI/dashboard or web surface is added

- `python-testing-patterns`
  - useful only if auxiliary test tooling/scripts are later added in Python

- `github:gh-fix-ci`
  - use when a future PR has failing CI

- `github:gh-address-comments`
  - use when addressing GitHub review threads

- `github:yeet`
  - use for intentional commit/push/PR publication workflows

- `firecrawl`
  - use for any web research about MCP specs, agent integrations, or current ecosystem behavior

In normal local repo work, the most useful built-in actions remain:

- `rg` for code search
- `cargo test` for verification
- focused edits with small milestone commits

## Current Commit Landmarks

Recent milestone commits:

- `4b29192` Initial project scaffold
- `42f41b9` Implement local event persistence and reporting
- `4423b5c` Add data-backed recommendations and JSONL export
- `178aab5` Implement process wrapper session tracing
- `24bc879` Add staged review-fix plan
- `91d5092` Fix wrap privacy defaults and exit codes
- `09d7fb5` Treat missing skill success as unknown
- `c98f512` Resolve skill definition paths via config
- `ea4b7f5` Implement stdio MCP tool handlers
- `a94901a` Keep MCP session and JSONL state consistent
- `77e45e9` Resolve core issues from repository review

Use these as checkpoints when reasoning about why the current code looks the way it does.
