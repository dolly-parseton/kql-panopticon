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
use crate::session::InputDef;
use crate::validator::join_continuation_lines;
use anyhow::Result;
use clap::{Parser, Subcommand};
use kql_panopticon_core::pack::Pack;
use std::path::PathBuf;

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
    /// Define or edit a user input parameter
    #[command(alias = "in")]
    Input {
        /// Input name
        name: String,
        /// Value (opens editor if omitted; edits existing if name exists)
        #[arg(value_parser = parse_assignment, trailing_var_arg = true, num_args = 0..)]
        value: Vec<String>,
    },

    /// Define or edit a query step
    Query {
        /// Step name
        name: String,
        /// KQL query (opens editor if omitted; edits existing if name exists)
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
        /// Pack file path to execute directly (session unchanged)
        path: Option<std::path::PathBuf>,

        /// Execute an ad-hoc query instead of session/pack
        #[arg(long, short)]
        query: Option<String>,

        /// Timespan for query (e.g., "P7D", "PT1H")
        #[arg(long, short)]
        timespan: Option<String>,

        /// Run on all available workspaces
        #[arg(long)]
        all: bool,
    },

    /// Sample a single step with limited results
    Sample {
        /// Step name to sample
        step: String,

        /// Maximum rows to return
        #[arg(short, long, default_value = "10")]
        limit: usize,
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

    /// Peek at step results from last execution
    Peek {
        /// Step name to peek at
        step: String,
        /// Job ID or short prefix (default: last execution)
        #[arg(long)]
        job: Option<String>,
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
        Command::Run { path, query, timespan, all } => run::execute(path, query, timespan, all, ctx).await,
        Command::Sample { step, limit } => run::sample(step, limit, ctx).await,
        Command::Validate { step, query } => run::validate(step, query, ctx).await,
        Command::Jobs { action } => jobs::execute(action, ctx).await,
        Command::Results { job_id } => jobs::results(job_id, ctx).await,
        Command::History { count } => history(count, ctx).await,
        Command::Peek { step, job } => peek_step_results(step, job, ctx).await,
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

/// Context needed to continue execution after input collection
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// The pack to execute
    pub pack: Pack,
    /// Path to the pack file (if loaded from file)
    pub pack_path: Option<PathBuf>,
    /// Whether to run on all workspaces
    pub all_workspaces: bool,
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
    /// Open KQL editor for a step
    EditStep { name: String, content: String },
    /// Open KQL editor to create a new step
    NewStep { name: String },
    /// Open YAML editor for an input (future)
    EditInput { name: String, content: String },
    /// View step results in a results widget
    ViewResults {
        name: String,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// Inputs required before execution can proceed (TUI mode)
    InputsRequired {
        /// Input definitions to collect
        inputs: Vec<InputDef>,
        /// Context needed to continue execution
        context: ExecutionContext,
    },
    /// Open input definition form (for `input <name>` without value)
    DefineInput {
        /// Input name
        name: String,
        /// Existing input definition if editing
        existing: Option<InputDef>,
    },
    /// Execution completed with results (TUI mode)
    ExecutionComplete {
        /// Summary text
        summary: String,
        /// Step results with their data
        step_results: Vec<StepResultData>,
    },
    /// Start execution immediately with provided inputs (TUI mode)
    ///
    /// Unlike InputsRequired, this doesn't show a form - execution starts immediately.
    /// Used when all inputs are already available (e.g., defaults or no inputs needed).
    StartExecution {
        /// Context for execution
        context: ExecutionContext,
        /// Pre-filled input values
        inputs: std::collections::HashMap<String, String>,
    },
    /// Open workspace selector widget (TUI mode)
    SelectWorkspaces,
}

/// Data for a single step's execution result
#[derive(Debug, Clone)]
pub struct StepResultData {
    /// Step name
    pub name: String,
    /// Column headers
    pub columns: Vec<String>,
    /// Data rows
    pub rows: Vec<Vec<String>>,
    /// Row count
    pub row_count: usize,
    /// Whether step succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
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

/// Peek at step results from an execution
async fn peek_step_results(step: String, job_id: Option<String>, ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let history = ctx.history();

    // Find the execution record
    let record = match &job_id {
        Some(id) => history.get_by_prefix(id),
        None => history.last(),
    };

    let record = match record {
        Some(r) => r,
        None => {
            return Ok(CommandResult::error(if job_id.is_some() {
                "Execution not found"
            } else {
                "No executions in history"
            }));
        }
    };

    // Get output path
    let output_path = match &record.output_path {
        Some(p) => p,
        None => return Ok(CommandResult::error("No output path for this execution")),
    };

    // Find CSV file for the step - search recursively in output_path
    // Structure: output_path/{subscription}/{workspace}/{timestamp}/{step}.csv
    let step_csv = format!("{}.csv", step);
    let csv_path = find_step_csv(output_path, &step_csv).await?;

    match csv_path {
        Some(path) => {
            // Read and parse CSV
            let content = tokio::fs::read_to_string(&path).await
                .map_err(|e| anyhow::anyhow!("Failed to read results: {}", e))?;

            let (columns, rows) = parse_csv(&content);

            if rows.is_empty() {
                return Ok(CommandResult::message(format!(
                    "Step '{}' has no results",
                    step
                )));
            }

            Ok(CommandResult::ViewResults {
                name: step,
                columns,
                rows,
            })
        }
        None => Ok(CommandResult::error(format!(
            "No results found for step '{}' in execution {}",
            step,
            record.short_id()
        ))),
    }
}

/// Find a step CSV file recursively within the output directory
async fn find_step_csv(base_path: &std::path::Path, filename: &str) -> Result<Option<std::path::PathBuf>> {
    use tokio::fs;

    if !base_path.exists() {
        return Ok(None);
    }

    // Check if the file is directly in the base path
    let direct_path = base_path.join(filename);
    if direct_path.exists() {
        return Ok(Some(direct_path));
    }

    // Otherwise search subdirectories (BFS)
    let mut dirs_to_check = vec![base_path.to_path_buf()];

    while let Some(dir) = dirs_to_check.pop() {
        let mut entries = match fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                dirs_to_check.push(path.clone());
            } else if path.file_name().map(|n| n == filename).unwrap_or(false) {
                return Ok(Some(path));
            }
        }
    }

    Ok(None)
}

/// Parse CSV content into columns and rows
fn parse_csv(content: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut lines = content.lines();

    // First line is header
    let columns: Vec<String> = match lines.next() {
        Some(header) => header.split(',').map(|s| s.trim().to_string()).collect(),
        None => return (vec![], vec![]),
    };

    // Remaining lines are data
    let rows: Vec<Vec<String>> = lines
        .filter(|line| !line.is_empty())
        .map(|line| {
            // Simple CSV parsing - handles basic cases
            // For quoted fields with commas, we'd need more sophisticated parsing
            parse_csv_line(line)
        })
        .collect();

    (columns, rows)
}

/// Parse a single CSV line, handling quoted fields
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                // Check for escaped quote ("")
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    in_quotes = false;
                }
            }
            '"' if !in_quotes => {
                in_quotes = true;
            }
            ',' if !in_quotes => {
                fields.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
    }

    // Don't forget the last field
    fields.push(current.trim().to_string());
    fields
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
  peek <step> [--job <id>]     Peek at step results from last execution

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
