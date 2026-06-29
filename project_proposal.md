# Project Proposal: `skilltrace`

## Summary

Build `skilltrace`, a Rust-based local telemetry tool for observing how AI coding agents discover and use agent skills.

Here, "skill" means the agent-skill concept popularized by Claude Code and now adopted by other agent tools: reusable `SKILL.md`-style instructions, workflows, scripts, and related assets that an agent can discover and load on demand.

`skilltrace` should not define a user's available skills. Agent tools already have their own discovery rules for global and project-local skills. Instead, `skilltrace` should receive skill inventory and usage events from the running agent through hooks, plugins, MCP tools, or a small local intake API.

The product is a local-first background observer:

- agent discovers available skills
- agent reports the discovered skill inventory to `skilltrace`
- agent reports skill lifecycle events as skills are used
- `skilltrace` stores those observations locally
- user reviews reports to adjust, merge, remove, fix, or promote skills

Initial integration target: **Codex CLI**, because this repository already has Codex-oriented storage, MCP, wrapper, and reporting code.

High-value follow-up targets: **Claude Code** and **OpenCode**, because both have strong hook/plugin surfaces that fit this model well.

## Core Goal

Help users evaluate and improve their real agent skill set by answering:

- Which discovered skills are used most?
- Which discovered skills are never used?
- Which discovered skills are stale across recent sessions?
- Which skills succeed, fail, or retry most often?
- Which skills are selected but later abandoned?
- Which skills are slow or noisy?
- Which skills tend to appear together?
- Which skill chains correlate with successful or failed tasks?
- Which skills appear redundant, too broad, too narrow, or unreliable?

The tool should make the user's existing skill directories observable. It should not ask the user to duplicate that inventory in a separate `skills.toml` file.

## Product Shape

`skilltrace` should be:

- Rust-based
- CLI-first
- local-first
- background-process friendly
- hook-friendly
- MCP-compatible
- append-only by default
- agent-readable
- usable without a web server or hosted account
- respectful of agent-specific skill discovery rules

## Design Principle

`skilltrace` observes; agent tools decide.

The agent tool remains responsible for:

- finding global skills
- finding project-local skills
- applying skill permissions
- deciding when a skill is relevant
- loading skill content
- running any skill-provided scripts or references

`skilltrace` is responsible for:

- accepting normalized telemetry from agents
- preserving session, skill inventory, and skill usage history
- reporting usage, health, and recommendation signals
- staying local and privacy-preserving by default

## MVP Data Flow

The ideal flow is hook-driven:

1. Agent session starts.
2. A session-start hook calls `skilltrace` with session metadata.
3. The agent or adapter sends the current available skill inventory.
4. When the agent starts using a skill, a hook or MCP tool records `skill_start`.
5. When the skill finishes, fails, or is abandoned, a hook or MCP tool records `skill_end` or `skill_error`.
6. When the session ends, a hook records `session_end`.
7. `skilltrace` reports discovered-vs-used skills and usage quality over time.

Example event sequence:

```text
session_start
skills_discovered
skill_start writing-plans
skill_end writing-plans success
skill_start verification-before-completion
skill_end verification-before-completion success
session_end
```

## MVP Scope

### Primary Commands

```bash
skilltrace init
skilltrace serve
skilltrace mcp
skilltrace event
skilltrace stats
skilltrace timeline
skilltrace failures
skilltrace unused
skilltrace recommend
skilltrace export jsonl
```

### MVP Features

1. Local initialization and storage.
2. Local intake service for hook/plugin calls.
3. MCP server exposing the same core recording/reporting operations.
4. First-class session lifecycle recording.
5. First-class discovered skill inventory recording.
6. First-class skill lifecycle recording.
7. Concurrent event ingestion from parallel agents and subagents.
8. Reports based on discovered skills, not a manually maintained catalog.
9. JSONL export.
10. Privacy-preserving summaries by default.

### MVP Non-Goals

Do not build:

- hosted service
- web dashboard
- team auth
- distributed tracing backend
- complex TUI
- raw prompt logging by default
- model-specific analytics requiring private APIs
- a duplicate user-maintained skill registry

## Skill Inventory Model

The available skill set should come from the agent or adapter at session start.

Example `skills_discovered` payload:

```json
{
  "event_type": "skills_discovered",
  "session_id": "session-123",
  "agent": "codex",
  "adapter": "codex-hooks",
  "cwd": "/repo",
  "skills": [
    {
      "name": "writing-plans",
      "description": "Use when you have a spec or requirements for a multi-step task, before touching code",
      "source": "project",
      "path": ".agents/skills/writing-plans/SKILL.md",
      "compatibility": ["codex"],
      "metadata": {}
    },
    {
      "name": "verification-before-completion",
      "description": "Use before claiming work is complete or passing",
      "source": "global",
      "path": "~/.agents/skills/verification-before-completion/SKILL.md",
      "compatibility": ["codex", "claude-code"],
      "metadata": {}
    }
  ]
}
```

