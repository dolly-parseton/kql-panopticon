//! Shell command definitions
//!
//! Uses clap derive for automatic parsing, completions, and help generation.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Shell command enum - all available commands in the REPL
#[derive(Debug, Parser)]
#[command(name = "", multicall = true)]
pub enum ShellCommand {
    // === Navigation & Context ===
    /// Manage workspaces
    #[command(subcommand)]
    Workspace(WorkspaceCommand),

    /// Manage query packs
    #[command(subcommand)]
    Pack(PackCommand),

    /// Manage investigation packs
    #[command(subcommand)]
    Investigation(InvestigationCommand),

    /// Manage sessions
    #[command(subcommand)]
    Session(SessionCommand),

    // === Query Operations ===
    /// Open query editor or create new query
    Query {
        /// Load query from file
        #[arg(short, long)]
        load: Option<PathBuf>,

        /// Edit query by index (from loaded pack)
        #[arg(short, long)]
        edit: Option<usize>,
    },

    /// Execute current query or pack
    Run {
        /// Target workspace (or 'all')
        #[arg(short, long)]
        workspace: Option<String>,

        /// Run on all workspaces
        #[arg(long)]
        all: bool,

        /// Query index from pack to run
        #[arg(short, long)]
        query: Option<usize>,
    },

    /// Validate query syntax without executing
    Validate,

    // === Job Management ===
    /// List background jobs
    Jobs,

    /// View job details or results
    #[command(name = "job")]
    JobDetail {
        /// Job ID to view
        id: String,

        /// Show full logs
        #[arg(short, long)]
        logs: bool,
    },

    /// Bring background job monitor to foreground
    Fg,

    /// View results from a job
    Results {
        /// Job ID (defaults to last completed)
        job_id: Option<String>,

        /// Export results to file
        #[arg(short, long)]
        export: Option<PathBuf>,

        /// Export format (csv, json)
        #[arg(short, long, default_value = "csv")]
        format: String,
    },

    // === General ===
    /// Show current status
    Status,

    /// Configure settings
    #[command(subcommand)]
    Config(ConfigCommand),

    /// Clear the screen
    Clear,

    /// Exit the shell
    Exit,

    /// Exit the shell (alias)
    Quit,
}

/// Workspace management commands
#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// List available workspaces
    List,

    /// Select active workspace
    Select {
        /// Workspace name or ID
        workspace: Option<String>,
    },

    /// Show workspace details
    Info {
        /// Workspace name or ID
        workspace: Option<String>,
    },

    /// Refresh workspace list
    Refresh,
}

/// Pack management commands
#[derive(Debug, Subcommand)]
pub enum PackCommand {
    /// List available packs
    List,

    /// Load a pack
    Load {
        /// Pack name or path
        pack: String,
    },

    /// Show pack details
    Info {
        /// Pack name or path
        pack: Option<String>,
    },

    /// Unload current pack
    Unload,
}

/// Investigation management commands
#[derive(Debug, Subcommand)]
pub enum InvestigationCommand {
    /// List available investigation packs
    List,

    /// Load an investigation pack
    Load {
        /// Investigation pack name or path
        pack: String,
    },

    /// Show investigation details
    Info {
        /// Investigation name or path
        pack: Option<String>,
    },

    /// Run an investigation
    Run {
        /// Investigation to run (or use loaded)
        pack: Option<String>,

        /// Set input variables (key=value)
        #[arg(short, long, value_parser = parse_key_value)]
        set: Vec<(String, String)>,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Create new investigation interactively
    New {
        /// Investigation name
        name: String,
    },

    /// Add step to current investigation
    #[command(subcommand)]
    Step(InvestigationStepCommand),
}

/// Investigation step commands
#[derive(Debug, Clone, Subcommand)]
pub enum InvestigationStepCommand {
    /// Add a new step
    Add {
        /// Step name
        name: String,

        /// Dependencies
        #[arg(short, long)]
        depends: Vec<String>,
    },

    /// Edit a step
    Edit {
        /// Step name
        name: String,
    },

    /// Remove a step
    Remove {
        /// Step name
        name: String,
    },

    /// List all steps
    List,
}

/// Session management commands
#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    /// List saved sessions
    List,

    /// Save current session
    Save {
        /// Session name
        name: String,
    },

    /// Load a saved session
    Load {
        /// Session name
        name: String,
    },

    /// Delete a session
    Delete {
        /// Session name
        name: String,
    },
}

/// Configuration commands
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Show current configuration
    Show,

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,

        /// Value to set
        value: String,
    },

    /// Reset to default configuration
    Reset,
}

/// Parse key=value pairs for --set arguments
fn parse_key_value(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("Invalid format: '{}'. Expected key=value", s))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}
