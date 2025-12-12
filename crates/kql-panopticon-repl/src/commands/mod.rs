//! Command definitions and handlers
//!
//! Uses clap derive to define the command structure.
//! Each subcommand module contains its handler implementation.

pub mod authoring;
pub mod exploration;
pub mod jobs;
pub mod pack;
pub mod run;
pub mod workspace;

use crate::context::SharedContext;
use crate::validator::join_continuation_lines;
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
    // === Pack Authoring Commands ===
    /// Define a user input parameter
    #[command(alias = "in")]
    Input {
        /// Input name
        name: String,
        /// Value (opens editor if omitted)
        #[arg(value_parser = parse_assignment, trailing_var_arg = true, num_args = 0..)]
        value: Vec<String>,
    },

    /// Define a query step
    Query {
        /// Step name
        name: String,
        /// KQL query (opens editor if omitted)
        #[arg(value_parser = parse_assignment, trailing_var_arg = true, num_args = 0..)]
        kql: Vec<String>,
    },

    /// List defined inputs
    Inputs,

    /// List defined query steps
    Steps {
        /// Show full query for a specific step
        #[arg(long)]
        show: Option<String>,
    },

    /// Remove an input or step
    #[command(alias = "rm")]
    Remove {
        /// Name of input or step to remove
        name: String,
    },

    /// Edit an existing input or step
    Edit {
        /// Name of input or step to edit
        name: String,
    },

    /// Show pack session info
    Info,

    /// Start a new pack session
    New {
        /// Pack name
        #[arg(long)]
        name: Option<String>,
    },

    // === Exploring Interpreter Commands ===
    /// Revert to a previous state
    #[command(alias = "undo")]
    Revert {
        /// State ID to revert to (default: previous state)
        state_id: Option<u64>,
    },

    /// Manage checkpoints
    #[command(alias = "cp")]
    Checkpoint {
        #[command(subcommand)]
        action: exploration::CheckpointAction,
    },

    /// Show execution history
    Trace {
        /// Show as tree with branches
        #[arg(long)]
        tree: bool,
    },

    // === Workspace & Pack Management ===
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

    // === Execution ===
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

    /// Validate loaded pack, session steps, or ad-hoc query
    Validate {
        /// Validate a specific session step by name
        step: Option<String>,

        /// Validate an ad-hoc query string
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

/// Parse an assignment value (handles `= "value"` syntax)
fn parse_assignment(s: &str) -> Result<String, String> {
    // Strip leading `=` if present
    let s = s.strip_prefix('=').unwrap_or(s).trim();

    // Strip surrounding quotes if present
    let s = s
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s);

    Ok(s.to_string())
}

/// Execute a parsed command
pub async fn execute(command: Command, ctx: SharedContext) -> Result<CommandResult> {
    match command {
        // Pack authoring commands
        Command::Input { name, value } => {
            let value = if value.is_empty() {
                None
            } else {
                Some(value.join(" "))
            };
            authoring::input(name, value, ctx).await
        }
        Command::Query { name, kql } => {
            let kql = if kql.is_empty() {
                None
            } else {
                // Join parts and process line continuation sequences
                let joined = kql.join(" ");
                Some(join_continuation_lines(&joined))
            };
            authoring::query(name, kql, ctx).await
        }
        Command::Inputs => authoring::list_inputs(ctx).await,
        Command::Steps { show } => authoring::list_steps(show, ctx).await,
        Command::Remove { name } => authoring::remove(name, ctx).await,
        Command::Edit { name } => authoring::edit(name, ctx).await,
        Command::Info => authoring::info(ctx).await,
        Command::New { name } => authoring::new_session(name, ctx).await,

        // Exploring interpreter commands
        Command::Revert { state_id } => exploration::revert(state_id, ctx).await,
        Command::Checkpoint { action } => exploration::checkpoint(action, ctx).await,
        Command::Trace { tree } => exploration::trace(tree, ctx).await,

        // Workspace and pack management
        Command::Workspace { action } => workspace::execute(action, ctx).await,
        Command::Pack { action } => pack::execute(action, ctx).await,

        // Execution
        Command::Run { query, timespan, all } => run::execute(query, timespan, all, ctx).await,
        Command::Validate { step, query } => run::validate(step, query, ctx).await,
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
    #[allow(dead_code)]
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

Pack Authoring:
  input <name> [= "<value>"]   Define a user input parameter
  query <name> [= "<kql>"]     Define a query step (use \ for line continuation)
  inputs                       List defined inputs
  steps [--show <name>]        List steps or show full query for a step
  remove <name>                Remove an input or step
  info                         Show pack session info
  new [--name "<name>"]        Start a new pack session

Exploring (Backtracking & Checkpoints):
  revert [N]                   Return to state #N (default: previous)
  checkpoint (cp)              Manage named checkpoints
    save <name>                Save current state as checkpoint
    restore <name>             Return to checkpoint
    list                       List all checkpoints
    delete <name>              Delete a checkpoint
  trace [--tree]               Show execution history

Workspace & Pack Management:
  workspace (ws)               Manage workspaces
    list                       Discover and list workspaces
    select <name>              Select workspace for execution
    schema                     Show schema status for all workspaces
    schema --capture           Capture schema for selected workspace(s)
  pack                         Manage query packs
    load <path>                Load a pack from file

Execution:
  run                          Execute session steps or loaded pack
    --query "<kql>"            Execute ad-hoc query
  validate                     Validate query syntax

Jobs & Results:
  jobs                         View running/completed jobs
  results [job_id]             View execution results
  history                      Show execution history

Other:
  config                       Show/set configuration
  status                       Show current status
  clear                        Clear the screen
  help [command]               Show this help
  exit (quit, q)               Exit the REPL

Examples:
  input threat_ip = "10.0.0.1"
  query events = "SecurityEvent | take 10"
  query filtered = "{{events}} | where IP == '{{inputs.threat_ip}}'"

Multi-line queries (use \ for continuation):
  query complex = "SecurityEvent \
  . | where EventID == 4625 \
  . | project Account, IpAddress"

Exploring example:
  panopticon #3 > checkpoint save before-analysis
  panopticon #3 [before-analysis] > query step1 = "..."
  panopticon #4 > query step2 = "..."
  panopticon #5 > revert 3           # back to checkpoint
  panopticon #3 > trace              # see history
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
