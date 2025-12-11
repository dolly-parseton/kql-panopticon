//! Command handlers for the shell
//!
//! Each command in ShellCommand has a corresponding handler function.

use super::commands::*;
use super::context::ShellContext;
use crate::error::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Handle a shell command, returning true if the shell should exit
pub async fn handle_command(
    cmd: ShellCommand,
    context: Arc<RwLock<ShellContext>>,
) -> Result<bool> {
    match cmd {
        // Exit commands
        ShellCommand::Exit | ShellCommand::Quit => return Ok(true),

        // Clear screen
        ShellCommand::Clear => {
            print!("\x1B[2J\x1B[1;1H");
            Ok(false)
        }

        // Status
        ShellCommand::Status => {
            handle_status(context).await?;
            Ok(false)
        }

        // Workspace commands
        ShellCommand::Workspace(cmd) => {
            handle_workspace(cmd, context).await?;
            Ok(false)
        }

        // Pack commands
        ShellCommand::Pack(cmd) => {
            handle_pack(cmd, context).await?;
            Ok(false)
        }

        // Investigation commands
        ShellCommand::Investigation(cmd) => {
            handle_investigation(cmd, context).await?;
            Ok(false)
        }

        // Session commands
        ShellCommand::Session(cmd) => {
            handle_session(cmd, context).await?;
            Ok(false)
        }

        // Query
        ShellCommand::Query { load, edit } => {
            handle_query(load, edit, context).await?;
            Ok(false)
        }

        // Run
        ShellCommand::Run { workspace, all, query } => {
            handle_run(workspace, all, query, context).await?;
            Ok(false)
        }

        // Validate
        ShellCommand::Validate => {
            handle_validate(context).await?;
            Ok(false)
        }

        // Jobs
        ShellCommand::Jobs => {
            handle_jobs(context).await?;
            Ok(false)
        }

        // Job detail
        ShellCommand::JobDetail { id, logs } => {
            handle_job_detail(id, logs, context).await?;
            Ok(false)
        }

        // Foreground
        ShellCommand::Fg => {
            handle_fg(context).await?;
            Ok(false)
        }

        // Results
        ShellCommand::Results { job_id, export, format } => {
            handle_results(job_id, export, format, context).await?;
            Ok(false)
        }

        // Config
        ShellCommand::Config(cmd) => {
            handle_config(cmd, context).await?;
            Ok(false)
        }
    }
}

async fn handle_status(context: Arc<RwLock<ShellContext>>) -> Result<()> {
    let ctx = context.read().await;

    println!("KQL Panopticon Status");
    println!("─────────────────────");
    println!();

    // Workspaces
    println!("Workspaces: {} discovered", ctx.workspaces.len());
    if let Some(ws) = ctx.primary_workspace() {
        println!("  Selected: {} ({})", ws.name, ws.subscription_name);
    }
    println!();

    // Pack
    if let Some(pack) = &ctx.current_pack {
        let queries = pack.pack.get_queries();
        println!("Loaded Pack: {}", pack.pack.name);
        println!("  Queries: {}", queries.len());
        if let Some(idx) = pack.selected_query {
            if let Some(q) = queries.get(idx) {
                println!("  Selected: {} ({})", idx + 1, q.name);
            }
        }
    } else {
        println!("Loaded Pack: (none)");
    }
    println!();

    // Session
    if let Some(name) = &ctx.session_name {
        let dirty = if ctx.session_dirty { " (unsaved)" } else { "" };
        println!("Session: {}{}", name, dirty);
    } else {
        println!("Session: (none)");
    }
    println!();

    // Jobs
    let running = ctx.running_job_count();
    let total = ctx.background_jobs.len();
    println!("Background Jobs: {} running, {} total", running, total);

    Ok(())
}

