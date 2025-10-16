mod client;
mod error;
mod query_job;
mod session;
mod tui;
mod workspace;

use client::Client;
use error::Result;
use std::fs::OpenOptions;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger to write to a file instead of stdout
    // This prevents log messages from corrupting the TUI
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("kql-panopticon.log")
        .expect("Failed to open log file");

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    // Create Azure client (no validation - happens async in TUI)
    let client = Client::new()?;

    // Run TUI (authentication and workspace loading happen in background)
    tui::run_tui(client).await?;

    Ok(())
}
