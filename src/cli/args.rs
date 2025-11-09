use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "kql-panopticon")]
#[command(
    version,
    about = "Execute KQL queries across Azure Log Analytics workspaces"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Launch interactive TUI (default)
    Tui,

    /// Run a query pack
    RunPack {
        /// Path to query pack file (.yaml, .yml, or .json)
        /// Can be absolute path or relative to ~/.kql-panopticon/packs/
        pack: String,

        /// Override workspace selection (comma-separated IDs or 'all')
        #[arg(short, long)]
        workspaces: Option<String>,

        /// Output format
        #[arg(short = 'f', long, value_enum, default_value = "files")]
        format: OutputFormat,

        /// Print results to stdout as JSON (alias for --format stdout)
        #[arg(long)]
        json: bool,

        /// Validate pack without executing
        #[arg(long)]
        validate_only: bool,
    },

    /// Export a session as a query pack
    ExportPack {
        /// Session name to export
        session: String,

        /// Output path (default: ~/.kql-panopticon/packs/<session-name>.yaml)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,

        /// Output format
        #[arg(short = 'f', long, value_enum, default_value = "yaml")]
        format: PackFormat,
    },
}

#[derive(ValueEnum, Clone)]
pub enum OutputFormat {
    /// Write to files (default)
    Files,
    /// Print to stdout as JSON
    Stdout,
}

#[derive(ValueEnum, Clone)]
pub enum PackFormat {
    /// YAML format (default)
    Yaml,
    /// JSON format
    Json,
}
