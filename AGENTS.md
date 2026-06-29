# AGENTS

This repository is for `skilltrace`, a local-first Rust CLI and MCP-compatible telemetry tool for observing how coding agents discover and use agent skills.

## Mission

`skilltrace` exists to help a personal user answer:

- Which agent-discovered skills are used most or least?
- Which discovered skills are never used?
- Which skills have the highest success, failure, or retry rates?
- Which decisions and skill chains correlate with success or failure?
- Which parts of an agent workflow should be promoted, fixed, merged, or removed?

The product direction is:

- Rust-based
- CLI-first
- local-first
- append-only by default
- agent-readable
- MCP-compatible
- hook/plugin-friendly
- safe for concurrent events from parallel subagents
- based on agent-reported skill inventory, not a duplicated `skills.toml` catalog
- Codex-first for the next integration pass, with Claude Code and OpenCode as high-value follow-ups

## Current Stage

The project is in the **post-Phase-1 reframing stage**.

The original Codex MVP foundation is implemented, but the product direction has been narrowed and corrected: `skilltrace` should observe skill inventory and skill usage reported by agent hooks/plugins/MCP tools. It should not require users to define available skills again in `skills.toml`.

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
- SQLite busy timeout for concurrent write pressure
- hashing/redaction for sensitive summary-like fields before persistence/output, including common key/value, bearer, query-string, compact JSON, and spaced JSON/log forms
- test coverage for wrapper privacy/exit/boundary/correlation behavior, skill-definition resolution, MCP request handling, session correlation, recommendation chains, and privacy hashing

Not implemented yet:

- `skilltrace serve` local intake service for hook/plugin calls
- first-class skill inventory storage for agent-discovered skills
- MCP/intake tools for `record_session_start`, `record_session_end`, and `record_skills_discovered`
- unused/recommend reports backed by discovered skill inventory instead of `skills.toml`
- idempotent caller-supplied event ids for concurrent hook/subagent writes
- explicit subagent/agent correlation fields beyond generic metadata
- rich internal tool-call capture beyond explicit wrapper command boundaries and MCP/manual events
- deeper recommendation heuristics such as longer multi-skill chain analysis and time-windowed correlations
- stronger parsing/redaction for future raw capture paths if raw capture is ever enabled
- non-stdio MCP transports
- packaged hook/plugin adapters beyond the current Codex-first baseline
- OpenTelemetry export/serve functionality

## Roadmap

### Phase 1: Historical Codex MVP

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

### Phase 2: Reframed Local MVP

Planned:

- implement skill inventory storage
- add `record_session_start`, `record_session_end`, and `record_skills_discovered`
- update `unused` and `recommend` to use discovered skill inventory
- add or prepare `skilltrace serve` as the local hook/plugin intake surface
- make concurrent hook/MCP/wrapper/subagent writes explicit and tested
- keep `skilltrace wrap` as fallback session correlation, not the primary skill tracing mechanism
- document Codex hook-based integration

Implementation plan:

- `docs/superpowers/plans/2026-06-29-reframed-local-mvp.md`

### Phase 3: Codex Hook Integration

Planned:

- installable Codex hook config
- session lifecycle reporting through hooks
- discovered skill inventory reporting where Codex exposes enough metadata
- explicit MCP calls or hooks for accurate skill lifecycle events
- observability-gap reporting when Codex exposes inventory but not usage

### Phase 4: Claude Code

Planned:

- adapter for Claude Code hooks and MCP workflows
- command, HTTP, or MCP-tool hook transport into `skilltrace`
- session lifecycle, skill inventory, and skill usage normalization
- documentation of supported hook events and limitations

### Phase 5: OpenCode

Planned:

- OpenCode plugin
- native skill tool observation
- OpenCode skill inventory reporting
- session and tool event normalization

### Phase 6: Broader Agent Support

Planned:

- add adapters only when an agent exposes lifecycle hooks, plugin event streams, MCP tool calls, stable local trace files, or structured trace export

### Phase 7: OpenTelemetry Export

Planned:

