# Phase 1 Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the remaining Codex MVP Phase 1 milestones for richer workflow tracing, chain-aware recommendations, session correlation, and stronger privacy defaults.

**Architecture:** Keep `skilltrace` local-first and append-only. Add value through explicit event records, SQLite-backed reports, and MCP/CLI-visible outputs without introducing raw prompt/output capture by default.

**Tech Stack:** Rust 2024, `clap`, `rusqlite`, `serde`, `serde_json`, `toml`, `chrono`, `uuid`, `sha2`, integration tests via `cargo test`.

## Global Constraints

- Keep CLI output table-first and agent-readable.
- Prefer explicit event recording over fragile terminal parsing.
- Preserve privacy defaults: summaries by default, no raw prompts by default, no raw outputs by default.
- Preserve append-only event semantics.
- Keep SQLite and JSONL behavior aligned for every persisted event.
- MCP changes must return protocol-shaped JSON-RPC errors instead of crashing the server.
- Commit each finished milestone separately.
- Each milestone must be implemented in a repo-local git worktree under `.worktrees/`.
- Merge to `main` only after implementation review and verification show no core feature conflict.

---

### Task 1: Stronger Privacy Sanitization

**Files:**
- Modify: `src/privacy.rs`
- Test: `tests/privacy.rs`

**Interfaces:**
- Consumes: `privacy::sanitize_event(&mut EventRecord, &PrivacyConfig)`
- Produces: stronger sanitization for summary fields, error, planner reason, alternatives, and metadata JSON without changing public CLI arguments.

- [ ] **Step 1: Write failing tests**

Add integration tests proving default hashing removes these sensitive forms from stdout and JSONL:

```rust
#[test]
fn event_hashes_url_query_secrets_by_default() {
    // `--input-summary "GET https://example.test/path?token=super-secret-token&safe=value"`
    // Assert stdout and JSONL do not contain `super-secret-token`.
    // Assert JSONL still contains `token=sha256:`.
}

#[test]
fn event_hashes_json_like_secret_fields_by_default() {
    // `--output-summary "{\"api_key\":\"abc123secret\",\"safe\":\"visible\"}"`
    // Assert stdout and JSONL do not contain `abc123secret`.
    // Assert JSONL still contains `api_key`.
}
```

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test privacy`

Expected: the new tests fail because URL query secrets and JSON-like secret values are not fully sanitized.

- [ ] **Step 3: Implement minimal sanitizer improvements**

Update `src/privacy.rs` so sensitive key/value detection handles:

```text
token=secret
token:secret
token:"secret"
"token":"secret"
https://host/path?token=secret&safe=value
Authorization: Bearer secret
```

Do not add dependencies. Preserve existing `sha256:<16 hex chars>` output shape.

- [ ] **Step 4: Verify focused tests pass**

Run: `cargo test privacy`

Expected: all privacy tests pass.

- [ ] **Step 5: Verify broader suite**

Run: `cargo test`

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/privacy.rs tests/privacy.rs
git commit -m "Strengthen privacy sanitization"
```

### Task 2: Chain-Aware Recommendations

**Files:**
- Modify: `src/db.rs`
- Modify: `src/recommend.rs`
- Test: unit tests in `src/recommend.rs` and `src/db.rs`

**Interfaces:**
- Consumes: existing event ordering by `session_id`, `timestamp`, and `id`.
- Produces: a new chain/co-occurrence query result type and recommendations that mention repeated adjacent skill chains.

- [ ] **Step 1: Write failing tests**

Add tests showing:

```rust
// db.rs
// Given session-1 has skill_end: repo_search, edit_file, run_tests
// and session-2 has skill_end: repo_search, edit_file
// database.skill_chains(None) returns repo_search -> edit_file with count 2.

// recommend.rs
// Given chain rows for repo_search -> edit_file repeated at least twice,
// build_recommendations includes a line containing
// "Review chain `repo_search` -> `edit_file`".
```

- [ ] **Step 2: Verify tests fail**

Run: `cargo test recommend`

Expected: chain recommendation test fails before implementation.

Run: `cargo test db`

Expected: chain query test fails before implementation.

- [ ] **Step 3: Implement storage query**

Add a `SkillChainRow` struct and `Database::skill_chains(agent: Option<&str>) -> Result<Vec<SkillChainRow>>`.