async fn handle_workspace(cmd: WorkspaceCommand, context: Arc<RwLock<ShellContext>>) -> Result<()> {
    match cmd {
        WorkspaceCommand::List => {
            let ctx = context.read().await;
            if ctx.workspaces.is_empty() {
                println!("No workspaces discovered. Try 'workspace refresh'.");
            } else {
                println!("Available Workspaces:");
                for (i, ws) in ctx.workspaces.iter().enumerate() {
                    let selected = if ctx.selected_workspaces.contains(&i) {
                        " *"
                    } else {
                        "  "
                    };
                    println!("{}[{}] {} ({})", selected, i + 1, ws.name, ws.subscription_name);
                }
            }
        }
        WorkspaceCommand::Select { workspace } => {
            let mut ctx = context.write().await;
            if let Some(ws_ref) = workspace {
                // Try to find by name or index
                if let Ok(idx) = ws_ref.parse::<usize>() {
                    if idx > 0 && idx <= ctx.workspaces.len() {
                        ctx.selected_workspaces = vec![idx - 1];
                        let ws = &ctx.workspaces[idx - 1];
                        println!("Selected: {} ({})", ws.name, ws.subscription_name);
                    } else {
                        println!("Invalid workspace index: {}", idx);
                    }
                } else {
                    // Search by name
                    if let Some(idx) = ctx
                        .workspaces
                        .iter()
                        .position(|w| w.name.to_lowercase().contains(&ws_ref.to_lowercase()))
                    {
                        ctx.selected_workspaces = vec![idx];
                        let ws = &ctx.workspaces[idx];
                        println!("Selected: {} ({})", ws.name, ws.subscription_name);
                    } else {
                        println!("Workspace not found: {}", ws_ref);
                    }
                }
            } else {
                // TODO: Show TUI selector
                println!("Interactive workspace selection not yet implemented.");
                println!("Use 'workspace list' and 'workspace select <name or number>'");
            }
        }
        WorkspaceCommand::Info { workspace } => {
            let ctx = context.read().await;
            let ws = if let Some(ws_ref) = workspace {
                ctx.workspaces
                    .iter()
                    .find(|w| w.name.to_lowercase().contains(&ws_ref.to_lowercase()))
            } else {
                ctx.primary_workspace()
            };

            if let Some(ws) = ws {
                println!("Workspace: {}", ws.name);
                println!("  ID: {}", ws.workspace_id);
                println!("  Location: {}", ws.location);
                println!("  Resource Group: {}", ws.resource_group);
                println!("  Subscription: {} ({})", ws.subscription_name, ws.subscription_id);
            } else {
                println!("No workspace selected or found.");
            }
        }
        WorkspaceCommand::Refresh => {
            let mut ctx = context.write().await;
            println!("Discovering workspaces...");
            ctx.discover_workspaces().await?;
            println!("Found {} workspaces.", ctx.workspaces.len());
        }
    }
    Ok(())
}

async fn handle_pack(cmd: PackCommand, context: Arc<RwLock<ShellContext>>) -> Result<()> {
    match cmd {
        PackCommand::List => {
            println!("Pack listing not yet implemented.");
            println!("Use 'pack load <path>' to load a pack file.");
        }
        PackCommand::Load { pack } => {
            let path = std::path::PathBuf::from(&pack);
            let query_pack = crate::query_pack::QueryPack::load_from_file(&path)?;

            let mut ctx = context.write().await;
            let queries = query_pack.get_queries();
            println!("Loaded: {} ({} queries)", query_pack.name, queries.len());
            for (i, q) in queries.iter().enumerate() {
                println!("  {}. {}", i + 1, q.name);
            }

            ctx.current_pack = Some(super::context::LoadedPack {
                pack: query_pack,
                path,
                selected_query: None,
            });
            ctx.session_dirty = true;
        }
        PackCommand::Info { pack: _ } => {
            let ctx = context.read().await;
            if let Some(loaded) = &ctx.current_pack {
                let queries = loaded.pack.get_queries();
                println!("Pack: {}", loaded.pack.name);
                if let Some(desc) = &loaded.pack.description {
                    println!("Description: {}", desc);
                }
                println!("Path: {}", loaded.path.display());
                println!("Queries: {}", queries.len());
                for (i, q) in queries.iter().enumerate() {
                    let selected = if Some(i) == loaded.selected_query {
                        " *"
                    } else {
                        "  "
                    };
                    println!("{}[{}] {}", selected, i + 1, q.name);
                }
            } else {
                println!("No pack loaded.");
            }
        }
        PackCommand::Unload => {
            let mut ctx = context.write().await;
            if ctx.current_pack.is_some() {
                ctx.current_pack = None;
                ctx.session_dirty = true;
                println!("Pack unloaded.");
            } else {
                println!("No pack loaded.");
            }
        }
    }
    Ok(())
}