Only `name` is required for MVP reporting. Description, path, source, compatibility, and metadata improve diagnostics but should be optional.

## Definition of Skill Usage

A skill usage is an agent-observed lifecycle event for a named agent skill.

For MVP, prefer explicit events:

- `skill_start`
- `skill_end`
- `skill_error`
- `skill_abandoned`

Example `skill_end` payload:

```json
{
  "event_type": "skill_end",
  "session_id": "session-123",
  "task_id": "optional-task-id",
  "skill": "writing-plans",
  "agent": "codex",
  "adapter": "codex-hooks",
  "timestamp": "2026-06-29T10:00:00Z",
  "duration_ms": 5000,
  "success": true,
  "error": null,
  "retry_count": 0,
  "input_summary": "Create implementation plan from approved spec",
  "output_summary": "Plan written to docs/superpowers/plans/...",
  "metadata": {}
}
```

Planner fields such as `planner_reason`, `confidence`, and `alternatives` are useful later, but they should not be required for the MVP. Many current agents will not expose them reliably.

## Intake Surfaces

`skilltrace` should support multiple intake surfaces because agent tools expose different extension points.

### Local Service

```bash
skilltrace serve
```

Runs a local background intake service. Hooks and plugins can POST events to it or call a lightweight local command that forwards to it.

This should become the preferred integration path because hooks are often shell commands, HTTP callbacks, or plugin functions.

### MCP Server

```bash
skilltrace mcp
```

Exposes recording and reporting tools to agents that can call MCP tools directly.

MVP tools:

```text
skilltrace.record_session_start
skilltrace.record_session_end
skilltrace.record_skills_discovered
skilltrace.record_skill_start
skilltrace.record_skill_end
skilltrace.record_event
skilltrace.get_stats
skilltrace.get_failures
skilltrace.get_recommendations
```

The existing `record_event`, `record_skill_start`, `record_skill_end`, `get_stats`, `get_failures`, and `get_recommendations` tools are a good foundation. The missing pieces are first-class session lifecycle tools and `record_skills_discovered`.

## Concurrency Requirements

`skilltrace` must assume multiple incoming requests can arrive at the same time.

Common cases:

- parallel subagents start and end skills concurrently
- multiple hooks fire for the same parent session in quick succession
- MCP tool calls and local service calls arrive during the same session
- wrapper-created sessions receive child-process events while the wrapper is also writing boundary events

The intake layer should therefore be safe for concurrent writes:

- every incoming event gets a unique event id
- callers may provide a stable `session_id`, `task_id`, `agent_id`, `subagent_id`, or `parent_session_id`
- writes are append-only and idempotent where a caller supplies an event id
- `record_skill_end` should correlate to a specific `skill_start` event id when available
- session state must not depend on a single global "current session" when multiple sessions are active
- SQLite writes should use transactions and a busy timeout or equivalent retry strategy
- JSONL mirroring should serialize append writes so lines are not interleaved
- report queries should tolerate partially completed sessions and in-flight skill starts

The current `SKILLTRACE_SESSION_ID` fallback is still useful, but it is not enough for parallel subagents. Adapters should pass explicit ids whenever the agent tool exposes them.

### CLI Event Recording

```bash
skilltrace event skill_start --skill writing-plans
skilltrace event skill_end --skill writing-plans --success true
```

Manual CLI recording remains useful for testing hooks and for agents that can only run shell commands.

### Process Wrapper

```bash
skilltrace wrap codex
```

The wrapper should be kept as a fallback session boundary tool, not the primary skill tracing mechanism.

Wrapper mode can still:

- create `session_start` and `session_end`
- expose `SKILLTRACE_SESSION_ID` to child processes
- correlate child hook/MCP events with the wrapper session
- record coarse command boundary events

Wrapper mode should not try to infer skill usage from terminal parsing. Accurate skill tracing should come from hooks, plugins, or explicit MCP calls.

## Storage

Use local SQLite by default.

Default path:

```bash
~/.skilltrace/skilltrace.db
```

Also mirror append-only events to JSONL:

```bash
~/.skilltrace/events.jsonl
```

### Current Tables That Remain Useful

The existing `sessions` and `events` tables remain useful and should be preserved.

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

### New Storage Needed

Add a first-class skill inventory table, or store equivalent normalized records.

Suggested table:

```sql
CREATE TABLE skill_inventory (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  source TEXT,
  path TEXT,
  compatibility_json TEXT,
  metadata_json TEXT,
  discovered_at TEXT NOT NULL,
  UNIQUE(session_id, name, path)
);
```

