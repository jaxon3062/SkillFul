# Reframed Local MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rework `skilltrace` from a manually defined skill-catalog tracer into a hook-friendly local telemetry service that records agent-discovered skills and concurrent skill usage events.

**Architecture:** Preserve the current SQLite/JSONL event foundation, MCP stdio server, privacy sanitizer, and wrapper session correlation. Add first-class skill inventory storage, explicit session/inventory recording APIs, concurrency-safe intake behavior, and reports that compare discovered skills against observed skill lifecycle events instead of reading `skills.toml`.

**Tech Stack:** Rust 2024, `clap`, `rusqlite`, `serde`, `serde_json`, `toml`, `chrono`, `uuid`, `anyhow`, `tokio`, an HTTP server crate selected with current docs during implementation, integration tests via `cargo test`.

## Global Constraints

- `skilltrace` observes; agent tools decide skill discovery and usage.
- Do not require users to duplicate available skills in `skills.toml`.
- Prefer hooks, plugins, explicit MCP calls, and local intake over terminal parsing.
- Preserve privacy defaults: summaries by default, no raw prompts by default, no raw outputs by default.
- Preserve append-only event semantics.
- Keep SQLite and JSONL behavior aligned for every persisted event.
- Concurrent hook, MCP, wrapper, and subagent requests must not corrupt SQLite rows or JSONL lines.
- Do not rely on a single global "current session" when explicit session/subagent ids are available.
- MCP changes must return protocol-shaped JSON-RPC errors instead of crashing the server.
- Keep the old Phase 1 completion plan as historical context; this plan is the next implementation direction.

---

## File Structure

- Modify `src/db.rs`: add skill inventory schema, inventory insert/query methods, idempotent event insertion, and report queries that use inventory.
- Modify `src/event.rs`: allow optional caller-provided event ids and add correlation metadata through `metadata_json` first, avoiding a broad schema migration for subagent fields in the first task.
- Create `src/inventory.rs`: define `SkillInventoryRecord`, `DiscoveredSkill`, and payload conversion helpers.
- Modify `src/main.rs`: expose the new `inventory` module.
- Modify `src/cli.rs`: add `serve`, add CLI support for `skills_discovered` or a dedicated inventory command, and remove `skills.toml` as the default unused/recommend path.
- Modify `src/mcp/tools.rs`: add `record_session_start`, `record_session_end`, and `record_skills_discovered` tool schemas.
- Modify `src/mcp/server.rs`: implement the new MCP tools and update recommendations to use inventory-backed reports.
- Modify `src/stats.rs`: change `UnusedSkillsReport` to compare inventory rows against observed skill events.
- Modify `src/recommend.rs`: remove required `defined_skills_path`, use inventory-backed unused/stale candidates, keep success/retry/overlap/chain heuristics.
- Modify `src/config.rs`: deprecate `[skills].definition_file`; keep backward-compatible parsing if needed, but do not use it for default reports.
- Modify `src/export/jsonl.rs` only if inventory export is added.
- Add or modify tests in `tests/mcp.rs`, `tests/skills_resolution.rs`, `tests/wrap.rs`, and a new `tests/inventory.rs`.
- Update `project_proposal.md` and `AGENTS.md` when implementation changes affect documented behavior.

## Task 1: Skill Inventory Storage

**Files:**
- Create: `src/inventory.rs`
- Modify: `src/main.rs`
- Modify: `src/db.rs`
- Test: add unit tests in `src/db.rs`

**Interfaces:**
- Produces: `SkillInventoryRecord`, `DiscoveredSkill`, `Database::record_skill_inventory(session_id, agent, adapter, skills)`, `Database::discovered_skills(since, agent)`.
- Consumes: existing `Database::open(...).initialize()` flow.

- [ ] **Step 1: Write the failing storage tests**

Add tests in `src/db.rs` that create an in-memory database, record two discovered skills for one session, record one duplicate skill again, and assert duplicate inventory rows are not created.

