mod adapters;
mod cli;
mod config;
mod db;
mod event;
mod export;
mod mcp;
mod recommend;
mod stats;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    match cli.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if let Some(exit) = error.downcast_ref::<cli::WrappedCommandExit>() {
                return ExitCode::from(exit.code().clamp(0, u8::MAX as i32) as u8);
            }

            eprintln!("Error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
