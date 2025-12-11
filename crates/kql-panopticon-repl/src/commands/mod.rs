//! Command definitions and handlers
//!
//! Uses clap derive to define the command structure.
//! Each subcommand module contains its handler implementation.

pub mod jobs;
pub mod pack;
pub mod run;
pub mod workspace;

use crate::context::SharedContext;
use anyhow::Result;
use clap::{Parser, Subcommand};

/// Root command enum for the REPL
#[derive(Debug, Parser)]
#[command(name = "")]
pub struct ReplCommand {
    #[command(subcommand)]
    pub command: Command,
}

/// Available commands
#[derive(Debug, Subcommand)]
#[command(disable_help_subcommand = true)]
pub enum Command {
    /// Manage workspaces
    #[command(alias = "ws")]
    Workspace {
        #[command(subcommand)]
        action: workspace::WorkspaceAction,
    },

    /// Manage query packs
    Pack {
        #[command(subcommand)]
        action: pack::PackAction,
    },

    /// Execute queries or packs
    Run {
        /// Execute an ad-hoc query instead of loaded pack
        #[arg(long, short)]
        query: Option<String>,

        /// Timespan for query (e.g., "P7D", "PT1H")
        #[arg(long, short)]
        timespan: Option<String>,

        /// Run on all available workspaces
        #[arg(long)]
        all: bool,
    },

    /// Validate loaded pack or query
    Validate {
        /// Query to validate (uses loaded pack if not specified)
        #[arg(long, short)]
        query: Option<String>,
    },

    /// View running and completed jobs
    Jobs {
        #[command(subcommand)]
        action: Option<jobs::JobsAction>,
    },

    /// View results from an execution
    Results {
        /// Job ID or short prefix
        job_id: Option<String>,
    },

    /// Show execution history
    History {
        /// Number of entries to show
        #[arg(short, long, default_value = "10")]
        count: usize,
    },

    /// Show or set configuration
    Config {
        /// Configuration key to show or set
        key: Option<String>,
        /// Value to set
        value: Option<String>,
    },

    /// Show status summary
    Status,

    /// Clear the screen
    Clear,

    /// Show help
    Help {
        /// Command to get help for
        command: Option<String>,
    },

    /// Exit the REPL
    #[command(alias = "quit", alias = "q")]
    Exit,
}

/// Execute a parsed command
pub async fn execute(command: Command, ctx: SharedContext) -> Result<CommandResult> {
    match command {
        Command::Workspace { action } => workspace::execute(action, ctx).await,
        Command::Pack { action } => pack::execute(action, ctx).await,
        Command::Run { query, timespan, all } => run::execute(query, timespan, all, ctx).await,
        Command::Validate { query } => run::validate(query, ctx).await,
        Command::Jobs { action } => jobs::execute(action, ctx).await,
        Command::Results { job_id } => jobs::results(job_id, ctx).await,
        Command::History { count } => history(count, ctx).await,
        Command::Config { key, value } => config(key, value, ctx).await,
        Command::Status => status(ctx).await,
        Command::Clear => {
            // Clear screen handled by caller
            Ok(CommandResult::Clear)
        }
        Command::Help { command } => help(command),
        Command::Exit => Ok(CommandResult::Exit),
    }
}

/// Result of command execution
#[derive(Debug)]
pub enum CommandResult {
    /// Command completed successfully with optional message
    Success(Option<String>),
    /// Command output (multi-line)
    Output(String),
    /// Clear screen
    Clear,
    /// Exit the REPL
    Exit,
    /// Error occurred
    Error(String),
}

impl CommandResult {
    pub fn success() -> Self {
        Self::Success(None)
    }

    pub fn message(msg: impl Into<String>) -> Self {
        Self::Success(Some(msg.into()))
    }

    pub fn output(msg: impl Into<String>) -> Self {
        Self::Output(msg.into())
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self::Error(msg.into())
    }
}