async fn handle_investigation(
    cmd: InvestigationCommand,
    _context: Arc<RwLock<ShellContext>>,
) -> Result<()> {
    match cmd {
        InvestigationCommand::List => {
            println!("Investigation listing not yet implemented.");
        }
        InvestigationCommand::Load { pack } => {
            println!("Loading investigation: {}", pack);
            println!("Investigation loading not yet implemented.");
        }
        InvestigationCommand::Info { pack: _ } => {
            println!("Investigation info not yet implemented.");
        }
        InvestigationCommand::Run { pack, set, output } => {
            println!(
                "Running investigation: {:?} with inputs: {:?}, output: {:?}",
                pack, set, output
            );
            println!("Investigation execution not yet implemented.");
        }
        InvestigationCommand::New { name } => {
            println!("Creating new investigation: {}", name);
            println!("Investigation creation not yet implemented.");
        }
        InvestigationCommand::Step(step_cmd) => match step_cmd {
            InvestigationStepCommand::Add { name, depends } => {
                println!("Adding step: {} (depends on: {:?})", name, depends);
            }
            InvestigationStepCommand::Edit { name } => {
                println!("Editing step: {}", name);
            }
            InvestigationStepCommand::Remove { name } => {
                println!("Removing step: {}", name);
            }
            InvestigationStepCommand::List => {
                println!("Listing steps...");
            }
        },
    }
    Ok(())
}

async fn handle_session(cmd: SessionCommand, _context: Arc<RwLock<ShellContext>>) -> Result<()> {
    match cmd {
        SessionCommand::List => {
            println!("Session listing not yet implemented.");
        }
        SessionCommand::Save { name } => {
            println!("Saving session: {}", name);
            println!("Session saving not yet implemented.");
        }
        SessionCommand::Load { name } => {
            println!("Loading session: {}", name);
            println!("Session loading not yet implemented.");
        }
        SessionCommand::Delete { name } => {
            println!("Deleting session: {}", name);
            println!("Session deletion not yet implemented.");
        }
    }
    Ok(())
}

async fn handle_query(
    load: Option<std::path::PathBuf>,
    edit: Option<usize>,
    _context: Arc<RwLock<ShellContext>>,
) -> Result<()> {
    if let Some(path) = load {
        println!("Loading query from: {}", path.display());
        println!("Query loading not yet implemented.");
    } else if let Some(idx) = edit {
        println!("Editing query {}", idx);
        println!("Query editing not yet implemented.");
    } else {
        println!("Opening query editor...");
        println!("Query editor not yet implemented.");
        println!("Hint: Use 'pack load <file>' to load queries from a pack.");
    }
    Ok(())
}

