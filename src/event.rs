use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: String,
    pub event_type: String,
    pub session_id: String,
    pub task_id: Option<String>,
    pub skill: Option<String>,
    pub agent: String,
    pub adapter: String,
    pub timestamp: String,
    pub duration_ms: Option<i64>,
    pub success: Option<bool>,
    pub error: Option<String>,
    pub retry_count: i64,
    pub input_summary: Option<String>,
    pub output_summary: Option<String>,
    pub planner_reason: Option<String>,
    pub confidence: Option<f64>,
    pub alternatives: Vec<String>,
    pub tokens_input: Option<i64>,
    pub tokens_output: Option<i64>,
    pub cost_usd: Option<f64>,
    pub metadata_json: Option<String>,
}

impl EventRecord {
    pub fn new(
        event_type: String,
        session_id: String,
        task_id: Option<String>,
        skill: Option<String>,
        agent: String,
        adapter: String,
        success: Option<bool>,
        duration_ms: Option<i64>,
        error: Option<String>,
        retry_count: i64,
        input_summary: Option<String>,
        output_summary: Option<String>,
        planner_reason: Option<String>,
        confidence: Option<f64>,
        alternatives: Vec<String>,
        tokens_input: Option<i64>,
        tokens_output: Option<i64>,
        cost_usd: Option<f64>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_type,
            session_id,
            task_id,
            skill,
            agent,
            adapter,
            timestamp: Utc::now().to_rfc3339(),
            duration_ms,
            success,
            error,
            retry_count,
            input_summary,
            output_summary,
            planner_reason,
            confidence,
            alternatives,
            tokens_input,
            tokens_output,
            cost_usd,
            metadata_json: None,
        }
    }
}
