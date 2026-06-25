use std::{
    io::{self, BufRead, Write},
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    config::{AppConfig, RuntimeState, StoragePaths},
    db::Database,
    event::EventRecord,
    recommend, stats,
};

use super::tools;

pub fn run_stdio() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line.context("failed to read stdin")?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match parse_request(&line) {
            Ok(request) => match handle_request(request) {
                Ok(response) => response,
                Err(error) => McpResponse::error(json!(null), -32603, error.to_string()),
            },
            Err(response) => response,
        };
        serde_json::to_writer(&mut writer, &response)
            .context("failed to serialize MCP response")?;
        writer.write_all(b"\n").context("failed to write MCP response")?;
        writer.flush().context("failed to flush MCP response")?;
    }

    Ok(())
}

pub fn handle_request(request: McpRequest) -> Result<McpResponse> {
    if request.jsonrpc != "2.0" {
        return Ok(McpResponse::error(
            request.id,
            -32600,
            format!("unsupported jsonrpc version: {}", request.jsonrpc),
        ));
    }

    let result = match request.method.as_str() {
        "initialize" => initialize_result(),
        "tools/list" => tools::list_result(),
        "tools/call" => {
            let params = match request.params {
                Some(params) => params,
                None => {
                    return Ok(McpResponse::error(
                        request.id,
                        -32602,
                        "missing tools/call params".to_string(),
                    ));
                }
            };
            let call: ToolCallParams = match serde_json::from_value(params) {
                Ok(call) => call,
                Err(error) => {
                    return Ok(McpResponse::error(
                        request.id,
                        -32602,
                        format!("failed to decode tool call params: {error}"),
                    ));
                }
            };
            match handle_tool_call(call) {
                Ok(result) => result,
                Err(ToolCallError::UnknownTool(name)) => {
                    return Ok(McpResponse::error(
                        request.id,
                        -32601,
                        format!("unknown tool: {name}"),
                    ));
                }
                Err(ToolCallError::InvalidParams(message)) => {
                    return Ok(McpResponse::error(request.id, -32602, message));
                }
                Err(ToolCallError::Internal(error)) => return Err(error),
            }
        }
        method => {
            return Ok(McpResponse::error(request.id, -32601, format!("unknown method: {method}")));
        }
    };

    Ok(McpResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id,
        result: Some(result),
        error: None,
    })
}

