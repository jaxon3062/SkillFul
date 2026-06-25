# Project Proposal: `skilltrace`

## Summary

Build `skilltrace`, a Rust-based CLI and MCP-native tracing tool for monitoring AI agent skill usage.

The tool records how agents select, invoke, succeed, fail, retry, and combine skills/tools during coding-agent workflows. It is designed for personal users who want lightweight, local-first observability without dashboards or SaaS dependencies.

Initial integration target: **Codex CLI first**.

Future integration targets: **Claude Code, OpenCode, OpenClaw, Hermes**.

## Core Goal

Help users evaluate and adjust their AI agent skill/toolset by answering:

- Which skills are used most?
- Which skills are never used?
- Which skills succeed or fail most often?
- Which skills are slow or expensive?
- Which skills are selected but later abandoned?
- Which skills overlap with each other?
- Which skill combinations correlate with successful tasks?
- Which planner decisions lead to poor outcomes?

## Product Shape

`skilltrace` should be:

- Rust-based
- CLI-first
- local-first
- agent-readable
- MCP-compatible
- append-only by default
- usable without a web server
- friendly to Codex and other terminal coding agents

## Initial MVP

### Primary command

```bash
skilltrace
```

### Main subcommands

```bash
skilltrace init
skilltrace wrap <command>
skilltrace event
skilltrace stats
skilltrace timeline
skilltrace failures
skilltrace unused
skilltrace export
skilltrace mcp
```

## MVP Use Case: Codex First

Codex should be supported first through a lightweight wrapper and MCP server.

### Mode 1: Process wrapper

```bash
skilltrace wrap codex
```

This runs Codex while collecting observable events around:

- session start
- session end
- command execution
- tool invocation
- MCP tool calls
- skill selection
- skill result
- errors
- retries
- user interruption

### Mode 2: MCP server

```bash
skilltrace mcp
```

Expose skill tracing tools to Codex through MCP.

Initial MCP tools:

```text
skilltrace.record_event
skilltrace.record_skill_start
skilltrace.record_skill_end
skilltrace.record_decision
skilltrace.get_stats
skilltrace.get_failures
skilltrace.get_recommendations
```

This lets Codex call `skilltrace` directly during agent execution.

## Definition of Skill Usage

A “skill” is any named capability, tool, MCP function, agent subroutine, workflow, or prompt module that the agent can choose.

Examples:

```text
repo_search
file_edit
run_tests
debug_error
web_lookup
mcp_github_issue
mcp_docs_lookup
planner
code_review
refactor
shell_command
```

Each skill usage event should capture:

```json
{
  "event_type": "skill_end",
  "session_id": "uuid",
  "task_id": "uuid",
  "skill": "run_tests",
  "agent": "codex",
  "adapter": "codex-wrapper",
  "started_at": "2026-06-25T12:00:00Z",
  "ended_at": "2026-06-25T12:00:05Z",
  "duration_ms": 5000,
  "success": true,
  "error": null,
  "retry_count": 0,
  "input_summary": "Run project test suite",
  "output_summary": "12 tests passed",
  "tokens_input": null,
  "tokens_output": null,
  "cost_usd": null,
  "planner_reason": "User requested validation after code edit",
  "confidence": 0.82,
  "alternatives": ["static_analysis", "manual_review"],
  "metadata": {}
}
```

## Storage

Use a local SQLite database by default.

Default path:

```bash
~/.skilltrace/skilltrace.db
```

Also support JSONL export:

```bash
~/.skilltrace/events.jsonl
```

SQLite tables:

### `sessions`

```sql
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  agent TEXT NOT NULL,
  adapter TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  cwd TEXT,
  repo TEXT,
  branch TEXT,
  metadata_json TEXT
);
```

### `events`

```sql
CREATE TABLE events (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  task_id TEXT,
  event_type TEXT NOT NULL,
  skill TEXT,
  agent TEXT,
  adapter TEXT,
  timestamp TEXT NOT NULL,
  duration_ms INTEGER,
  success BOOLEAN,
  error TEXT,
  retry_count INTEGER DEFAULT 0,
  input_summary TEXT,
  output_summary TEXT,
  planner_reason TEXT,
  confidence REAL,
  alternatives_json TEXT,
  tokens_input INTEGER,
  tokens_output INTEGER,
  cost_usd REAL,
  metadata_json TEXT
);
```