```rust
#[test]
fn skill_inventory_records_discovered_skills_idempotently() {
    let temp = tempfile::tempdir().expect("tempdir");
    let database = Database::open(&temp.path().join("skilltrace.db"))
        .expect("open")
        .initialize()
        .expect("init");
    database
        .record_skill_inventory(
            "session-1",
            "codex",
            "codex-hooks",
            &[
                DiscoveredSkill {
                    name: "writing-plans".to_string(),
                    description: Some("Plan implementation work".to_string()),
                    source: Some("project".to_string()),
                    path: Some(".agents/skills/writing-plans/SKILL.md".to_string()),
                    compatibility: vec!["codex".to_string()],
                    metadata_json: None,
                },
                DiscoveredSkill {
                    name: "verification-before-completion".to_string(),
                    description: None,
                    source: Some("global".to_string()),
                    path: None,
                    compatibility: vec![],
                    metadata_json: None,
                },
            ],
        )
        .expect("record inventory");
    database
        .record_skill_inventory(
            "session-1",
            "codex",
            "codex-hooks",
            &[DiscoveredSkill {
                name: "writing-plans".to_string(),
                description: Some("Plan implementation work".to_string()),
                source: Some("project".to_string()),
                path: Some(".agents/skills/writing-plans/SKILL.md".to_string()),
                compatibility: vec!["codex".to_string()],
                metadata_json: None,
            }],
        )
        .expect("record duplicate");

    let rows = database.discovered_skills(None, None).expect("rows");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| row.name == "writing-plans"));
}
```

- [ ] **Step 2: Run the failing test**

Run: `cargo test skill_inventory_records_discovered_skills_idempotently`

Expected: fail because `DiscoveredSkill`, `record_skill_inventory`, and `discovered_skills` do not exist.

- [ ] **Step 3: Implement minimal inventory types and schema**

Create `src/inventory.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredSkill {
    pub name: String,
    pub description: Option<String>,
    pub source: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub compatibility: Vec<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillInventoryRecord {
    pub session_id: String,
    pub name: String,
    pub description: Option<String>,
    pub source: Option<String>,
    pub path: Option<String>,
    pub compatibility: Vec<String>,
    pub metadata_json: Option<String>,
    pub discovered_at: String,
}
```

Add `mod inventory;` in `src/main.rs`.

Add `SKILL_INVENTORY_SCHEMA` to `src/db.rs`:

```rust
const SKILL_INVENTORY_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS skill_inventory (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  agent TEXT NOT NULL,
  adapter TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  source TEXT,
  path TEXT,
  compatibility_json TEXT NOT NULL,
  metadata_json TEXT,
  discovered_at TEXT NOT NULL,
  UNIQUE(session_id, name, path)
);
"#;
```

Call it from `Database::initialize`.

- [ ] **Step 4: Implement inventory insert/query**

Add methods to `Database`:

```rust
pub fn record_skill_inventory(
    &self,
    session_id: &str,
    agent: &str,
    adapter: &str,
    skills: &[DiscoveredSkill],
) -> Result<()> {
    let discovered_at = chrono::Utc::now().to_rfc3339();
    let tx = self.connection.unchecked_transaction().context("failed to start inventory transaction")?;
    for skill in skills {
        let compatibility_json = serde_json::to_string(&skill.compatibility)
            .context("failed to encode skill compatibility")?;
        tx.execute(
            "INSERT INTO skill_inventory (
               id, session_id, agent, adapter, name, description, source, path,
               compatibility_json, metadata_json, discovered_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(session_id, name, path) DO UPDATE SET
               description = COALESCE(excluded.description, skill_inventory.description),
               source = COALESCE(excluded.source, skill_inventory.source),
               compatibility_json = excluded.compatibility_json,
               metadata_json = COALESCE(excluded.metadata_json, skill_inventory.metadata_json)",
            params![
                uuid::Uuid::new_v4().to_string(),
                session_id,
                agent,
                adapter,
                skill.name,
                skill.description,
                skill.source,
                skill.path,
                compatibility_json,
                skill.metadata_json,
                discovered_at,
            ],
        )
        .context("failed to insert skill inventory row")?;
    }
    tx.commit().context("failed to commit skill inventory")?;
    Ok(())
}
```