fn handle_tool_call(call: ToolCallParams) -> std::result::Result<Value, ToolCallError> {
    let paths = StoragePaths::discover().map_err(ToolCallError::Internal)?;
    paths.ensure_dirs().map_err(ToolCallError::Internal)?;
    let config = AppConfig::load_or_create(&paths).map_err(ToolCallError::Internal)?;
    let database = Database::open(&paths.database_path())
        .and_then(Database::initialize)
        .map_err(ToolCallError::Internal)?;
    let state = RuntimeState::load(&paths).map_err(ToolCallError::Internal)?;

    match call.name.as_str() {
        "skilltrace.record_event" => {
            let args: RecordEventArgs = serde_json::from_value(call.arguments)
                .map_err(|error| ToolCallError::InvalidParams(error.to_string()))?;
            ensure_session(
                &database,
                &args.session_id,
                args.agent.as_deref().unwrap_or("codex"),
                args.adapter.as_deref().unwrap_or("mcp"),
            )
            .map_err(ToolCallError::Internal)?;
            let event = EventRecord::new(
                args.event_type,
                args.session_id,
                args.task_id,
                args.skill,
                args.agent.unwrap_or_else(|| "codex".to_string()),
                args.adapter.unwrap_or_else(|| "mcp".to_string()),
                args.success,
                args.duration_ms,
                args.error,
                args.retry_count.unwrap_or(0),
                args.input_summary,
                args.output_summary,
                args.planner_reason,
                args.confidence,
                args.alternatives.unwrap_or_default(),
                args.tokens_input,
                args.tokens_output,
                args.cost_usd,
            );
            persist_event(&paths, &database, &event).map_err(ToolCallError::Internal)?;
            Ok(json!({ "event_id": event.id, "status": "recorded" }))
        }
        "skilltrace.record_skill_start" => {
            let args: RecordSkillStartArgs = serde_json::from_value(call.arguments)
                .map_err(|error| ToolCallError::InvalidParams(error.to_string()))?;
            let session_id = args
                .session_id
                .or_else(|| state.preferred_session_id())
                .unwrap_or_else(new_session_id);
            let agent = args.agent.unwrap_or_else(|| "codex".to_string());
            let adapter = args.adapter.unwrap_or_else(|| "mcp".to_string());
            ensure_session(&database, &session_id, &agent, &adapter)
                .map_err(ToolCallError::Internal)?;
            let event = EventRecord::new(
                "skill_start".to_string(),
                session_id,
                args.task_id,
                Some(args.skill),
                agent,
                adapter,
                None,
                None,
                None,
                0,
                None,
                None,
                args.planner_reason,
                args.confidence,
                args.alternatives.unwrap_or_default(),
                None,
                None,
                None,
            );
            persist_event(&paths, &database, &event).map_err(ToolCallError::Internal)?;
            Ok(json!({ "event_id": event.id, "status": "recorded" }))
        }
        "skilltrace.record_skill_end" => {
            let args: RecordSkillEndArgs = serde_json::from_value(call.arguments)
                .map_err(|error| ToolCallError::InvalidParams(error.to_string()))?;
            let completed =
                complete_skill_event(&database, &args).map_err(ToolCallError::Internal)?;
            persist_event(&paths, &database, &completed).map_err(ToolCallError::Internal)?;
            Ok(json!({
                "status": "recorded",
                "event_id": completed.id,
                "duration_ms": completed.duration_ms
            }))
        }
        "skilltrace.record_decision" => {
            let args: RecordDecisionArgs = serde_json::from_value(call.arguments)
                .map_err(|error| ToolCallError::InvalidParams(error.to_string()))?;
            ensure_session(
                &database,
                &args.session_id,
                args.agent.as_deref().unwrap_or("codex"),
                args.adapter.as_deref().unwrap_or("mcp"),
            )
            .map_err(ToolCallError::Internal)?;
            let event = EventRecord::new(
                "decision".to_string(),
                args.session_id,
                args.task_id,
                args.skill,
                args.agent.unwrap_or_else(|| "codex".to_string()),
                args.adapter.unwrap_or_else(|| "mcp".to_string()),
                None,
                None,
                None,
                0,
                None,
                None,
                args.planner_reason,
                args.confidence,
                args.alternatives.unwrap_or_default(),
                None,
                None,
                None,
            );
            persist_event(&paths, &database, &event).map_err(ToolCallError::Internal)?;
            Ok(json!({ "event_id": event.id, "status": "recorded" }))
        }
        "skilltrace.get_stats" => {
            let args: QueryFilterArgs = serde_json::from_value(call.arguments)
                .map_err(|error| ToolCallError::InvalidParams(error.to_string()))?;
            let query = stats::StatsQuery::from_filter_args(args.since, args.agent, args.skill);
            let rows = database
                .skill_stats(
                    query.since_timestamp().map_err(ToolCallError::Internal)?.as_deref(),
                    query.agent.as_deref(),
                    query.skill.as_deref(),
                )
                .map_err(ToolCallError::Internal)?;
            Ok(json!({ "skills": rows }))
        }
        "skilltrace.get_failures" => {
            let args: QueryFilterArgs = serde_json::from_value(call.arguments)
                .map_err(|error| ToolCallError::InvalidParams(error.to_string()))?;
            let query = stats::StatsQuery::from_filter_args(args.since, args.agent, args.skill);
            let rows = database
                .failures(
                    query.since_timestamp().map_err(ToolCallError::Internal)?.as_deref(),
                    query.agent.as_deref(),
                    query.skill.as_deref(),
                )
                .map_err(ToolCallError::Internal)?;
            Ok(json!({ "failures": rows }))
        }
        "skilltrace.get_recommendations" => {
            let args: RecommendationArgs = serde_json::from_value(call.arguments)
                .map_err(|error| ToolCallError::InvalidParams(error.to_string()))?;
            let cwd = args.cwd.as_deref().map(Path::new).unwrap_or_else(|| Path::new("."));
            let definition_file = config.resolved_definition_file(cwd);
            let observed = database.observed_skills().map_err(ToolCallError::Internal)?;
            let skill_stats = database
                .skill_stats(None, args.agent.as_deref(), None)
                .map_err(ToolCallError::Internal)?;
            let recommendations =
                recommend::build_recommendations(&skill_stats, &definition_file, &observed)
                    .map_err(ToolCallError::Internal)?;
            Ok(json!({ "recommendations": recommendations }))
        }
        name => Err(ToolCallError::UnknownTool(name.to_string())),
    }
}