### `skill_stats_cache`

Optional later optimization.

## CLI Commands

### Initialize

```bash
skilltrace init
```

Creates:

```text
~/.skilltrace/
~/.skilltrace/skilltrace.db
~/.skilltrace/config.toml
```

### Wrap agent

```bash
skilltrace wrap codex
skilltrace wrap codex -- codex --model gpt-5.1-codex
```

### Record event manually

```bash
skilltrace event skill_start --skill run_tests
skilltrace event skill_end --skill run_tests --success true --duration-ms 5000
```

### Stats

```bash
skilltrace stats
skilltrace stats --since 7d
skilltrace stats --agent codex
skilltrace stats --skill run_tests
```

Output should be table-first and agent-readable.

Example:

```text
skill          uses   success_rate   avg_ms   retries   failures
run_tests      42     0.88           4210     3         5
repo_search    37     0.73           980      1         10
file_edit      29     0.91           1340     0         2
```

### Timeline

```bash
skilltrace timeline --last
skilltrace timeline --session <id>
```

Example:

```text
12:00:01 session_start codex
12:00:05 skill_start repo_search
12:00:07 skill_end repo_search success
12:00:09 skill_start file_edit
12:00:15 skill_end file_edit success
12:00:16 skill_start run_tests
12:00:21 skill_end run_tests failure
```

### Failure analysis

```bash
skilltrace failures
skilltrace failures --skill run_tests
```

### Unused skills

```bash
skilltrace unused --defined-skills skills.toml
```

Compares declared skills against observed usage.

### Recommendations

```bash
skilltrace recommend
```

Example output:

```text
Recommendations:

1. Consider merging `repo_search` and `file_search`.
   Reason: high co-occurrence and similar success patterns.

2. Improve `run_tests`.
   Reason: high usage, high failure rate, frequent retries.

3. Remove or demote `docs_lookup`.
   Reason: 0 uses in the last 30 days.

4. Promote `static_analysis`.
   Reason: low usage but high success rate when selected.
```

## Configuration

`~/.skilltrace/config.toml`

```toml
[storage]
backend = "sqlite"
path = "~/.skilltrace/skilltrace.db"
jsonl_mirror = true

[agents.codex]
enabled = true
adapter = "wrapper"

[mcp]
enabled = true
transport = "stdio"

[privacy]
capture_raw_prompts = false
capture_raw_outputs = false
hash_sensitive_values = true

[skills]
definition_file = "skills.toml"
```

## Skill Definition File

`skills.toml`

```toml
[[skill]]
name = "repo_search"
description = "Search local repository files"
category = "retrieval"

[[skill]]
name = "file_edit"
description = "Modify source files"
category = "coding"

[[skill]]
name = "run_tests"
description = "Run project tests"
category = "validation"

[[skill]]
name = "debug_error"
description = "Investigate and fix runtime or test failures"
category = "debugging"
```

## Rust Architecture

Suggested crates:

```toml
clap = "4"
serde = "1"
serde_json = "1"
toml = "0.8"
tokio = "1"
rusqlite = "0.32"
uuid = "1"
chrono = "0.4"
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

Optional later:

```toml
opentelemetry = "0.27"
opentelemetry-otlp = "0.27"
ratatui = "0.29"
```

## Internal Modules

```text
src/
  main.rs
  cli.rs
  config.rs
  db.rs
  event.rs
  stats.rs
  recommend.rs
  adapters/
    mod.rs
    codex.rs
    claude_code.rs
    opencode.rs
    openclaw.rs
    hermes.rs
  mcp/
    mod.rs
    server.rs
    tools.rs
  export/
    jsonl.rs
    otel.rs
