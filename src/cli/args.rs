use clap::{Parser, Subcommand, ValueEnum};

/// Parse a key=value pair for --set arguments
fn parse_key_value(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=VALUE: no '=' found in '{}'", s))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

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
    /// Launch interactive TUI
    Tui,

    /// Launch interactive shell (REPL with contextual TUI)
    Shell,

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

    /// Run an investigation pack (chained queries with variable extraction)
    RunInvestigation {
        /// Path to investigation pack file (.yaml, .yml, or .json)
        /// Can be absolute path or relative to ~/.kql-panopticon/investigations/
        pack: String,

        /// Override workspace selection (comma-separated IDs, names, or 'all')
        #[arg(short, long)]
        workspaces: Option<String>,

        /// Set input variable values (can be repeated)
        /// Format: --set name=value
        #[arg(long = "set", value_parser = parse_key_value)]
        inputs: Vec<(String, String)>,

        /// Output folder override
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,

        /// Validate pack without executing
        #[arg(long)]
        validate_only: bool,

        /// Print results to stdout as JSON summary
        #[arg(long)]
        json: bool,
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