fn persist_event(paths: &StoragePaths, database: &Database, event: &EventRecord) -> Result<()> {
    database.insert_event(event)?;
    append_jsonl_snapshot(paths, event)
}

fn append_jsonl_snapshot(paths: &StoragePaths, event: &EventRecord) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(paths.jsonl_path())
        .with_context(|| format!("failed to open {}", paths.jsonl_path().display()))?;
    writeln!(file, "{}", serde_json::to_string(event)?).context("failed to append JSONL event")
}

fn ensure_session(database: &Database, session_id: &str, agent: &str, adapter: &str) -> Result<()> {
    if database.get_session(session_id)?.is_none() {
        let mut session =
            crate::config::SessionRecord::new(agent.to_string(), adapter.to_string(), None);
        session.id = session_id.to_string();
        database.upsert_session(&session)?;
    }

    Ok(())
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "serverInfo": {
            "name": "skilltrace",
            "version": env!("CARGO_PKG_VERSION")
        },
        "capabilities": {
            "tools": {}
        }
    })
}

fn new_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn parse_request(line: &str) -> std::result::Result<McpRequest, McpResponse> {
    let value: Value = serde_json::from_str(line).map_err(|error| {
        McpResponse::error(json!(null), -32700, format!("parse error: {error}"))
    })?;
    let id = value.get("id").cloned().unwrap_or_else(|| json!(null));
    serde_json::from_value(value)
        .map_err(|error| McpResponse::error(id, -32600, format!("invalid request: {error}")))
}

fn complete_skill_event(database: &Database, args: &RecordSkillEndArgs) -> Result<EventRecord> {
    let start_event = database
        .event_by_id(&args.event_id)?
        .ok_or_else(|| anyhow!("event {} not found", args.event_id))?;
    let ended_at = Utc::now().to_rfc3339();
    let duration_ms =
        chrono::DateTime::parse_from_rfc3339(&start_event.timestamp).ok().and_then(|started_at| {
            chrono::DateTime::parse_from_rfc3339(&ended_at)
                .ok()
                .map(|ended| (ended - started_at).num_milliseconds())
        });

    let mut event = EventRecord::new(
        "skill_end".to_string(),
        start_event.session_id.clone(),
        start_event.task_id.clone(),
        start_event.skill.clone(),
        start_event.agent.clone(),
        start_event.adapter.clone(),
        Some(args.success),
        duration_ms,
        args.error.clone(),
        start_event.retry_count,
        start_event.input_summary.clone(),
        args.output_summary.clone().or(start_event.output_summary.clone()),
        start_event.planner_reason.clone(),
        start_event.confidence,
        start_event.alternatives.clone(),
        start_event.tokens_input,
        start_event.tokens_output,
        start_event.cost_usd,
    );
    event.timestamp = ended_at;
    Ok(event)
}

enum ToolCallError {
    UnknownTool(String),
    InvalidParams(String),
    Internal(anyhow::Error),
}

#[derive(Debug, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