```

## Codex Adapter MVP

The Codex adapter should start simple:

1. Launch Codex as a child process.
2. Create a `session_start` event.
3. Stream stdout/stderr.
4. Detect obvious command/tool boundaries when possible.
5. Allow Codex to explicitly call `skilltrace` through MCP for accurate events.
6. Create `session_end` event.

Do not overfit to fragile terminal parsing. Prefer explicit MCP event recording whenever possible.

## MCP Design

`skilltrace mcp` should run over stdio.

The MCP server should expose tools that agents can call.

### Tool: `record_skill_start`

Input:

```json
{
  "skill": "repo_search",
  "task_id": "optional-task-id",
  "planner_reason": "Need to inspect source files",
  "confidence": 0.77,
  "alternatives": ["ripgrep", "semantic_search"]
}
```

Output:

```json
{
  "event_id": "uuid",
  "status": "recorded"
}
```

### Tool: `record_skill_end`

Input:

```json
{
  "event_id": "uuid",
  "success": true,
  "output_summary": "Found relevant files",
  "error": null
}
```

Output:

```json
{
  "status": "recorded"
}
```

### Tool: `get_stats`

Input:

```json
{
  "since": "30d",
  "agent": "codex"
}
```

Output:

```json
{
  "skills": [
    {
      "skill": "repo_search",
      "uses": 37,
      "success_rate": 0.73,
      "avg_duration_ms": 980
    }
  ]
}
```

## Evaluation Metrics

Track:

- usage count
- success rate
- failure rate
- average latency
- retry count
- fallback count
- co-occurrence with other skills
- unused skills
- high-cost skills
- skill chains before failure
- skill chains before success
- planner confidence vs actual success
- selected skill vs alternatives

## Recommendation Logic

Initial heuristic rules:

### Dead skill

```text
defined but unused for N days
```

### Weak skill

```text
high usage + low success rate
```

### Hidden gem

```text
low usage + high success rate
```

### Overlapping skills

```text
two skills frequently appear in same task with similar outcomes
```

### Planner mismatch

```text
high confidence + low success
```

### Retry hotspot

```text
skill has unusually high retry count
```

## Privacy Requirements

Default behavior:

- Do not store full prompts.
- Do not store full model outputs.
- Store summaries only.
- Allow raw capture only with explicit config.
- Redact common secrets.
- Keep all data local by default.

## Future Pathway

### Phase 1: Codex MVP

- CLI
- SQLite storage
- JSONL mirror
- Codex wrapper
- MCP server
- stats/failures/timeline/recommend

### Phase 2: Claude Code

Add adapter for Claude Code MCP workflows.

Focus:

- MCP tool call tracing
- session metadata
- skill event recording
- command wrapper support

### Phase 3: OpenCode

Add OpenCode adapter.

Focus:

- terminal session tracing
- config-based MCP registration
- multi-session support

### Phase 4: OpenClaw

Add OpenClaw adapter.

Focus:

- ingest OpenTelemetry events
- normalize OTLP traces into `skilltrace` events
- map OpenClaw diagnostics to skill usage

### Phase 5: Hermes

Add Hermes adapter once integration surface is clarified.

Focus:

- wrapper mode first
- MCP mode if supported
- custom event import if available

### Phase 6: OpenTelemetry Export

Support:

```bash
skilltrace export otel
skilltrace serve-otel
```

This allows later compatibility with Grafana, Jaeger, Langfuse, Phoenix, or other observability systems.

## Non-Goals for MVP

Do not build:

- hosted service
- web dashboard
- team auth
- distributed tracing backend
- complex TUI
- raw prompt logging by default
- model-specific analytics requiring private APIs

## Desired Developer Experience

A user should be able to run:

```bash
cargo install skilltrace
skilltrace init
skilltrace mcp
skilltrace wrap codex
skilltrace stats
skilltrace recommend
```

Codex should be able to run:

```bash
skilltrace stats --since 7d
skilltrace failures
skilltrace recommend
```

and use the result to improve the local skill/toolset.

## Success Criteria

The MVP is successful when it can:

1. Record Codex skill events.
2. Store events locally.
3. Expose an MCP server.
4. Produce skill usage stats.
5. Identify unused skills.
6. Identify unreliable skills.
7. Recommend skillset changes.
8. Export JSONL.
9. Work without a cloud service.
10. Be understandable and modifiable by AI coding agents.