- `skilltrace export otel`
- `skilltrace serve-otel`

## Working Rules

- Keep the CLI output table-first and agent-readable.
- Maintain `README.md` as a GitHub-facing product page: open with the core value in one strong sentence, explain from abstract idea to concrete behavior, then show usage commands. Keep it concise, active, and current with the implemented state.
- Prefer explicit event recording over fragile terminal parsing.
- Treat agent-reported skill inventory as authoritative for available skills.
- Do not add new features that require users to maintain a duplicate skill catalog.
- Treat `skills.toml` and `[skills].definition_file` as legacy implementation details until removed or explicitly kept as an import fallback.
- Preserve privacy defaults:
  - summaries by default
  - no raw prompts by default
  - no raw outputs by default
- Preserve append-only semantics unless there is a strong reason not to.
- Treat Codex as the primary adapter for the next integration pass, but keep storage/reporting independent of Codex-specific assumptions.
- When implementing storage/reporting changes, keep SQLite and JSONL behavior aligned.
- When implementing intake changes, assume multiple hooks, MCP calls, wrapper events, and subagents can write concurrently.
- Prefer explicit `session_id`, `agent_id`, `subagent_id`, `task_id`, and `source_event_id` over runtime "current session" fallback whenever the adapter exposes them.
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
│           ├── 2026-06-28-phase-1-completion.md
│           └── 2026-06-29-reframed-local-mvp.md
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
  - legacy `[skills].definition_file` support until inventory-backed reports replace it

- `src/db.rs`
  - SQLite connection setup
  - schema bootstrap
  - event/session queries
  - adjacent skill chain query support
  - streaming JSONL export source
  - next phase: skill inventory schema and concurrent/idempotent intake support

- `src/event.rs`
  - `EventRecord` model
  - next phase: optional caller-supplied ids and correlation metadata for hook/subagent intake

- `src/mcp/server.rs`
  - stdio JSON-RPC loop
  - MCP request parsing and error framing
  - tool-call handlers backed by local storage
  - MCP session fallback from `SKILLTRACE_SESSION_ID`
  - next phase: first-class session lifecycle and `record_skills_discovered` tools

- `src/mcp/tools.rs`
  - MCP tool metadata / schema surface

- `src/privacy.rs`
  - summary-field hashing/redaction helpers driven by privacy config
  - key/value, bearer, query-string, compact JSON, and spaced JSON/log sanitization

- `src/stats.rs`
  - rendering and filter parsing
  - current `unused` logic still depends on declared skills; next phase should use discovered inventory

- `src/recommend.rs`
  - current heuristic recommendation logic including overlap, inactivity, and adjacent chain signals
  - current unused/stale inputs still depend on declared skills; next phase should use discovered inventory

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
cargo run -- event skill_start --skill writing-plans
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

When touching future inventory/concurrency work, add and run focused tests for:

- discovered skill inventory insert/query behavior
- unused/recommend reports without `skills.toml`
- concurrent event writes from explicit session ids
- JSONL append line integrity under concurrent writes

## Known Gaps

These are active backlog items, not accidental omissions:

- `skills.toml` is still present in the current implementation, but the revised product direction treats it as legacy/fallback rather than authoritative.
- `unused` and part of `recommend` still depend on declared skill definitions; they should be migrated to discovered skill inventory.
- `skilltrace serve` does not exist yet.
- MCP does not yet expose first-class `record_session_start`, `record_session_end`, or `record_skills_discovered`.
- concurrent multi-process session semantics are improved through environment correlation and SQLite busy timeout, but parallel subagent intake still needs explicit ids, idempotent writes, and JSONL append integrity tests.
- recommendation logic is improved and chain-aware for adjacent pairs, but still heuristic rather than a full workflow-correlation engine.
- wrapper tracing captures session lifecycle and command boundaries, but not arbitrary internal tool-call boundaries inside wrapped processes.
- privacy hashing covers summary-like fields and common sensitive text forms, but future raw capture paths would need separate stronger controls before being enabled
- `otel` export is still placeholder-only

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
