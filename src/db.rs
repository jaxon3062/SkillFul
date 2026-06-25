use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

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
}
