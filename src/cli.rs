use std::{env, fs::OpenOptions, io::Write, path::PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{
    config::{AppConfig, RuntimeState, SessionRecord, StoragePaths},
    db::Database,
    event::EventRecord,
    export, mcp, recommend, stats,
};

#[derive(Debug, Parser)]
#[command(name = "skilltrace", version, about = "Trace coding-agent skill usage locally")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Commands::Init => init_command(),
            Commands::Wrap(args) => wrap_command(args),
            Commands::Event(args) => event_command(args),
            Commands::Stats(args) => stats_command(args),
            Commands::Timeline(args) => timeline_command(args),
            Commands::Failures(args) => failures_command(args),
            Commands::Unused(args) => unused_command(args),
            Commands::Recommend => recommend_command(),
            Commands::Export(args) => export_command(args),
            Commands::Mcp => mcp_command(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init,
    Wrap(WrapArgs),
    Event(EventArgs),
    Stats(FilterArgs),
    Timeline(TimelineArgs),
    Failures(FilterArgs),
    Unused(UnusedArgs),
    Recommend,
    Export(ExportArgs),
    Mcp,
}

#[derive(Debug, Args)]
struct WrapArgs {
    #[arg(required = true, trailing_var_arg = true)]
    command: Vec<String>,
}

#[derive(Debug, Clone, ValueEnum)]
enum EventKind {
    SkillStart,
    SkillEnd,
    Decision,
    SessionStart,
    SessionEnd,
    Error,
}

impl EventKind {
    fn as_event_type(&self) -> &'static str {
        match self {
            Self::SkillStart => "skill_start",
            Self::SkillEnd => "skill_end",
            Self::Decision => "decision",
            Self::SessionStart => "session_start",
            Self::SessionEnd => "session_end",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Args)]
struct EventArgs {
    kind: EventKind,
    #[arg(long)]
    session_id: Option<String>,
    #[arg(long)]
    task_id: Option<String>,
    #[arg(long)]
    skill: Option<String>,
    #[arg(long, default_value = "codex")]
    agent: String,
    #[arg(long, default_value = "manual")]
    adapter: String,
    #[arg(long)]
    success: Option<bool>,
    #[arg(long)]
    error: Option<String>,
    #[arg(long, default_value_t = 0)]
    retry_count: i64,
    #[arg(long)]
    duration_ms: Option<i64>,
    #[arg(long)]
    input_summary: Option<String>,
    #[arg(long)]
    output_summary: Option<String>,
    #[arg(long)]
    planner_reason: Option<String>,
    #[arg(long)]
    confidence: Option<f64>,
    #[arg(long, value_delimiter = ',')]
    alternatives: Vec<String>,
    #[arg(long)]
    tokens_input: Option<i64>,
    #[arg(long)]
    tokens_output: Option<i64>,
    #[arg(long)]
    cost_usd: Option<f64>,
}

#[derive(Debug, Args)]
struct FilterArgs {
    #[arg(long)]
    since: Option<String>,
    #[arg(long)]
    agent: Option<String>,
    #[arg(long)]
    skill: Option<String>,
}

#[derive(Debug, Args)]
struct TimelineArgs {
    #[arg(long)]
    last: bool,
    #[arg(long)]
    session: Option<String>,
}

#[derive(Debug, Args)]
struct UnusedArgs {
    #[arg(long, default_value = "skills.toml")]
    defined_skills: PathBuf,
}

#[derive(Debug, Clone, ValueEnum)]
enum ExportKind {
    Jsonl,
    Otel,
}

#[derive(Debug, Args)]
struct ExportArgs {
    kind: ExportKind,
}

fn init_command() -> Result<()> {
    let paths = StoragePaths::discover()?;
    paths.ensure_dirs()?;
    AppConfig::write_default_if_missing(&paths)?;
    Database::open(&paths.database_path())?.initialize()?;
    RuntimeState { current_session_id: None }.save(&paths)?;

    println!("Initialized skilltrace at {}", paths.root.display());
    Ok(())
}

fn wrap_command(args: WrapArgs) -> Result<()> {
    let command_line = args.command.join(" ");
    let adapters = crate::adapters::supported_adapters().join(", ");
    println!("Wrapper scaffold ready for: {command_line}\nSupported adapters: {adapters}");
    Ok(())
}

fn event_command(args: EventArgs) -> Result<()> {
    let paths = StoragePaths::discover()?;
    paths.ensure_dirs()?;
    AppConfig::write_default_if_missing(&paths)?;
    let database = Database::open(&paths.database_path())?.initialize()?;
    let mut state = RuntimeState::load(&paths)?;

    let session_id = match (&args.kind, args.session_id.clone(), state.current_session_id.clone()) {
        (EventKind::SessionStart, Some(session_id), _) => session_id,
        (EventKind::SessionStart, None, _) => {
            let session = SessionRecord::new(
                args.agent.clone(),
                args.adapter.clone(),
                current_working_directory(),
            );
            let session_id = session.id.clone();
            database.upsert_session(&session)?;
            state.current_session_id = Some(session_id.clone());
            state.save(&paths)?;
            session_id
        }
        (_, Some(session_id), _) => session_id,
        (_, None, Some(session_id)) => session_id,
        (_, None, None) => {
            let session = SessionRecord::new(
                args.agent.clone(),
                args.adapter.clone(),
                current_working_directory(),
            );
            let session_id = session.id.clone();
            database.upsert_session(&session)?;
            state.current_session_id = Some(session_id.clone());
            state.save(&paths)?;
            session_id
        }
    };

    if database.get_session(&session_id)?.is_none() {
        let mut session = SessionRecord::new(
            args.agent.clone(),
            args.adapter.clone(),
            current_working_directory(),
        );
        session.id = session_id.clone();
        database.upsert_session(&session)?;
    }

    let record = EventRecord::new(
        args.kind.as_event_type().to_string(),
        session_id.clone(),
        args.task_id,
        args.skill,
        args.agent,
        args.adapter,
        args.success,
        args.duration_ms,
        args.error,
        args.retry_count,
        args.input_summary,
        args.output_summary,
        args.planner_reason,
        args.confidence,
        args.alternatives,
        args.tokens_input,
        args.tokens_output,
        args.cost_usd,
    );
    database.insert_event(&record)?;
    append_jsonl(&paths, &record)?;

    if matches!(args.kind, EventKind::SessionEnd) {
        database.mark_session_ended(&session_id, &Utc::now().to_rfc3339())?;
        if state.current_session_id.as_deref() == Some(session_id.as_str()) {
            state.current_session_id = None;
            state.save(&paths)?;
        }
    }

    println!("{}", serde_json::to_string_pretty(&record)?);
    Ok(())
}

fn stats_command(args: FilterArgs) -> Result<()> {
    let summary = stats::StatsQuery::from_filter_args(args.since, args.agent, args.skill);
    let paths = StoragePaths::discover()?;
    let database = Database::open(&paths.database_path())?.initialize()?;
    let rows = database.skill_stats(
        summary.since_timestamp()?.as_deref(),
        summary.agent.as_deref(),
        summary.skill.as_deref(),
    )?;
    println!("{}", stats::render_skill_stats(&rows));
    Ok(())
}

fn timeline_command(args: TimelineArgs) -> Result<()> {
    let paths = StoragePaths::discover()?;
    let database = Database::open(&paths.database_path())?.initialize()?;
    let rows = database.timeline(args.session.as_deref(), args.last)?;
    println!("{}", stats::render_timeline(&rows));
    Ok(())
}

fn failures_command(args: FilterArgs) -> Result<()> {
    let summary = stats::StatsQuery::from_filter_args(args.since, args.agent, args.skill);
    let paths = StoragePaths::discover()?;
    let database = Database::open(&paths.database_path())?.initialize()?;
    let rows = database.failures(
        summary.since_timestamp()?.as_deref(),
        summary.agent.as_deref(),
        summary.skill.as_deref(),
    )?;
    println!("{}", stats::render_failures(&rows));
    Ok(())
}

fn unused_command(args: UnusedArgs) -> Result<()> {
    let paths = StoragePaths::discover()?;
    let database = Database::open(&paths.database_path())?.initialize()?;
    let observed = database.observed_skills()?;
    let report =
        stats::UnusedSkillsReport::from_declared_and_observed(&args.defined_skills, &observed)?;
    println!("{}", report.render());
    Ok(())
}

fn recommend_command() -> Result<()> {
    let paths = StoragePaths::discover()?;
    let database = Database::open(&paths.database_path())?.initialize()?;
    let observed = database.observed_skills()?;
    let stats_rows = database.skill_stats(None, None, None)?;
    let recommendations =
        recommend::build_recommendations(&stats_rows, &PathBuf::from("skills.toml"), &observed)?;

    println!("Recommendations:");
    for (index, line) in recommendations.iter().enumerate() {
        println!("{}. {line}", index + 1);
    }
    Ok(())
}

fn export_command(args: ExportArgs) -> Result<()> {
    let paths = StoragePaths::discover()?;
    let database = Database::open(&paths.database_path())?.initialize()?;

    match args.kind {
        ExportKind::Jsonl => println!("{}", export::jsonl::export_events(&database)?),
        ExportKind::Otel => println!("{}", export::otel::export_stub()),
    };
    Ok(())
}

fn mcp_command() -> Result<()> {
    println!("{}", mcp::server::startup_banner());
    for tool in mcp::tools::TOOLS {
        println!("- {tool}");
    }
    Ok(())
}

fn current_working_directory() -> Option<String> {
    env::current_dir().ok().map(|path| path.display().to_string())
}

fn append_jsonl(paths: &StoragePaths, event: &EventRecord) -> Result<()> {
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(paths.jsonl_path())
        .with_context(|| format!("failed to open {}", paths.jsonl_path().display()))?;
    writeln!(file, "{}", serde_json::to_string(event)?).context("failed to append JSONL event")
}
