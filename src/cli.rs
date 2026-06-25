use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{
    config::{AppConfig, StoragePaths},
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
    skill: Option<String>,
    #[arg(long)]
    success: Option<bool>,
    #[arg(long)]
    duration_ms: Option<i64>,
    #[arg(long)]
    input_summary: Option<String>,
    #[arg(long)]
    output_summary: Option<String>,
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
    let record = EventRecord::from_cli(
        args.kind.as_event_type().to_string(),
        args.skill,
        args.success,
        args.duration_ms,
        args.input_summary,
        args.output_summary,
    );
    println!("{}", serde_json::to_string_pretty(&record)?);
    Ok(())
}

fn stats_command(args: FilterArgs) -> Result<()> {
    let summary = stats::StatsQuery::from_filter_args(args.since, args.agent, args.skill);
    println!("{}", summary.render_table());
    Ok(())
}

fn timeline_command(args: TimelineArgs) -> Result<()> {
    let selector = if args.last {
        "last session".to_string()
    } else {
        args.session.unwrap_or_else(|| "current session".to_string())
    };

    println!("Timeline scaffold ready for {selector}");
    Ok(())
}

fn failures_command(args: FilterArgs) -> Result<()> {
    println!(
        "Failure analysis scaffold ready for agent={:?} skill={:?} since={:?}",
        args.agent, args.skill, args.since
    );
    Ok(())
}

fn unused_command(args: UnusedArgs) -> Result<()> {
    let report = stats::UnusedSkillsReport::from_file(&args.defined_skills)?;
    println!("{}", report.render());
    Ok(())
}

fn recommend_command() -> Result<()> {
    for line in recommend::default_recommendations() {
        println!("{line}");
    }
    Ok(())
}

fn export_command(args: ExportArgs) -> Result<()> {
    match args.kind {
        ExportKind::Jsonl => println!("{}", export::jsonl::export_stub()),
        ExportKind::Otel => println!("{}", export::otel::export_stub()),
    }
    Ok(())
}

fn mcp_command() -> Result<()> {
    println!("{}", mcp::server::startup_banner());
    for tool in mcp::tools::TOOLS {
        println!("- {tool}");
    }
    Ok(())
}