Also add `discovered_skills(since: Option<&str>, agent: Option<&str>)`.

- [ ] **Step 5: Run the test**

Run: `cargo test skill_inventory_records_discovered_skills_idempotently`

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/inventory.rs src/db.rs
git commit -m "feat: add skill inventory storage"
```

## Task 2: Inventory-Backed Unused Reports

**Files:**
- Modify: `src/stats.rs`
- Modify: `src/db.rs`
- Modify: `src/cli.rs`
- Test: `tests/inventory.rs` or focused unit tests in `src/stats.rs`

**Interfaces:**
- Consumes: `Database::discovered_skills`, `Database::observed_skills`.
- Produces: `skilltrace unused` that no longer requires `--defined-skills`.

- [ ] **Step 1: Write failing report test**

Create `tests/inventory.rs` that initializes a temp `SKILLTRACE_HOME`, records a `skills_discovered` inventory with two skills, records a `skill_end` for one skill, then asserts `skilltrace unused` reports only the unused discovered skill.

- [ ] **Step 2: Run failing test**

Run: `cargo test unused_reports_discovered_but_unobserved_skills`

Expected: fail because `unused` still reads `skills.toml`.

- [ ] **Step 3: Replace `UnusedSkillsReport` input model**

Change `UnusedSkillsReport` to accept discovered skill names and observed skill names:

```rust
impl UnusedSkillsReport {
    pub fn from_discovered_and_observed(discovered: &[String], observed: &[String]) -> Self {
        let names = discovered
            .iter()
            .filter(|name| !observed.iter().any(|seen| seen == *name))
            .cloned()
            .collect();
        Self { names }
    }
}
```

- [ ] **Step 4: Update `unused_command`**

Load discovered skills from the database and remove default dependency on `resolve_defined_skills_path`.

- [ ] **Step 5: Run focused tests**

Run: `cargo test unused`

Expected: pass after updating tests that previously asserted configured skill definition resolution.

- [ ] **Step 6: Commit**

```bash
git add src/stats.rs src/db.rs src/cli.rs tests/inventory.rs tests/skills_resolution.rs
git commit -m "feat: report unused discovered skills"
```

## Task 3: MCP Session And Inventory Recording Tools

**Files:**
- Modify: `src/mcp/tools.rs`
- Modify: `src/mcp/server.rs`
- Test: `tests/mcp.rs`

**Interfaces:**
- Produces MCP tools: `skilltrace.record_session_start`, `skilltrace.record_session_end`, `skilltrace.record_skills_discovered`.
- Consumes: existing `ensure_session`, `persist_event`, and `Database::record_skill_inventory`.

- [ ] **Step 1: Write failing MCP tests**

Add tests that call:

```json
{"name":"skilltrace.record_session_start","arguments":{"session_id":"session-1","agent":"codex","adapter":"codex-hooks"}}
```

and:

```json
{"name":"skilltrace.record_skills_discovered","arguments":{"session_id":"session-1","agent":"codex","adapter":"codex-hooks","skills":[{"name":"writing-plans"}]}}
```

Assert the session exists and the inventory row is queryable through the database.

- [ ] **Step 2: Run failing tests**

Run: `cargo test mcp_record_session_start_and_skills_discovered`

Expected: fail because the tools are not listed or handled.

- [ ] **Step 3: Add MCP tool schemas**

Update `src/mcp/tools.rs` with schemas for:

```text
skilltrace.record_session_start
skilltrace.record_session_end
skilltrace.record_skills_discovered
```

- [ ] **Step 4: Implement handlers**

In `handle_tool_call`, add branches that upsert sessions, mark sessions ended, and record inventory. Return:

```json
{"status":"recorded","session_id":"session-1"}
```

for session calls and:

```json
{"status":"recorded","skills":1}
```

for inventory calls.

- [ ] **Step 5: Run focused tests**

Run: `cargo test mcp`

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/mcp/tools.rs src/mcp/server.rs tests/mcp.rs
git commit -m "feat: add mcp session and inventory tools"
```

