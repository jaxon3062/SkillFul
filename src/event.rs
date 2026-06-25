use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub event_type: String,
    pub session_id: String,
    pub task_id: Option<String>,
    pub skill: Option<String>,
    pub agent: String,
    pub adapter: String,
    pub timestamp: String,
    pub duration_ms: Option<i64>,
    pub success: Option<bool>,
    pub input_summary: Option<String>,
    pub output_summary: Option<String>,
}

impl EventRecord {
    pub fn from_cli(
        event_type: String,
        skill: Option<String>,
        success: Option<bool>,
        duration_ms: Option<i64>,
        input_summary: Option<String>,
        output_summary: Option<String>,
    ) -> Self {
        Self {
            event_type,
            session_id: Uuid::new_v4().to_string(),
            task_id: None,
            skill,
            agent: "codex".to_string(),
            adapter: "manual".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            duration_ms,
            success,
            input_summary,
            output_summary,
        }
    }
}
