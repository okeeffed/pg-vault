mod aws;
mod cli;
mod config;
mod credentials;
mod tui;

use anyhow::Result;
use clap::Parser;

use cli::{run_command, Commands};

#[derive(Parser)]
#[command(name = "pg-vault")]
#[command(about = "A CLI tool for managing PostgreSQL credentials")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(command) => run_command(command),
        None => tui::run(),
    }
}