/// Show execution history
async fn history(count: usize, ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let history = ctx.history();

    if history.is_empty() {
        return Ok(CommandResult::message("No executions in history"));
    }

    let mut output = String::new();
    output.push_str("Execution History:\n");
    output.push_str(&"-".repeat(60));
    output.push('\n');

    for record in history.recent().take(count) {
        let status_icon = match record.result.status {
            crate::history::ExecutionStatusSummary::Completed => "✓",
            crate::history::ExecutionStatusSummary::PartialSuccess => "◐",
            crate::history::ExecutionStatusSummary::Failed => "✗",
            crate::history::ExecutionStatusSummary::Running => "⟳",
            crate::history::ExecutionStatusSummary::Pending => "○",
        };

        output.push_str(&format!(
            "{} {} {} ({} rows, {}ms)\n",
            record.short_id(),
            status_icon,
            record.source_name(),
            record.result.total_rows,
            record.result.duration_ms,
        ));
        output.push_str(&format!(
            "   {} on {} workspace(s)\n",
            record.timestamp.format("%Y-%m-%d %H:%M:%S"),
            record.workspaces.len(),
        ));
    }

    Ok(CommandResult::output(output))
}

/// Show or set configuration
async fn config(key: Option<String>, value: Option<String>, ctx: SharedContext) -> Result<CommandResult> {
    match (key, value) {
        (None, _) => {
            let ctx = ctx.read().await;
            let mut output = String::new();
            output.push_str("Configuration:\n");
            output.push_str(&format!("  output_dir: {}\n", ctx.output_dir().display()));
            Ok(CommandResult::output(output))
        }
        (Some(key), Some(value)) => {
            let mut ctx = ctx.write().await;
            match key.as_str() {
                "output_dir" => {
                    let msg = format!("Set output_dir to {}", value);
                    ctx.set_output_dir(value.into());
                    Ok(CommandResult::message(msg))
                }
                _ => Ok(CommandResult::error(format!("Unknown config key: {}", key))),
            }
        }
        (Some(key), None) => {
            let ctx = ctx.read().await;
            match key.as_str() {
                "output_dir" => Ok(CommandResult::output(format!(
                    "output_dir: {}",
                    ctx.output_dir().display()
                ))),
                _ => Ok(CommandResult::error(format!("Unknown config key: {}", key))),
            }
        }
    }
}

/// Show status summary
async fn status(ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let summary = ctx.status_summary();

    let mut output = String::new();
    output.push_str("Status:\n");

    if summary.initialized {
        output.push_str(&format!("  Workspaces: {} available\n", summary.workspace_count));
        if let Some(ws) = &summary.selected_workspace {
            if summary.selected_count > 1 {
                output.push_str(&format!(
                    "  Selected: {} (+{} more)\n",
                    ws,
                    summary.selected_count - 1
                ));
            } else {
                output.push_str(&format!("  Selected: {}\n", ws));
            }
        } else {
            output.push_str("  Selected: (none)\n");
        }
    } else {
        output.push_str("  Not connected (run 'workspace list' to connect)\n");
    }

    if let Some(pack) = &summary.loaded_pack {
        output.push_str(&format!("  Pack: {}\n", pack));
    } else {
        output.push_str("  Pack: (none loaded)\n");
    }

    if summary.running_jobs > 0 {
        output.push_str(&format!("  Running jobs: {}\n", summary.running_jobs));
    }

    Ok(CommandResult::output(output))
}

/// Show help
fn help(command: Option<String>) -> Result<CommandResult> {
    match command {
        None => {
            let help = r#"Available commands:

  workspace (ws)  Manage workspaces
    list          Discover and list available workspaces
    select        Select workspace(s) for execution

  pack            Manage query packs
    load          Load a pack from file
    info          Show loaded pack details
    validate      Validate pack queries

  run             Execute loaded pack or ad-hoc query
    --query       Execute ad-hoc query
    --timespan    Set query timespan
    --all         Run on all workspaces

  validate        Validate query syntax

  jobs            View job status
    list          List all jobs
    view          View job details

  results         View execution results

  history         Show execution history

  config          Show/set configuration

  status          Show current status

  clear           Clear the screen

  help            Show this help

  exit (quit, q)  Exit the REPL

Type 'help <command>' for more details on a specific command.
"#;
            Ok(CommandResult::output(help))
        }
        Some(cmd) => {
            // Could expand this with per-command help
            Ok(CommandResult::message(format!(
                "Help for '{}' not yet available. Try '{} --help'",
                cmd, cmd
            )))
        }
    }
}