## Task 4: Concurrent Intake Safety

**Files:**
- Modify: `src/db.rs`
- Modify: `src/cli.rs`
- Modify: `src/export/jsonl.rs` if JSONL append path is split out
- Test: `tests/mcp.rs` or new `tests/concurrency.rs`

**Interfaces:**
- Produces: idempotent event insertion when caller supplies an id; serialized JSONL append behavior; no reliance on ambiguous runtime session fallback for parallel sessions.

- [ ] **Step 1: Write failing concurrent write test**

Create a test that spawns multiple threads or processes writing events with explicit session ids and asserts all rows and JSONL lines are present.

- [ ] **Step 2: Run failing test**

Run: `cargo test concurrent_event_writes_do_not_drop_or_interleave_records`

Expected: fail if JSONL writes interleave or event ids conflict.

- [ ] **Step 3: Make event insertion idempotent**

Change event insertion to `INSERT ... ON CONFLICT(id) DO NOTHING` only when the incoming event id is caller supplied. Keep generated ids unique for normal calls.

- [ ] **Step 4: Serialize JSONL append**

Ensure each event append opens the file with append mode and writes exactly one complete line per event. If needed, route appends through one helper function that writes `serde_json::to_writer` plus `writeln!`.

- [ ] **Step 5: Make ambiguous session fallback explicit**

When more than one runtime session is active and no `SKILLTRACE_SESSION_ID` is present, return an error from hook/MCP paths that require a session instead of silently picking one.

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test concurrent_event_writes_do_not_drop_or_interleave_records
cargo test mcp
cargo test wrap
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/db.rs src/cli.rs src/export/jsonl.rs tests/concurrency.rs tests/mcp.rs tests/wrap.rs
git commit -m "fix: make concurrent skilltrace intake safe"
```

## Task 5: Inventory-Backed Recommendations

**Files:**
- Modify: `src/recommend.rs`
- Modify: `src/cli.rs`
- Modify: `src/mcp/server.rs`
- Test: `src/recommend.rs`, `tests/mcp.rs`

**Interfaces:**
- Consumes: discovered inventory rows, observed skills, skill stats, skill history, overlap rows, chain rows.
- Produces: recommendations that no longer require a configured skill definition file.

- [ ] **Step 1: Write failing recommendation test**

Add a test where `writing-plans` and `verification-before-completion` are discovered, only `writing-plans` is observed, and recommendations include demoting or reviewing `verification-before-completion`.

- [ ] **Step 2: Run failing test**

Run: `cargo test recommendations_use_discovered_inventory`

Expected: fail because `build_recommendations` currently requires `skills.toml`.

- [ ] **Step 3: Change `build_recommendations` signature**

Replace `defined_skills_path: &Path` with `discovered_skills: &[String]`.

- [ ] **Step 4: Update CLI and MCP callers**

Load discovered skill names from the database and pass them to `build_recommendations`.

- [ ] **Step 5: Remove default config dependency**

Stop loading `[skills].definition_file` for `recommend`. Leave config parsing compatible if removing the field would break existing config files.

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test recommend
cargo test mcp
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add src/recommend.rs src/cli.rs src/mcp/server.rs tests/mcp.rs
git commit -m "feat: recommend from discovered skill inventory"
```

## Task 6: Hook-Friendly Local Intake

