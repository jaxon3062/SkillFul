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

The project is in the **post-Phase-1 Codex MVP stabilization stage**.

Implemented today:

- local initialization via `skilltrace init`
- SQLite-backed sessions/events storage
- JSONL mirroring
- manual event recording via `skilltrace event`
- reporting via `stats`, `timeline`, `failures`, `unused`, and `recommend`
- process wrapper session tracing via `skilltrace wrap <command>`
- explicit wrapper command boundary events via `command_start` and `command_end`
- wrapped child session correlation through `SKILLTRACE_SESSION_ID`, `SKILLTRACE_AGENT`, and `SKILLTRACE_ADAPTER`
- stdio MCP server with working tool handlers
- MCP `record_skill_start` correlation through `SKILLTRACE_SESSION_ID`
- review-driven correctness/privacy fixes
- recommendation heuristics for overlap/co-occurrence, adjacent skill chains, and 30-day inactivity
- hashing/redaction for sensitive summary-like fields before persistence/output, including common key/value, bearer, query-string, compact JSON, and spaced JSON/log forms
- test coverage for wrapper privacy/exit/boundary/correlation behavior, skill-definition resolution, MCP request handling, session correlation, recommendation chains, and privacy hashing

Not implemented yet:

- first-class session leases/locks for concurrent long-lived workflows
- rich internal tool-call capture beyond explicit wrapper command boundaries and MCP/manual events
- deeper recommendation heuristics such as longer multi-skill chain analysis and time-windowed correlations
- stronger parsing/redaction for future raw capture paths if raw capture is ever enabled
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
- wrapper command boundary events
- wrapped child session correlation
- MCP stdio server
- MCP event/session correlation for `record_skill_start`
- data-backed stats/failures/timeline/recommend
- review-driven correctness/privacy fixes
- overlap/inactivity recommendation heuristics
- adjacent chain recommendation heuristics
- summary-field sensitive value hashing/redaction for key/value, bearer, query-string, compact JSON, and spaced JSON/log forms
- Phase 1 completion plan and implementation record in `docs/superpowers/plans/2026-06-28-phase-1-completion.md`

Remaining in Phase 1: none.

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
├── docs
│   └── superpowers
│       └── plans
│           └── 2026-06-28-phase-1-completion.md
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
│   ├── privacy.rs
│   ├── recommend.rs
│   └── stats.rs
├── tests
│   ├── mcp.rs
│   ├── privacy.rs
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
  - wrapper command boundary events
  - wrapped child environment correlation
  - manual event recording
  - reporting command integration

- `src/config.rs`
  - `~/.skilltrace` path management
  - config loading/default creation
  - runtime session state
  - `SKILLTRACE_*` environment variable constants and session fallback helpers

- `src/db.rs`
  - SQLite connection setup
  - schema bootstrap
  - event/session queries
  - adjacent skill chain query support
  - streaming JSONL export source

- `src/event.rs`
  - `EventRecord` model

- `src/mcp/server.rs`
  - stdio JSON-RPC loop
  - MCP request parsing and error framing
  - tool-call handlers backed by local storage
  - MCP session fallback from `SKILLTRACE_SESSION_ID`

- `src/mcp/tools.rs`
  - MCP tool metadata / schema surface

- `src/privacy.rs`
  - summary-field hashing/redaction helpers driven by privacy config
  - key/value, bearer, query-string, compact JSON, and spaced JSON/log sanitization

- `src/stats.rs`
  - rendering and filter parsing

- `src/recommend.rs`
  - current heuristic recommendation logic including overlap, inactivity, and adjacent chain signals

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
- `cargo test privacy`
- `cargo test skills_resolution`

When touching session correlation, also run:

- `SKILLTRACE_SESSION_ID=ambient-session cargo test mcp::server::tests::skill_start_without_session_id_uses_single_active_runtime_session`

## Known Gaps

These are active backlog items, not accidental omissions:

- recommendation logic is improved and chain-aware for adjacent pairs, but still heuristic rather than a full workflow-correlation engine
- wrapper tracing captures session lifecycle and command boundaries, but not arbitrary internal tool-call boundaries inside wrapped processes
- privacy hashing covers summary-like fields and common sensitive text forms, but future raw capture paths would need separate stronger controls before being enabled
- `otel` export is still placeholder-only
- concurrent multi-process session semantics are improved through environment correlation, but not fully modeled as first-class session leases/locks

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
- `7d43f69` Improve recommendation heuristics
- `1e4eecd` Hash sensitive event summaries
- `03b616d` Handle MCP notifications without responses
- `3a95a36` Ignore repo-local worktrees
- `1608e97` Add Phase 1 completion plan
- `8acebdf` Merge chain-aware recommendations
- `67ce088` Merge wrapper command boundaries
- `197bc56` Merge stronger privacy sanitization
- `f4c5ea4` Merge wrapped session correlation

Use these as checkpoints when reasoning about why the current code looks the way it does.