async fn handle_run(
    workspace: Option<String>,
    all: bool,
    query: Option<usize>,
    context: Arc<RwLock<ShellContext>>,
) -> Result<()> {
    let ctx = context.read().await;

    // Determine target workspaces
    let target_workspaces: Vec<&crate::workspace::Workspace> = if all {
        ctx.workspaces.iter().collect()
    } else if let Some(ws_name) = workspace {
        ctx.workspaces
            .iter()
            .filter(|w| w.name.to_lowercase().contains(&ws_name.to_lowercase()))
            .collect()
    } else {
        ctx.selected_workspace_list()
    };

    if target_workspaces.is_empty() {
        println!("No workspaces selected. Use 'workspace select' first.");
        return Ok(());
    }

    // Determine query to run
    let query_to_run = if let Some(pack) = &ctx.current_pack {
        let queries = pack.pack.get_queries();
        if let Some(idx) = query {
            if idx > 0 && idx <= queries.len() {
                Some((queries[idx - 1].query.clone(), queries[idx - 1].name.clone()))
            } else {
                println!("Invalid query index: {}", idx);
                return Ok(());
            }
        } else if let Some(idx) = pack.selected_query {
            queries.get(idx).map(|q| (q.query.clone(), q.name.clone()))
        } else if queries.len() == 1 {
            Some((queries[0].query.clone(), queries[0].name.clone()))
        } else {
            println!("Multiple queries in pack. Use 'run --query N' to specify which one.");
            println!("Available queries:");
            for (i, q) in queries.iter().enumerate() {
                println!("  {}. {}", i + 1, q.name);
            }
            return Ok(());
        }
    } else if let Some(q) = &ctx.current_query {
        Some((q.clone(), "adhoc".to_string()))
    } else {
        None
    };

    if let Some((query, name)) = query_to_run {
        println!("Executing: {}", name);
        println!("Workspaces: {}", target_workspaces.iter().map(|w| w.name.as_str()).collect::<Vec<_>>().join(", "));
        println!("Query execution not yet implemented.");
        println!();
        println!("Query preview:");
        println!("{}", query.lines().take(5).collect::<Vec<_>>().join("\n"));
        if query.lines().count() > 5 {
            println!("... ({} more lines)", query.lines().count() - 5);
        }
    } else {
        println!("No query to run. Load a pack or use 'query' to enter a query.");
    }

    Ok(())
}

async fn handle_validate(_context: Arc<RwLock<ShellContext>>) -> Result<()> {
    println!("Query validation not yet implemented.");
    Ok(())
}

async fn handle_jobs(context: Arc<RwLock<ShellContext>>) -> Result<()> {
    let ctx = context.read().await;

    if ctx.background_jobs.is_empty() {
        println!("No background jobs.");
    } else {
        println!("Background Jobs:");
        for job in ctx.background_jobs.values() {
            let status = match job.status {
                super::context::JobStatus::Running => "⟳ running",
                super::context::JobStatus::Completed => "✓ complete",
                super::context::JobStatus::Failed => "✗ failed",
            };
            let progress = job
                .progress_message
                .as_ref()
                .map(|m| format!(" - {}", m))
                .unwrap_or_default();
            println!("  [{}] {} {}{}", job.id, job.name, status, progress);
        }
    }
    Ok(())
}

async fn handle_job_detail(
    id: String,
    logs: bool,
    context: Arc<RwLock<ShellContext>>,
) -> Result<()> {
    let ctx = context.read().await;

    if let Some(job) = ctx.background_jobs.get(&id) {
        println!("Job: {} ({})", job.name, job.id);
        println!("Type: {:?}", job.job_type);
        println!("Status: {:?}", job.status);
        println!("Started: {}", job.started_at.format("%Y-%m-%d %H:%M:%S"));
        if let Some(msg) = &job.progress_message {
            println!("Progress: {}", msg);
        }
        if logs {
            println!("\nLogs: (not yet implemented)");
        }
    } else {
        println!("Job not found: {}", id);
    }
    Ok(())
}

async fn handle_fg(_context: Arc<RwLock<ShellContext>>) -> Result<()> {
    println!("Foreground job monitor not yet implemented.");
    Ok(())
}

async fn handle_results(
    job_id: Option<String>,
    export: Option<std::path::PathBuf>,
    format: String,
    _context: Arc<RwLock<ShellContext>>,
) -> Result<()> {
    println!(
        "Results viewer for job {:?} (export: {:?}, format: {}) not yet implemented.",
        job_id, export, format
    );
    Ok(())
}

async fn handle_config(cmd: ConfigCommand, _context: Arc<RwLock<ShellContext>>) -> Result<()> {
    match cmd {
        ConfigCommand::Show => {
            println!("Configuration:");
            println!("  output_dir: ./output");
            println!("  (more config options not yet implemented)");
        }
        ConfigCommand::Set { key, value } => {
            println!("Setting {} = {}", key, value);
            println!("Configuration not yet implemented.");
        }
        ConfigCommand::Reset => {
            println!("Resetting configuration...");
            println!("Configuration not yet implemented.");
        }
    }
    Ok(())
}