The query should consider only `event_type = 'skill_end'` rows with non-null `skill`, ordered per session by `timestamp ASC, id ASC`, and count adjacent pairs.

- [ ] **Step 4: Implement recommendation heuristic**

Extend `recommend::build_recommendations` to accept chain rows. Recommend reviewing a chain when the same adjacent pair appears at least twice.

Message shape:

```text
Review chain `repo_search` -> `edit_file`. Reason: observed as an adjacent skill sequence 2 times.
```

- [ ] **Step 5: Wire CLI and MCP recommendation callers**

Update `recommend_command()` and `skilltrace.get_recommendations` to pass chain rows.

- [ ] **Step 6: Verify**

Run:

```bash
cargo test recommend
cargo test db
cargo test mcp
cargo test
```

- [ ] **Step 7: Commit**

```bash
git add src/db.rs src/recommend.rs src/cli.rs src/mcp/server.rs
git commit -m "Add chain-aware recommendations"
```

### Task 3: Wrapper Command Boundary Events

**Files:**
- Modify: `src/cli.rs`
- Test: `tests/wrap.rs`

**Interfaces:**
- Consumes: existing `wrap` session lifecycle.
- Produces: explicit child command boundary events inside the wrapper session without raw command arguments by default.

- [ ] **Step 1: Write failing tests**

Add a wrap integration test proving a successful wrapped command records these event types in JSONL:

```text
session_start
command_start
command_end
session_end
```

Assert:

```text
command_start.skill == "wrapped_command"
command_end.skill == "wrapped_command"
command_end.success == true
raw command arguments remain absent by default
```

Add a failure test proving `command_end.success == false` and the wrapper still returns the child exit status.

- [ ] **Step 2: Verify tests fail**

Run: `cargo test wrap`

Expected: new command boundary assertions fail because only session-level events exist.

- [ ] **Step 3: Implement boundary events**

In `wrap_command`, record:

```text
command_start
command_end
```

Use `skill = Some("wrapped_command")`. The command start summary must use the existing privacy-aware `wrap_input_summary`. The command end summary must include only exit status or signal information, not raw command arguments.

- [ ] **Step 4: Verify**

Run:

```bash
cargo test wrap
cargo test
```

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs tests/wrap.rs
git commit -m "Record wrapper command boundaries"
```

### Task 4: Session Correlation for Wrapped and MCP Workflows

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/mcp/server.rs`
- Modify: `src/config.rs`
- Test: `tests/wrap.rs`, `tests/mcp.rs`, and unit tests in `src/config.rs`

**Interfaces:**
- Consumes: `RuntimeState::preferred_session_id()` and wrapper sessions.
- Produces: deterministic session correlation when a wrapped child invokes `skilltrace event` or MCP tools.

- [ ] **Step 1: Write failing tests**

Add tests proving:

```text
`skilltrace wrap env` exposes SKILLTRACE_SESSION_ID to the child.
`skilltrace event skill_end --skill child_tool` inside a wrapped shell records into the wrapper session without requiring --session-id.
MCP record_skill_start without a session_id uses SKILLTRACE_SESSION_ID when present.
```

- [ ] **Step 2: Verify tests fail**

Run:

```bash
cargo test wrap
cargo test mcp
```

Expected: the environment correlation assertions fail before implementation.

- [ ] **Step 3: Implement wrapper environment export**

When spawning the wrapped child, set:

```text
SKILLTRACE_SESSION_ID=<session.id>
SKILLTRACE_AGENT=<agent>
SKILLTRACE_ADAPTER=<adapter>
```

- [ ] **Step 4: Implement CLI event fallback**

When `skilltrace event` has no `--session-id`, prefer `SKILLTRACE_SESSION_ID` over runtime state if the environment variable is present.

- [ ] **Step 5: Implement MCP fallback**

When `record_skill_start` has no `session_id`, prefer `SKILLTRACE_SESSION_ID` over runtime state if the environment variable is present.

- [ ] **Step 6: Verify**

Run:

```bash
cargo test wrap
cargo test mcp
cargo test
```

- [ ] **Step 7: Commit**

```bash
git add src/cli.rs src/mcp/server.rs src/config.rs tests/wrap.rs tests/mcp.rs
git commit -m "Correlate wrapped child events"
```
