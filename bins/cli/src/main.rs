//! Pack Runner CLI
//!
//! Simple CLI tool for running KQL packs against Azure Log Analytics workspaces.
//!
//! Usage:
//!   pack-runner pack.yaml                    # Run against all discovered workspaces
//!   pack-runner pack.yaml --workspace ws1    # Run against specific workspace(s)
//!   pack-runner pack.yaml --output ./results # Specify output directory

use anyhow::{Context, Result};
use clap::Parser;
use kql_panopticon::{
    Client, ExecutionStatus, FileLayer, Pack, PackExecutor, PackExecutorConfig, StepStatus,
    Workspace,
};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{error, info, warn};
use tracing_subscriber::prelude::*;

#[derive(Parser)]
#[command(name = "panopticon-cli")]
#[command(about = "Run KQL packs against Azure Log Analytics workspaces")]
#[command(version)]
struct Cli {
    /// Path to the pack (YAML file or folder)
    #[arg(value_name = "PACK_PATH")]
    pack_file: PathBuf,

    /// Workspace names to run against (comma-separated)
    /// If not specified, runs against all discovered workspaces
    #[arg(short, long, value_delimiter = ',')]
    workspace: Option<Vec<String>>,

    /// Output directory for results
    #[arg(short, long, default_value = "./output")]
    output: PathBuf,

    /// Input values (key=value format, can be repeated)
    #[arg(short, long, value_parser = parse_input)]
    input: Vec<(String, String)>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// List available workspaces and exit
    #[arg(long)]
    list_workspaces: bool,

    /// Dry run - validate pack without executing
    #[arg(long)]
    dry_run: bool,
}

