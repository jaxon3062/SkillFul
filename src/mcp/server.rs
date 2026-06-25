use std::{
    io::{self, BufRead, Write},
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    config::{AppConfig, StoragePaths},
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

        let request: McpRequest =
            serde_json::from_str(&line).context("failed to parse MCP request")?;
        let response = handle_request(request)?;
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
            let params = request.params.ok_or_else(|| anyhow!("missing tools/call params"))?;
            let call: ToolCallParams =
                serde_json::from_value(params).context("failed to decode tool call params")?;
            handle_tool_call(call)?
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

fn handle_tool_call(call: ToolCallParams) -> Result<Value> {
    let paths = StoragePaths::discover()?;
    paths.ensure_dirs()?;
    let config = AppConfig::load_or_create(&paths)?;
    let database = Database::open(&paths.database_path())?.initialize()?;

    match call.name.as_str() {
        "skilltrace.record_event" => {
            let args: RecordEventArgs = serde_json::from_value(call.arguments)?;
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
            persist_event(&paths, &database, &event)?;
            Ok(json!({ "event_id": event.id, "status": "recorded" }))
        }
        "skilltrace.record_skill_start" => {
            let args: RecordSkillStartArgs = serde_json::from_value(call.arguments)?;
            let event = EventRecord::new(
                "skill_start".to_string(),
                args.session_id.unwrap_or_else(new_session_id),
                args.task_id,
                Some(args.skill),
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
            persist_event(&paths, &database, &event)?;
            Ok(json!({ "event_id": event.id, "status": "recorded" }))
        }
        "skilltrace.record_skill_end" => {
            let args: RecordSkillEndArgs = serde_json::from_value(call.arguments)?;
            let timestamp = Utc::now().to_rfc3339();
            let updated = database.update_event_from_skill_end(
                &args.event_id,
                &timestamp,
                args.success,
                args.output_summary.as_deref(),
                args.error.as_deref(),
            )?;
            Ok(json!({
                "status": "recorded",
                "event_id": updated.id,
                "duration_ms": updated.duration_ms
            }))
        }
        "skilltrace.record_decision" => {
            let args: RecordDecisionArgs = serde_json::from_value(call.arguments)?;
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
            persist_event(&paths, &database, &event)?;
            Ok(json!({ "event_id": event.id, "status": "recorded" }))
        }
        "skilltrace.get_stats" => {
            let args: QueryFilterArgs = serde_json::from_value(call.arguments)?;
            let query = stats::StatsQuery::from_filter_args(args.since, args.agent, args.skill);
            let rows = database.skill_stats(
                query.since_timestamp()?.as_deref(),
                query.agent.as_deref(),
                query.skill.as_deref(),
            )?;
            Ok(json!({ "skills": rows }))
        }
        "skilltrace.get_failures" => {
            let args: QueryFilterArgs = serde_json::from_value(call.arguments)?;
            let query = stats::StatsQuery::from_filter_args(args.since, args.agent, args.skill);
            let rows = database.failures(
                query.since_timestamp()?.as_deref(),
                query.agent.as_deref(),
                query.skill.as_deref(),
            )?;
            Ok(json!({ "failures": rows }))
        }
        "skilltrace.get_recommendations" => {
            let args: RecommendationArgs = serde_json::from_value(call.arguments)?;
            let cwd = args.cwd.as_deref().map(Path::new).unwrap_or_else(|| Path::new("."));
            let definition_file = config.resolved_definition_file(cwd);
            let observed = database.observed_skills()?;
            let skill_stats = database.skill_stats(None, args.agent.as_deref(), None)?;
            let recommendations =
                recommend::build_recommendations(&skill_stats, &definition_file, &observed)?;
            Ok(json!({ "recommendations": recommendations }))
        }
        name => Err(anyhow!("unknown tool: {name}")),
    }
}

fn persist_event(paths: &StoragePaths, database: &Database, event: &EventRecord) -> Result<()> {
    database.insert_event(event)?;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(paths.jsonl_path())
        .with_context(|| format!("failed to open {}", paths.jsonl_path().display()))?;
    writeln!(file, "{}", serde_json::to_string(event)?).context("failed to append JSONL event")
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

    use super::{McpRequest, handle_request};

    fn with_temp_home<T>(test: T)
    where
        T: FnOnce(),
    {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().expect("lock env");
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
        });
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
}
