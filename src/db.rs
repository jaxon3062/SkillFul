use std::{io::Write, path::Path, time::Duration};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::{config::SessionRecord, event::EventRecord};

const SESSION_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
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
"#;

const EVENT_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS events (
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
"#;

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)
            .with_context(|| format!("failed to open database {}", path.display()))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .with_context(|| format!("failed to set busy timeout for {}", path.display()))?;
        Ok(Self { connection })
    }

    pub fn initialize(self) -> Result<Self> {
        self.connection
            .execute_batch(SESSION_SCHEMA)
            .context("failed to initialize sessions schema")?;
        self.connection
            .execute_batch(EVENT_SCHEMA)
            .context("failed to initialize events schema")?;
        Ok(self)
    }

    pub fn upsert_session(&self, session: &SessionRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO sessions (id, agent, adapter, started_at, ended_at, cwd, repo, branch, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
               agent = excluded.agent,
               adapter = excluded.adapter,
               ended_at = COALESCE(excluded.ended_at, sessions.ended_at),
               cwd = COALESCE(excluded.cwd, sessions.cwd),
               repo = COALESCE(excluded.repo, sessions.repo),
               branch = COALESCE(excluded.branch, sessions.branch),
               metadata_json = COALESCE(excluded.metadata_json, sessions.metadata_json)",
            params![
                session.id,
                session.agent,
                session.adapter,
                session.started_at,
                session.ended_at,
                session.cwd,
                session.repo,
                session.branch,
                session.metadata_json,
            ],
        ).context("failed to upsert session")?;
        Ok(())
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>> {
        self.connection
            .query_row(
                "SELECT id, agent, adapter, started_at, ended_at, cwd, repo, branch, metadata_json
                 FROM sessions WHERE id = ?1",
                params![session_id],
                |row| {
                    Ok(SessionRecord {
                        id: row.get(0)?,
                        agent: row.get(1)?,
                        adapter: row.get(2)?,
                        started_at: row.get(3)?,
                        ended_at: row.get(4)?,
                        cwd: row.get(5)?,
                        repo: row.get(6)?,
                        branch: row.get(7)?,
                        metadata_json: row.get(8)?,
                    })
                },
            )
            .optional()
            .context("failed to load session")
    }

    pub fn mark_session_ended(&self, session_id: &str, ended_at: &str) -> Result<()> {
        self.connection
            .execute(
                "UPDATE sessions SET ended_at = ?2 WHERE id = ?1",
                params![session_id, ended_at],
            )
            .context("failed to update session end")?;
        Ok(())
    }

    pub fn insert_event(&self, event: &EventRecord) -> Result<()> {
        let alternatives_json =
            serde_json::to_string(&event.alternatives).context("failed to encode alternatives")?;
        self.connection.execute(
            "INSERT INTO events (
               id, session_id, task_id, event_type, skill, agent, adapter, timestamp, duration_ms,
               success, error, retry_count, input_summary, output_summary, planner_reason,
               confidence, alternatives_json, tokens_input, tokens_output, cost_usd, metadata_json
             ) VALUES (
               ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
             )",
            params![
                event.id,
                event.session_id,
                event.task_id,
                event.event_type,
                event.skill,
                event.agent,
                event.adapter,
                event.timestamp,
                event.duration_ms,
                event.success,
                event.error,
                event.retry_count,
                event.input_summary,
                event.output_summary,
                event.planner_reason,
                event.confidence,
                alternatives_json,
                event.tokens_input,
                event.tokens_output,
                event.cost_usd,
                event.metadata_json,
            ],
        ).context("failed to insert event")?;
        Ok(())
    }

    pub fn skill_stats(
        &self,
        since: Option<&str>,
        agent: Option<&str>,
        skill: Option<&str>,
    ) -> Result<Vec<SkillStatsRow>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
               skill,
               COUNT(*) AS uses,
               AVG(CASE
                     WHEN success = 1 THEN 1.0
                     WHEN success = 0 THEN 0.0
                     ELSE NULL
                   END) AS success_rate,
               AVG(duration_ms) AS avg_duration_ms,
               SUM(retry_count) AS retries,
               SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) AS failures
             FROM events
             WHERE skill IS NOT NULL
               AND event_type = 'skill_end'
               AND (?1 IS NULL OR timestamp >= ?1)
               AND (?2 IS NULL OR agent = ?2)
               AND (?3 IS NULL OR skill = ?3)
             GROUP BY skill
             ORDER BY uses DESC, skill ASC",
            )
            .context("failed to prepare stats query")?;

        let rows = statement
            .query_map(params![since, agent, skill], |row| {
                Ok(SkillStatsRow {
                    skill: row.get(0)?,
                    uses: row.get(1)?,
                    success_rate: row.get(2)?,
                    avg_duration_ms: row.get(3)?,
                    retries: row.get(4)?,
                    failures: row.get(5)?,
                })
            })
            .context("failed to run stats query")?;

        rows.collect::<rusqlite::Result<Vec<_>>>().context("failed to decode stats rows")
    }

    pub fn timeline(&self, session_id: Option<&str>, last: bool) -> Result<Vec<TimelineRow>> {
        let resolved_session = if let Some(session_id) = session_id {
            Some(session_id.to_string())
        } else if last {
            self.last_session_id()?
        } else {
            None
        };

        let mut statement = self
            .connection
            .prepare(
                "SELECT timestamp, event_type, skill, success, session_id
             FROM events
             WHERE (?1 IS NULL OR session_id = ?1)
             ORDER BY timestamp ASC",
            )
            .context("failed to prepare timeline query")?;

        let rows = statement
            .query_map(params![resolved_session], |row| {
                Ok(TimelineRow {
                    timestamp: row.get(0)?,
                    event_type: row.get(1)?,
                    skill: row.get(2)?,
                    success: row.get(3)?,
                    session_id: row.get(4)?,
                })
            })
            .context("failed to run timeline query")?;

        rows.collect::<rusqlite::Result<Vec<_>>>().context("failed to decode timeline rows")
    }

    pub fn failures(
        &self,
        since: Option<&str>,
        agent: Option<&str>,
        skill: Option<&str>,
    ) -> Result<Vec<FailureRow>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT timestamp, session_id, skill, error, retry_count, output_summary
             FROM events
             WHERE success = 0
               AND (?1 IS NULL OR timestamp >= ?1)
               AND (?2 IS NULL OR agent = ?2)
               AND (?3 IS NULL OR skill = ?3)
             ORDER BY timestamp DESC",
            )
            .context("failed to prepare failures query")?;

        let rows = statement
            .query_map(params![since, agent, skill], |row| {
                Ok(FailureRow {
                    timestamp: row.get(0)?,
                    session_id: row.get(1)?,
                    skill: row.get(2)?,
                    error: row.get(3)?,
                    retry_count: row.get(4)?,
                    output_summary: row.get(5)?,
                })
            })
            .context("failed to run failures query")?;

        rows.collect::<rusqlite::Result<Vec<_>>>().context("failed to decode failure rows")
    }

    pub fn observed_skills(&self) -> Result<Vec<String>> {
        let mut statement = self
            .connection
            .prepare("SELECT DISTINCT skill FROM events WHERE skill IS NOT NULL ORDER BY skill ASC")
            .context("failed to prepare observed skills query")?;

        let rows = statement
            .query_map([], |row| row.get(0))
            .context("failed to run observed skills query")?;

        rows.collect::<rusqlite::Result<Vec<_>>>().context("failed to decode observed skills")
    }

    pub fn skill_history(&self, agent: Option<&str>) -> Result<Vec<SkillHistoryRow>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT skill, MAX(timestamp) AS last_seen, COUNT(DISTINCT session_id) AS sessions
                 FROM events
                 WHERE skill IS NOT NULL
                   AND event_type = 'skill_end'
                   AND (?1 IS NULL OR agent = ?1)
                 GROUP BY skill
                 ORDER BY skill ASC",
            )
            .context("failed to prepare skill history query")?;

        let rows = statement
            .query_map(params![agent], |row| {
                Ok(SkillHistoryRow {
                    skill: row.get(0)?,
                    last_seen: row.get(1)?,
                    sessions: row.get(2)?,
                })
            })
            .context("failed to run skill history query")?;

        rows.collect::<rusqlite::Result<Vec<_>>>().context("failed to decode skill history rows")
    }

    pub fn skill_overlap(&self, agent: Option<&str>) -> Result<Vec<SkillOverlapRow>> {
        let mut statement = self
            .connection
            .prepare(
                "WITH session_skills AS (
                   SELECT DISTINCT session_id, skill
                   FROM events
                   WHERE skill IS NOT NULL
                     AND event_type = 'skill_end'
                     AND (?1 IS NULL OR agent = ?1)
                 )
                 SELECT a.skill, b.skill, COUNT(*) AS shared_sessions
                 FROM session_skills a
                 JOIN session_skills b
                   ON a.session_id = b.session_id
                  AND a.skill < b.skill
                 GROUP BY a.skill, b.skill
                 ORDER BY shared_sessions DESC, a.skill ASC, b.skill ASC",
            )
            .context("failed to prepare skill overlap query")?;

        let rows = statement
            .query_map(params![agent], |row| {
                Ok(SkillOverlapRow {
                    left_skill: row.get(0)?,
                    right_skill: row.get(1)?,
                    shared_sessions: row.get(2)?,
                })
            })
            .context("failed to run skill overlap query")?;

        rows.collect::<rusqlite::Result<Vec<_>>>().context("failed to decode skill overlap rows")
    }

    pub fn skill_chains(&self, agent: Option<&str>) -> Result<Vec<SkillChainRow>> {
        let mut statement = self
            .connection
            .prepare(
                "WITH ordered_skill_events AS (
                   SELECT
                     session_id,
                     skill AS left_skill,
                     LEAD(skill) OVER (
                       PARTITION BY session_id
                       ORDER BY timestamp ASC, id ASC
                     ) AS right_skill
                   FROM events
                   WHERE skill IS NOT NULL
                     AND event_type = 'skill_end'
                     AND (?1 IS NULL OR agent = ?1)
                 )
                 SELECT left_skill, right_skill, COUNT(*) AS count
                 FROM ordered_skill_events
                 WHERE right_skill IS NOT NULL
                 GROUP BY left_skill, right_skill
                 ORDER BY count DESC, left_skill ASC, right_skill ASC",
            )
            .context("failed to prepare skill chains query")?;

        let rows = statement
            .query_map(params![agent], |row| {
                Ok(SkillChainRow {
                    left_skill: row.get(0)?,
                    right_skill: row.get(1)?,
                    count: row.get(2)?,
                })
            })
            .context("failed to run skill chains query")?;

        rows.collect::<rusqlite::Result<Vec<_>>>().context("failed to decode skill chain rows")
    }

    #[cfg(test)]
    pub fn all_events(&self) -> Result<Vec<EventRecord>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                   id, session_id, task_id, event_type, skill, agent, adapter, timestamp, duration_ms,
                   success, error, retry_count, input_summary, output_summary, planner_reason,
                   confidence, alternatives_json, tokens_input, tokens_output, cost_usd, metadata_json
                 FROM events
                 ORDER BY timestamp ASC, id ASC",
            )
            .context("failed to prepare event export query")?;

        let rows = statement
            .query_map([], |row| {
                let alternatives_json: String = row.get(16)?;
                let alternatives = serde_json::from_str(&alternatives_json).unwrap_or_default();

                Ok(EventRecord {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    task_id: row.get(2)?,
                    event_type: row.get(3)?,
                    skill: row.get(4)?,
                    agent: row.get(5)?,
                    adapter: row.get(6)?,
                    timestamp: row.get(7)?,
                    duration_ms: row.get(8)?,
                    success: row.get(9)?,
                    error: row.get(10)?,
                    retry_count: row.get(11)?,
                    input_summary: row.get(12)?,
                    output_summary: row.get(13)?,
                    planner_reason: row.get(14)?,
                    confidence: row.get(15)?,
                    alternatives,
                    tokens_input: row.get(17)?,
                    tokens_output: row.get(18)?,
                    cost_usd: row.get(19)?,
                    metadata_json: row.get(20)?,
                })
            })
            .context("failed to run event export query")?;

        rows.collect::<rusqlite::Result<Vec<_>>>().context("failed to decode event rows")
    }

    pub fn write_all_events_jsonl<W: Write>(&self, writer: &mut W) -> Result<()> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                   id, session_id, task_id, event_type, skill, agent, adapter, timestamp, duration_ms,
                   success, error, retry_count, input_summary, output_summary, planner_reason,
                   confidence, alternatives_json, tokens_input, tokens_output, cost_usd, metadata_json
                 FROM events
                 ORDER BY timestamp ASC, id ASC",
            )
            .context("failed to prepare event export query")?;

        let rows = statement
            .query_map([], |row| {
                let alternatives_json: String = row.get(16)?;
                let alternatives = serde_json::from_str(&alternatives_json).unwrap_or_default();

                Ok(EventRecord {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    task_id: row.get(2)?,
                    event_type: row.get(3)?,
                    skill: row.get(4)?,
                    agent: row.get(5)?,
                    adapter: row.get(6)?,
                    timestamp: row.get(7)?,
                    duration_ms: row.get(8)?,
                    success: row.get(9)?,
                    error: row.get(10)?,
                    retry_count: row.get(11)?,
                    input_summary: row.get(12)?,
                    output_summary: row.get(13)?,
                    planner_reason: row.get(14)?,
                    confidence: row.get(15)?,
                    alternatives,
                    tokens_input: row.get(17)?,
                    tokens_output: row.get(18)?,
                    cost_usd: row.get(19)?,
                    metadata_json: row.get(20)?,
                })
            })
            .context("failed to run event export query")?;

        for row in rows {
            let event = row.context("failed to decode event row")?;
            writeln!(writer, "{}", serde_json::to_string(&event)?)
                .context("failed to stream JSONL event")?;
        }

        Ok(())
    }

    pub fn event_by_id(&self, event_id: &str) -> Result<Option<EventRecord>> {
        self.connection
            .query_row(
                "SELECT
                   id, session_id, task_id, event_type, skill, agent, adapter, timestamp, duration_ms,
                   success, error, retry_count, input_summary, output_summary, planner_reason,
                   confidence, alternatives_json, tokens_input, tokens_output, cost_usd, metadata_json
                 FROM events
                 WHERE id = ?1",
                params![event_id],
                |row| {
                    let alternatives_json: String = row.get(16)?;
                    let alternatives = serde_json::from_str(&alternatives_json).unwrap_or_default();

                    Ok(EventRecord {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        task_id: row.get(2)?,
                        event_type: row.get(3)?,
                        skill: row.get(4)?,
                        agent: row.get(5)?,
                        adapter: row.get(6)?,
                        timestamp: row.get(7)?,
                        duration_ms: row.get(8)?,
                        success: row.get(9)?,
                        error: row.get(10)?,
                        retry_count: row.get(11)?,
                        input_summary: row.get(12)?,
                        output_summary: row.get(13)?,
                        planner_reason: row.get(14)?,
                        confidence: row.get(15)?,
                        alternatives,
                        tokens_input: row.get(17)?,
                        tokens_output: row.get(18)?,
                        cost_usd: row.get(19)?,
                        metadata_json: row.get(20)?,
                    })
                },
            )
            .optional()
            .context("failed to load event by id")
    }

    fn last_session_id(&self) -> Result<Option<String>> {
        self.connection
            .query_row("SELECT id FROM sessions ORDER BY started_at DESC LIMIT 1", [], |row| {
                row.get(0)
            })
            .optional()
            .context("failed to fetch last session id")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillStatsRow {
    pub skill: String,
    pub uses: i64,
    pub success_rate: Option<f64>,
    pub avg_duration_ms: Option<f64>,
    pub retries: i64,
    pub failures: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineRow {
    pub timestamp: String,
    pub event_type: String,
    pub skill: Option<String>,
    pub success: Option<bool>,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailureRow {
    pub timestamp: String,
    pub session_id: String,
    pub skill: Option<String>,
    pub error: Option<String>,
    pub retry_count: i64,
    pub output_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillHistoryRow {
    pub skill: String,
    pub last_seen: String,
    pub sessions: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillOverlapRow {
    pub left_skill: String,
    pub right_skill: String,
    pub shared_sessions: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillChainRow {
    pub left_skill: String,
    pub right_skill: String,
    pub count: i64,
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::Database;
    use crate::event::EventRecord;

    #[test]
    fn skill_stats_leave_missing_success_as_unknown() {
        let temp = tempdir().expect("tempdir");
        let database = Database::open(&temp.path().join("skilltrace.db"))
            .expect("open db")
            .initialize()
            .expect("init db");

        database
            .insert_event(&EventRecord::new(
                "skill_end".to_string(),
                "session-1".to_string(),
                None,
                Some("run_tests".to_string()),
                "codex".to_string(),
                "manual".to_string(),
                None,
                Some(250),
                None,
                0,
                None,
                None,
                None,
                None,
                Vec::new(),
                None,
                None,
                None,
            ))
            .expect("insert event");

        let rows = database.skill_stats(None, None, None).expect("stats");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].skill, "run_tests");
        assert_eq!(rows[0].uses, 1);
        assert_eq!(rows[0].success_rate, None);
        assert_eq!(rows[0].failures, 0);
    }

    #[test]
    fn skill_chains_count_adjacent_skill_end_pairs_across_sessions() {
        let temp = tempdir().expect("tempdir");
        let database = Database::open(&temp.path().join("skilltrace.db"))
            .expect("open db")
            .initialize()
            .expect("init db");

        for (id, session_id, timestamp, skill) in [
            ("event-1", "session-1", "2026-06-28T00:00:00Z", "repo_search"),
            ("event-2", "session-1", "2026-06-28T00:00:01Z", "edit_file"),
            ("event-3", "session-1", "2026-06-28T00:00:02Z", "run_tests"),
            ("event-4", "session-2", "2026-06-28T00:00:00Z", "repo_search"),
            ("event-5", "session-2", "2026-06-28T00:00:01Z", "edit_file"),
        ] {
            let mut event = EventRecord::new(
                "skill_end".to_string(),
                session_id.to_string(),
                None,
                Some(skill.to_string()),
                "codex".to_string(),
                "manual".to_string(),
                Some(true),
                None,
                None,
                0,
                None,
                None,
                None,
                None,
                Vec::new(),
                None,
                None,
                None,
            );
            event.id = id.to_string();
            event.timestamp = timestamp.to_string();
            database.insert_event(&event).expect("insert event");
        }

        let rows = database.skill_chains(None).expect("skill chains");

        assert!(rows.iter().any(|row| {
            row.left_skill == "repo_search" && row.right_skill == "edit_file" && row.count == 2
        }));
    }
}