fn parse_input(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid input format '{}'. Expected key=value", s));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing with console and file layers
    let log_level = if cli.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    // Create file layer for persistent logging
    let file_layer = FileLayer::new().context("Failed to create trace log file")?;
    let trace_path = file_layer.path().clone();

    // Build subscriber with console output and file logging
    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_level(true)
                .with_filter(tracing_subscriber::filter::LevelFilter::from_level(log_level)),
        )
        .with(file_layer);

    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set tracing subscriber")?;

    info!("Trace log: {:?}", trace_path);

    // Initialize Azure client
    info!("Authenticating with Azure...");
    let client = Client::new()
        .context("Failed to authenticate with Azure. Ensure you're logged in via 'az login'")?;

    // Discover workspaces
    info!("Discovering workspaces...");
    let all_workspaces = client
        .list_workspaces()
        .await
        .context("Failed to discover workspaces")?;

    if all_workspaces.is_empty() {
        error!("No workspaces found. Check your Azure permissions.");
        return Ok(());
    }

    info!("Found {} workspace(s)", all_workspaces.len());

    // Handle --list-workspaces
    if cli.list_workspaces {
        println!("\nAvailable workspaces:");
        for ws in &all_workspaces {
            println!("  - {} ({})", ws.name, ws.subscription_name);
        }
        return Ok(());
    }

    // Filter workspaces if specified
    let workspaces: Vec<Workspace> = if let Some(ref filter) = cli.workspace {
        let filtered: Vec<Workspace> = all_workspaces
            .into_iter()
            .filter(|ws| filter.iter().any(|f| ws.name.contains(f)))
            .collect();

        if filtered.is_empty() {
            error!(
                "No workspaces matched filter: {:?}. Use --list-workspaces to see available.",
                filter
            );
            return Ok(());
        }

        info!(
            "Filtered to {} workspace(s): {}",
            filtered.len(),
            filtered
                .iter()
                .map(|w| w.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        filtered
    } else {
        all_workspaces
    };

    // Load pack (supports both single files and folders)
    info!("Loading pack from {:?}...", cli.pack_file);
    let pack = Pack::load(&cli.pack_file)
        .with_context(|| format!("Failed to load pack from {:?}", cli.pack_file))?;

    info!("Pack '{}' loaded with {} step(s)", pack.name, pack.acquisition.steps.len());

    // Build inputs from CLI args
    let mut inputs: HashMap<String, String> = HashMap::new();

    // First, apply defaults from pack definition
    for input_def in &pack.acquisition.inputs {
        if let Some(ref default) = input_def.default {
            inputs.insert(input_def.name.clone(), default.clone());
        }
    }

    // Then override with CLI-provided inputs
    for (key, value) in cli.input {
        inputs.insert(key, value);
    }

    // Check for missing required inputs
    let missing: Vec<&str> = pack
        .required_inputs()
        .iter()
        .filter(|i| !inputs.contains_key(&i.name))
        .map(|i| i.name.as_str())
        .collect();

    if !missing.is_empty() {
        error!(
            "Missing required inputs: {}",
            missing.join(", ")
        );
        println!("\nRequired inputs:");
        for input in pack.required_inputs() {
            println!(
                "  --input {}=<value>{}",
                input.name,
                input
                    .description
                    .as_ref()
                    .map(|d| format!("  # {}", d))
                    .unwrap_or_default()
            );
        }
        return Ok(());
    }

    // Build executor config
    let config = PackExecutorConfig::new(pack)
        .with_inputs(inputs)
        .with_output_dir(cli.output.clone())
        .with_pack_path(&cli.pack_file);

    // Create executor
    let executor = PackExecutor::new(client);

    // Validate pack
    info!("Validating pack...");
    executor
        .validate(&config)
        .context("Pack validation failed")?;
    info!("Pack validation passed");

    // Handle --dry-run
    if cli.dry_run {
        println!("\nDry run completed successfully.");
        println!("Pack: {}", config.pack.name);
        println!("Steps: {}", config.pack.acquisition.steps.len());
        println!("Workspaces: {}", workspaces.len());
        println!("Output: {:?}", cli.output);
        return Ok(());
    }

    // Execute pack
    info!("Executing pack against {} workspace(s)...", workspaces.len());
    let result = executor
        .execute(config, workspaces, None)
        .await
        .context("Pack execution failed")?;

    // Print results summary
    println!("\n{}", "=".repeat(60));
    println!("Pack Execution Results: {}", result.pack_name);
    println!("{}", "=".repeat(60));

    println!(
        "\nStatus: {:?}",
        result.status
    );
    println!("Duration: {}ms", result.duration_ms);
    println!(
        "Workspaces: {} succeeded, {} failed",
        result.success_count(),
        result.failure_count()
    );

    if let Some(ref output_dir) = result.output_dir {
        println!("Output: {:?}", output_dir);
    }

    // Print per-workspace details
    println!("\nWorkspace Results:");
    for ws_result in result.workspace_results.values() {
        let status_icon = match ws_result.status {
            ExecutionStatus::Success => "[OK]",
            ExecutionStatus::Failed => "[FAIL]",
            ExecutionStatus::Partial => "[PARTIAL]",
            ExecutionStatus::Pending => "[PENDING]",
            ExecutionStatus::Running => "[RUNNING]",
        };

        println!(
            "\n  {} {} ({}ms)",
            status_icon, ws_result.workspace_name, ws_result.duration_ms
        );

        for (step_name, step_result) in &ws_result.step_results {
            let step_icon = match step_result.status {
                StepStatus::Success => "+",
                StepStatus::Failed => "x",
                StepStatus::Skipped => "-",
                StepStatus::Pending => ".",
                StepStatus::Running => ">",
            };

            let rows = step_result
                .row_count
                .map(|n| format!("{} rows", n))
                .unwrap_or_else(|| "-".to_string());

            println!(
                "    [{}] {}: {} ({}ms)",
                step_icon, step_name, rows, step_result.duration_ms
            );

            if let Some(ref error) = step_result.error {
                println!("        Error: {}", error);
            }
        }

        if let Some(ref reason) = ws_result.failure_reason {
            warn!("    Failure: {}", reason);
        }
    }

    println!("\n{}", "=".repeat(60));

    // Exit with appropriate code
    if result.all_succeeded() {
        info!("Pack execution completed successfully");
        Ok(())
    } else {
        error!("Pack execution completed with failures");
        std::process::exit(1);
    }
}
