mod cli;
mod client;
mod error;
mod query_job;
mod query_pack;
mod session;
mod tui;
mod workspace;

use clap::Parser;
use cli::args::{Cli, Commands, PackFormat};
use client::Client;
use error::Result;
use std::fs::OpenOptions;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Commands::Tui) => {
            // Launch TUI (existing behavior)
            initialize_logger_to_file();
            let client = Client::new()?;
            tui::run_tui(client).await?;
        }
        Some(Commands::RunPack { pack, workspaces, format, json, validate_only }) => {
            initialize_logger_to_stderr();
            cli::run_pack::execute(pack, workspaces, format, json, validate_only).await?;
        }
        Some(Commands::ExportPack { session, output, format }) => {
            initialize_logger_to_stderr();
            let pack_format = match format {
                PackFormat::Yaml => cli::export_pack::PackFormat::Yaml,
                PackFormat::Json => cli::export_pack::PackFormat::Json,
            };
            cli::export_pack::execute(session, output, pack_format)?;
        }
    }

    Ok(())
}

fn initialize_logger_to_file() {
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("kql-panopticon.log")
        .expect("Failed to open log file");

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();
}

fn initialize_logger_to_stderr() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn")
    ).init();
}