impl McpResponse {
    fn error(id: Value, code: i64, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(McpError { code, message }),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct McpError {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
struct RecordEventArgs {
    event_type: String,
    session_id: String,
    task_id: Option<String>,
    skill: Option<String>,
    agent: Option<String>,
    adapter: Option<String>,
    success: Option<bool>,
    duration_ms: Option<i64>,
    error: Option<String>,
    retry_count: Option<i64>,
    input_summary: Option<String>,
    output_summary: Option<String>,
    planner_reason: Option<String>,
    confidence: Option<f64>,
    alternatives: Option<Vec<String>>,
    tokens_input: Option<i64>,
    tokens_output: Option<i64>,
    cost_usd: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RecordSkillStartArgs {
    skill: String,
    session_id: Option<String>,
    task_id: Option<String>,
    agent: Option<String>,
    adapter: Option<String>,
    planner_reason: Option<String>,
    confidence: Option<f64>,
    alternatives: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RecordSkillEndArgs {
    event_id: String,
    success: bool,
    output_summary: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RecordDecisionArgs {
    session_id: String,
    task_id: Option<String>,
    skill: Option<String>,
    agent: Option<String>,
    adapter: Option<String>,
    planner_reason: Option<String>,
    confidence: Option<f64>,
    alternatives: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct QueryFilterArgs {
    since: Option<String>,
    agent: Option<String>,
    skill: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RecommendationArgs {
    agent: Option<String>,
    cwd: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::Path,
        sync::{Mutex, OnceLock},
    };

    use serde_json::json;
    use tempfile::tempdir;

    use crate::{
        config::{RuntimeState, StoragePaths},
        db::Database,
    };

    use super::{McpRequest, handle_request};

    fn with_temp_home<T>(test: T)
    where
        T: FnOnce(),
    {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let home = tempdir().expect("tempdir");
        let previous_home = env::var_os("HOME");
        // Safety: test code controls process environment in a scoped single-threaded context.
        unsafe {
            env::set_var("HOME", home.path());
        }
        test();
        match previous_home {
            Some(value) => unsafe {
                env::set_var("HOME", value);
            },
            None => unsafe {
                env::remove_var("HOME");
            },
        }
    }

    #[test]
    fn initialize_returns_server_metadata() {
        let response = handle_request(McpRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "initialize".to_string(),
            params: None,
        })
        .expect("initialize response");

        assert_eq!(response.result.expect("result")["serverInfo"]["name"], "skilltrace");
    }

    #[test]
    fn tools_call_records_and_reports_stats() {
        with_temp_home(|| {
            let response = handle_request(McpRequest {
                jsonrpc: "2.0".to_string(),
                id: json!(1),
                method: "tools/call".to_string(),
                params: Some(json!({
                    "name": "skilltrace.record_event",
                    "arguments": {
                        "event_type": "skill_end",
                        "session_id": "session-1",
                        "skill": "run_tests",
                        "success": true,
                        "duration_ms": 500
                    }
                })),
            })
            .expect("record_event");
            assert_eq!(response.result.expect("result")["status"], "recorded");

            let stats = handle_request(McpRequest {
                jsonrpc: "2.0".to_string(),
                id: json!(2),
                method: "tools/call".to_string(),
                params: Some(json!({
                    "name": "skilltrace.get_stats",
                    "arguments": {}
                })),
            })
            .expect("get_stats");

            assert_eq!(stats.result.expect("result")["skills"][0]["skill"], "run_tests");
        });
    }

    #[test]
    fn skill_start_and_end_round_trip() {
        with_temp_home(|| {
            let start = handle_request(McpRequest {
                jsonrpc: "2.0".to_string(),
                id: json!(1),
                method: "tools/call".to_string(),
                params: Some(json!({
                    "name": "skilltrace.record_skill_start",
                    "arguments": {
                        "session_id": "session-2",
                        "skill": "repo_search"
                    }
                })),
            })
            .expect("record_skill_start");

            let event_id =
                start.result.expect("result")["event_id"].as_str().expect("event id").to_string();

            let end = handle_request(McpRequest {
                jsonrpc: "2.0".to_string(),
                id: json!(2),
                method: "tools/call".to_string(),
                params: Some(json!({
                    "name": "skilltrace.record_skill_end",
                    "arguments": {
                        "event_id": event_id,
                        "success": true,
                        "output_summary": "found files"
                    }
                })),
            })
            .expect("record_skill_end");

            assert_eq!(end.result.expect("result")["status"], "recorded");

            let paths = StoragePaths::discover().expect("paths");
            let database = Database::open(&paths.database_path())
                .expect("open db")
                .initialize()
                .expect("init db");
            assert!(database.get_session("session-2").expect("get session").is_some());
            let events = database.all_events().expect("all events");
            assert_eq!(events.len(), 2);
            assert_eq!(events[0].event_type, "skill_start");
            assert_eq!(events[1].event_type, "skill_end");

            let jsonl = fs::read_to_string(paths.jsonl_path()).expect("read jsonl");
            assert_eq!(jsonl.lines().count(), 2);
            assert!(jsonl.contains("\"event_type\":\"skill_start\""));
            assert!(jsonl.contains("\"event_type\":\"skill_end\""));
        });
    }

    #[test]
    fn skill_start_without_session_id_uses_single_active_runtime_session() {
        with_temp_home(|| {
            let paths = StoragePaths::discover().expect("paths");
            paths.ensure_dirs().expect("ensure dirs");
            RuntimeState {
                current_session_id: Some("session-active".to_string()),
                active_session_ids: vec!["session-active".to_string()],
            }
            .save(&paths)
            .expect("save state");

            let start = handle_request(McpRequest {
                jsonrpc: "2.0".to_string(),
                id: json!(1),
                method: "tools/call".to_string(),
                params: Some(json!({
                    "name": "skilltrace.record_skill_start",
                    "arguments": {
                        "skill": "repo_search"
                    }
                })),
            })
            .expect("record_skill_start");

            let event_id =
                start.result.expect("result")["event_id"].as_str().expect("event id").to_string();
            let database = Database::open(&paths.database_path())
                .expect("open db")
                .initialize()
                .expect("init db");
            let event = database.event_by_id(&event_id).expect("event").expect("existing");
            assert_eq!(event.session_id, "session-active");
        });
    }

    #[test]
    fn invalid_tool_returns_jsonrpc_error_response() {
        let response = handle_request(McpRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(7),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "skilltrace.not_real",
                "arguments": {}
            })),
        })
        .expect("response");

        assert_eq!(response.error.expect("error").code, -32601);
    }

    #[test]
    fn recommendations_use_configured_definition_file() {
        with_temp_home(|| {
            let home = env::var("HOME").expect("home");
            let skilltrace_root = Path::new(&home).join(".skilltrace");
            fs::create_dir_all(skilltrace_root.join("catalog")).expect("mkdir");
            fs::create_dir_all(Path::new(&home).join("catalog")).expect("mkdir cwd catalog");
            fs::write(
                skilltrace_root.join("config.toml"),
                "[storage]\nbackend = \"sqlite\"\npath = \"~/.skilltrace/skilltrace.db\"\njsonl_mirror = true\n\n[agents.codex]\nenabled = true\nadapter = \"wrapper\"\n\n[mcp]\nenabled = true\ntransport = \"stdio\"\n\n[privacy]\ncapture_raw_prompts = false\ncapture_raw_outputs = false\nhash_sensitive_values = true\n\n[skills]\ndefinition_file = \"catalog/skills.toml\"\n",
            )
            .expect("config");
            fs::write(
                Path::new(&home).join("catalog/skills.toml"),
                "[[skill]]\nname = \"debug_error\"\ndescription = \"Debug\"\ncategory = \"debugging\"\n",
            )
            .expect("skills");

            let response = handle_request(McpRequest {
                jsonrpc: "2.0".to_string(),
                id: json!(1),
                method: "tools/call".to_string(),
                params: Some(json!({
                    "name": "skilltrace.get_recommendations",
                    "arguments": {
                        "cwd": home
                    }
                })),
            })
            .expect("recommendations");

            let result = response.result.expect("result");
            let recommendations = result["recommendations"].as_array().expect("array");
            assert!(!recommendations.is_empty());
        });
    }

    #[test]
    fn record_event_creates_missing_session() {
        with_temp_home(|| {
            handle_request(McpRequest {
                jsonrpc: "2.0".to_string(),
                id: json!(1),
                method: "tools/call".to_string(),
                params: Some(json!({
                    "name": "skilltrace.record_event",
                    "arguments": {
                        "event_type": "decision",
                        "session_id": "session-3",
                        "agent": "codex",
                        "adapter": "mcp"
                    }
                })),
            })
            .expect("record_event");

            let paths = StoragePaths::discover().expect("paths");
            let database = Database::open(&paths.database_path())
                .expect("open db")
                .initialize()
                .expect("init db");
            assert!(database.get_session("session-3").expect("get session").is_some());
        });
    }
}