**Files:**
- Create: `src/intake/mod.rs`
- Create: `src/intake/payload.rs`
- Create: `src/intake/server.rs`
- Modify: `src/main.rs`
- Modify: `src/cli.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Test: new intake unit tests and one local-server integration test.

**Interfaces:**
- Produces: `skilltrace serve --bind 127.0.0.1:0` command, `/events` JSON POST endpoint, and payload handlers that reuse the same recording functions as MCP.
- Consumes: `Database`, `StoragePaths`, `AppConfig`, `EventRecord`, `DiscoveredSkill`.

- [ ] **Step 1: Confirm the HTTP server dependency**

Use Context7 to fetch current docs for the selected Rust HTTP server crate before implementation. Prefer a small `tokio`-compatible crate so `skilltrace serve` can accept local hook/plugin POST requests without adding a large framework surface.

- [ ] **Step 2: Write payload tests**

Test that a `skills_discovered` JSON payload converts into `DiscoveredSkill` rows and that a `skill_start` payload converts into an `EventRecord`.

- [ ] **Step 3: Implement shared intake handlers**

Create functions such as:

```rust
pub fn record_session_start(paths: &StoragePaths, payload: SessionStartPayload) -> Result<String>;
pub fn record_skills_discovered(paths: &StoragePaths, payload: SkillsDiscoveredPayload) -> Result<usize>;
pub fn record_event(paths: &StoragePaths, payload: EventPayload) -> Result<String>;
```

MCP should later call these functions instead of duplicating persistence logic.

- [ ] **Step 4: Add the HTTP endpoint**

Implement:

```text
POST /events
Content-Type: application/json
```

Accepted payload shape:

```json
{
  "event_type": "skill_start",
  "session_id": "session-1",
  "agent": "codex",
  "adapter": "codex-hooks",
  "skill": "writing-plans"
}
```

Return:

```json
{
  "status": "recorded",
  "event_id": "generated-or-caller-id"
}
```

For `skills_discovered`, return:

```json
{
  "status": "recorded",
  "skills": 2
}
```

- [ ] **Step 5: Add `skilltrace serve`**

Add `Commands::Serve(ServeArgs)` with:

```rust
#[derive(Debug, Args)]
struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1:0")]
    bind: String,
}
```

Print the bound address on startup so hook installers can discover the port.

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test intake
cargo check
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add src/intake src/main.rs src/cli.rs Cargo.toml Cargo.lock
git commit -m "feat: add hook-friendly local intake foundation"
```

## Task 7: Documentation And Migration Cleanup

**Files:**
- Modify: `project_proposal.md`
- Modify: `AGENTS.md`
- Modify: `docs/superpowers/plans/2026-06-29-reframed-local-mvp.md`
- Modify: `skills.toml` only if removing or marking it legacy is explicitly approved.

**Interfaces:**
- Produces: docs that describe `skills.toml` as legacy/fallback rather than the product model.

- [ ] **Step 1: Update docs after implementation**

Ensure docs mention:

- agent-discovered inventory is authoritative
- `skills.toml` is deprecated or legacy-only
- wrapper is fallback correlation, not primary skill tracing
- concurrency is required for parallel subagents
- `skilltrace serve` and MCP tools share the same intake model

- [ ] **Step 2: Run doc consistency scans**

Run:

```bash
rg -n "definition_file|defined-skills|skills\\.toml|Codex MVP|session leases" AGENTS.md project_proposal.md docs/superpowers/plans
```

Expected: remaining matches are historical or explicitly marked legacy.

- [ ] **Step 3: Run full verification**

Run:

```bash
cargo fmt
cargo test
```

Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add AGENTS.md project_proposal.md docs/superpowers/plans/2026-06-29-reframed-local-mvp.md
git commit -m "docs: align plan with hook-driven skill telemetry"
```

## Self-Review

- Spec coverage: The plan covers inventory ingestion, concurrent intake, MCP/session tools, reports, recommendations, local intake, and documentation migration.
- Placeholder scan: The plan avoids incomplete markers and unspecified implementation steps.
- Type consistency: `DiscoveredSkill`, `SkillInventoryRecord`, and inventory-backed report names are used consistently across tasks.
