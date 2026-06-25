mod adapters;
mod cli;
mod config;
mod db;
mod event;
mod export;
mod mcp;
mod recommend;
mod stats;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    cli.run()
}