This replaces `skills.toml` as the source for unused-skill reporting.

Add optional correlation fields to events as the schema evolves:

```text
agent_id
subagent_id
parent_session_id
source_event_id
```

These fields let reports distinguish one parent session from concurrent subagent work without forcing every adapter to model subagents the same way.

## Reports

### Stats

```bash
skilltrace stats
skilltrace stats --since 7d
skilltrace stats --agent codex
skilltrace stats --skill writing-plans
```

Example:

```text
skill                          uses   success_rate   avg_ms   retries   failures
writing-plans                  12     0.92           4210     1         1
verification-before-completion 9      1.00           980      0         0
brainstorming                  5      0.80           1340     0         1
```

### Timeline

```bash
skilltrace timeline --last
skilltrace timeline --session <id>
```

### Failures

```bash
skilltrace failures
skilltrace failures --skill writing-plans
```

### Unused

```bash
skilltrace unused
skilltrace unused --since 30d
skilltrace unused --agent codex
```

This should compare skills discovered in session inventories against skills actually used in recorded lifecycle events.

It should not require:

```bash
skilltrace unused --defined-skills skills.toml
```

That flag can be removed, deprecated, or kept only as an import/testing fallback.

### Recommendations

```bash
skilltrace recommend
```

Initial recommendations should use discovered inventory and observed events:

- declared-by-agent but never used
- discovered often but unused recently
- high usage with low success rate
- high retries
- repeated adjacent skill chains
- frequently co-occurring skills that may overlap
- low usage with high success rate

## Recommendation Logic

### Unused Discovered Skill

```text
discovered in recent sessions but never used
```

### Stale Skill

```text
used historically, still discovered, but not used in N days
```

### Weak Skill

```text
high usage + low success rate
```

### Hidden Gem

```text
low usage + high success rate
```

### Overlapping Skills

```text
two skills frequently appear in the same sessions or adjacent chains
```

### Retry Hotspot

```text
skill has unusually high retry count
```

### Adapter Gap

```text
sessions have skill inventory but no skill lifecycle events
```

This indicates that the agent integration is discovering skills but not yet reporting usage accurately.

## Agent Integration Strategy

### Codex MVP

Codex should be supported first because this repository already has a Codex-first implementation baseline.

Codex integration should prefer lifecycle hooks and MCP calls:

- `SessionStart` hook records session start
- session-start hook or adapter reports discovered skills
- `PreToolUse` / `PostToolUse` can observe MCP tool calls and some shell/edit events
- explicit MCP calls record accurate skill usage when available
- wrapper mode correlates a child Codex process with `SKILLTRACE_SESSION_ID`

Known limitation: Codex hooks do not necessarily expose every internal skill-selection event as a stable first-class event. The MVP should support best-effort Codex integration while keeping the core data model general enough for agents with stronger skill hooks.

### Claude Code Follow-Up

Claude Code is a strong fit because it supports lifecycle and tool hooks, including session start/end, pre/post tool use, failure hooks, MCP tool hooks, HTTP hooks, and command hooks.

Claude Code integration should focus on:

- hook config examples
- command or HTTP hook transport into `skilltrace serve`
- `SessionStart` inventory reporting
- skill lifecycle reporting where available

### OpenCode Follow-Up

OpenCode is a strong fit because it has a plugin system and explicit agent skills loaded through a native `skill` tool.

OpenCode integration should focus on:

- plugin-based reporting
- observing native `skill` tool calls
- reporting discovered skills from OpenCode's discovery model
- using OpenCode's event hooks for lifecycle and tool execution

## Configuration

Default config should focus on storage, privacy, and intake surfaces.

```toml
[storage]
backend = "sqlite"
path = "~/.skilltrace/skilltrace.db"
jsonl_mirror = true

[server]
enabled = true
bind = "127.0.0.1:0"

[mcp]
enabled = true
transport = "stdio"

[privacy]
capture_raw_prompts = false
capture_raw_outputs = false
hash_sensitive_values = true

[adapters.codex]
enabled = true
preferred_intake = "hooks"
wrapper_fallback = true
```

Remove this from the core config:

```toml
[skills]
definition_file = "skills.toml"
```

`skills.toml` is no longer part of the main product model.

## Implemented Feature Assessment

The current codebase is not wasted. Most infrastructure still fits the rewritten plan, but several features need to be repositioned.

### Directly Usable

- Rust CLI scaffold.
- Local initialization.
- SQLite-backed `sessions` and `events`.
- JSONL mirroring/export.
- Manual event recording.
- Table-first stats, timeline, and failures reports.
- MCP stdio server.
- MCP `record_event`, `record_skill_start`, `record_skill_end`, `get_stats`, and `get_failures`.
- Runtime session state and `SKILLTRACE_SESSION_ID` correlation.
- Privacy defaults and sensitive summary sanitization.
- Wrapper session lifecycle recording.

