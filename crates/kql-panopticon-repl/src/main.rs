//! KQL Panopticon REPL
//!
//! Interactive TUI shell for KQL query execution and pack management.
//!
//! ## Usage
//!
//! ```bash
//! kql-repl
//! ```
//!
//! ## Commands
//!
//! - `workspace list` - Discover and list available workspaces
//! - `workspace select <name>` - Select a workspace
//! - `pack load <path>` - Load a query pack
//! - `run` - Execute loaded pack
//! - `run --query "..."` - Execute ad-hoc query
//! - `jobs` - View running/completed jobs
//! - `history` - View execution history
//! - `exit` - Exit the REPL

mod commands;
mod completion;
mod context;
mod editor;
mod history;
mod input_form;
mod progress_display;
mod session;
mod state_graph;
mod tui;
mod validator;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging - use error level to avoid cluttering output
    // Users can override with RUST_LOG env var if needed
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();

    tui::run().await
}