### Usable With Reframing

- `skilltrace wrap`: keep as fallback session correlation, not as the primary tracing method.
- command boundary events: useful as coarse operational context, not as skill usage.
- recommendation heuristics: reuse success-rate, retry, co-occurrence, and chain logic, but change unused/stale signals to use discovered inventory instead of `skills.toml`.
- MCP `get_recommendations`: keep the tool, but update its implementation to stop requiring a skill definition file.

### Needs Replacement Or Deprecation

- `skills.toml` as the authoritative skill catalog.
- `[skills].definition_file` config.
- `skilltrace unused --defined-skills`.
- tests that assert recommendations or unused reports depend on configured skill definition paths.

### Missing For The Rewritten MVP

- `skilltrace serve` local intake service.
- first-class `record_session_start` and `record_session_end` MCP/intake operations.
- first-class `record_skills_discovered` MCP/intake operation.
- skill inventory storage.
- concurrent write handling for hook, MCP, wrapper, and subagent events.
- reports that compare discovered skills against used skills.
- agent hook/plugin installation examples.

## Rust Architecture

Current module structure can evolve without a rewrite:

```text
src/
  main.rs
  cli.rs
  config.rs
  db.rs
  event.rs
  stats.rs
  recommend.rs
  privacy.rs
  mcp/
    mod.rs
    server.rs
    tools.rs
  export/
    jsonl.rs
    mod.rs
    otel.rs
  adapters/
    mod.rs
    codex.rs
    claude_code.rs
    opencode.rs
    openclaw.rs
    hermes.rs
```

Suggested additions:

```text
src/
  inventory.rs
  intake/
    mod.rs
    server.rs
    payload.rs
  hooks/
    codex.rs
    claude_code.rs
    opencode.rs
```

The code should keep storage/reporting logic independent from any one agent. Adapters should normalize agent-specific hook/plugin payloads into a small shared event and inventory model.

## Privacy Requirements

Default behavior:

- Do not store full prompts.
- Do not store full model outputs.
- Store summaries only.
- Allow raw capture only with explicit config.
- Redact common secrets.
- Keep all data local by default.
- Treat skill descriptions and paths as potentially sensitive project metadata.

## Future Pathway

### Phase 1: Reframed Local MVP

- preserve current local storage and report commands
- add skill inventory storage
- add `record_skills_discovered`
- update unused/recommend reports to use discovered inventory
- keep MCP stdio support
- add local intake service
- keep wrapper as correlation fallback
- document a Codex hook-based integration

### Phase 2: Codex Integration Hardening

- provide installable Codex hook config
- record session lifecycle through hooks
- report discovered skills from Codex-visible skill metadata where possible
- correlate hook/MCP events through `SKILLTRACE_SESSION_ID`
- identify Codex-specific observability gaps explicitly in reports

### Phase 3: Claude Code Adapter

- provide Claude Code hook config
- support command, HTTP, or MCP-tool hook transport
- record session lifecycle, skill inventory, and skill usage
- document supported hook events and limitations

### Phase 4: OpenCode Plugin

- provide OpenCode plugin
- observe native skill tool calls
- report OpenCode skill inventory
- integrate with OpenCode session and tool events

### Phase 5: Broader Agent Support

Add adapters for other agents only when they expose one of:

- lifecycle hooks
- plugin event streams
- MCP tool calls
- stable local trace files
- OpenTelemetry or compatible structured traces

### Phase 6: Optional Observability Export

Support:

```bash
skilltrace export otel
skilltrace serve-otel
```

This is not part of the MVP. It should wait until the local event model is stable.

## Desired Developer Experience

Initial setup should feel like installing a local observer:

```bash
cargo install skilltrace
skilltrace init
skilltrace serve
skilltrace install-hooks codex
codex
skilltrace stats
skilltrace unused
skilltrace recommend
```

For MCP-capable agents:

```bash
skilltrace mcp
```

For fallback process correlation:

```bash
skilltrace wrap codex
```

## Success Criteria

The MVP is successful when it can:

1. Record agent sessions locally.
2. Record discovered skills from a running agent session.
3. Record skill start/end lifecycle events.
4. Store events and skill inventory locally.
5. Expose MCP tools for recording and reporting.
6. Provide a local intake path suitable for hooks/plugins.
7. Safely ingest simultaneous events from parallel subagents.
8. Produce skill usage stats.
9. Identify discovered-but-unused skills.
10. Identify unreliable or retry-heavy skills.
11. Recommend skill-set changes using observed data.
12. Export JSONL.
13. Work without a cloud service.
14. Avoid requiring a duplicate `skills.toml` catalog.
