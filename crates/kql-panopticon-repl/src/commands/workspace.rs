//! Workspace management commands

use crate::context::SharedContext;
use super::CommandResult;
use anyhow::Result;
use clap::{Subcommand, ValueHint};
use kql_panopticon_core::schema::SchemaStatus;

#[derive(Debug, Subcommand)]
pub enum WorkspaceAction {
    /// List available workspaces
    #[command(alias = "ls")]
    List,

    /// Select a workspace for execution
    Select {
        /// Workspace name to select
        #[arg(value_hint = ValueHint::Other)]
        name: Option<String>,

        /// Select all workspaces
        #[arg(long)]
        all: bool,
    },

    /// Show currently selected workspace(s)
    Current,

    /// Clear workspace selection
    Clear,

    /// Show or capture workspace schemas
    Schema {
        /// Capture schema for selected workspace(s)
        #[arg(long)]
        capture: bool,

        /// Force re-capture even if not stale
        #[arg(long)]
        force: bool,
    },
}

pub async fn execute(action: WorkspaceAction, ctx: SharedContext) -> Result<CommandResult> {
    match action {
        WorkspaceAction::List => list(ctx).await,
        WorkspaceAction::Select { name, all } => select(name, all, ctx).await,
        WorkspaceAction::Current => current(ctx).await,
        WorkspaceAction::Clear => clear(ctx).await,
        WorkspaceAction::Schema { capture, force } => schema(capture, force, ctx).await,
    }
}

async fn list(ctx: SharedContext) -> Result<CommandResult> {
    // Initialize if needed
    {
        let mut ctx = ctx.write().await;
        if !ctx.is_initialized() {
            println!("Discovering workspaces...");
            ctx.initialize().await?;
        }
    }

    let ctx = ctx.read().await;
    let workspaces = ctx.available_workspaces();
    let selected = ctx.selected_workspaces();

    if workspaces.is_empty() {
        return Ok(CommandResult::message("No workspaces found"));
    }

    let mut output = String::new();
    output.push_str(&format!("Available workspaces ({}):\n", workspaces.len()));

    // Group by subscription
    let mut by_subscription: std::collections::HashMap<&str, Vec<_>> =
        std::collections::HashMap::new();
    for ws in workspaces {
        by_subscription
            .entry(&ws.subscription_name)
            .or_default()
            .push(ws);
    }

    for (sub_name, ws_list) in by_subscription {
        output.push_str(&format!("\n  {}:\n", sub_name));
        for ws in ws_list {
            let marker = if selected.iter().any(|s| s.workspace_id == ws.workspace_id) {
                "* "
            } else {
                "  "
            };
            output.push_str(&format!(
                "  {} {} ({})\n",
                marker, ws.name, ws.location
            ));
        }
    }

    Ok(CommandResult::output(output))
}

async fn select(name: Option<String>, all: bool, ctx: SharedContext) -> Result<CommandResult> {
    // Initialize if needed
    {
        let mut ctx = ctx.write().await;
        if !ctx.is_initialized() {
            println!("Discovering workspaces...");
            ctx.initialize().await?;
        }
    }

    let mut ctx = ctx.write().await;

    if all {
        ctx.select_all_workspaces();
        let count = ctx.selected_workspaces().len();
        return Ok(CommandResult::message(format!(
            "Selected all {} workspaces",
            count
        )));
    }

    match name {
        Some(name) => {
            ctx.select_workspace(&name)?;
            Ok(CommandResult::message(format!("Selected workspace: {}", name)))
        }
        None => {
            // In future: open TUI selector popup
            // For now, list workspaces and ask for name
            let workspaces = ctx.available_workspaces();
            if workspaces.is_empty() {
                return Ok(CommandResult::error("No workspaces available"));
            }

            let mut output = String::new();
            output.push_str("Available workspaces:\n");
            for (i, ws) in workspaces.iter().enumerate() {
                output.push_str(&format!("  {}. {} ({})\n", i + 1, ws.name, ws.subscription_name));
            }
            output.push_str("\nUse 'workspace select <name>' to select one");

            Ok(CommandResult::output(output))
        }
    }
}

async fn current(ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let selected = ctx.selected_workspaces();

    if selected.is_empty() {
        return Ok(CommandResult::message("No workspace selected"));
    }

    let mut output = String::new();
    output.push_str(&format!("Selected workspace(s) ({}):\n", selected.len()));
    for ws in selected {
        output.push_str(&format!(
            "  {} ({}, {})\n",
            ws.name, ws.subscription_name, ws.location
        ));
    }

    Ok(CommandResult::output(output))
}

async fn clear(ctx: SharedContext) -> Result<CommandResult> {
    let mut ctx = ctx.write().await;
    ctx.clear_workspace_selection();
    Ok(CommandResult::message("Workspace selection cleared"))
}

/// Show schema status or trigger capture
async fn schema(capture: bool, force: bool, ctx: SharedContext) -> Result<CommandResult> {
    // Initialize if needed
    {
        let mut ctx = ctx.write().await;
        if !ctx.is_initialized() {
            println!("Discovering workspaces...");
            ctx.initialize().await?;
        }
    }

    if capture {
        // Trigger schema capture for selected workspaces
        return schema_capture(force, ctx).await;
    }

    // Show schema status for all workspaces
    let ctx = ctx.read().await;
    let workspaces = ctx.available_workspaces();
    let selected = ctx.selected_workspaces();

    if workspaces.is_empty() {
        return Ok(CommandResult::message("No workspaces found"));
    }

    let mut output = String::new();
    output.push_str("Workspace Schema Status:\n");
    output.push_str(&"-".repeat(60));
    output.push('\n');

    for ws in workspaces {
        let status = ctx.get_schema_status(&ws.workspace_id);
        let icon = match status {
            SchemaStatus::Available => "\x1b[32m●\x1b[0m",  // Green filled
            SchemaStatus::Capturing => "\x1b[33m◐\x1b[0m",  // Yellow half
            SchemaStatus::Stale => "\x1b[33m●\x1b[0m",       // Yellow filled
            SchemaStatus::None => "\x1b[31m○\x1b[0m",        // Red empty
        };
        let selected_marker = if selected.iter().any(|s| s.workspace_id == ws.workspace_id) {
            "*"
        } else {
            " "
        };
        output.push_str(&format!(
            "{} {} {:<30} {}\n",
            selected_marker, icon, ws.name, status
        ));
    }

    output.push_str(&"-".repeat(60));
    output.push('\n');
    output.push_str("Legend: \x1b[32m●\x1b[0m available  \x1b[33m●\x1b[0m stale  \x1b[33m◐\x1b[0m capturing  \x1b[31m○\x1b[0m none\n");
    output.push_str("\nUse 'workspace schema --capture' to capture schema for selected workspace(s)");

    Ok(CommandResult::output(output))
}

/// Trigger schema capture for selected workspaces
async fn schema_capture(_force: bool, ctx: SharedContext) -> Result<CommandResult> {
    use kql_panopticon_core::schema::SchemaCapture;
    use std::sync::Arc;
    use tokio::sync::RwLock as TokioRwLock;

    // Get client and workspaces needing capture
    let (client, workspaces) = {
        let mut ctx_guard = ctx.write().await;

        let client = ctx_guard.client().cloned().ok_or_else(|| {
            anyhow::anyhow!("Not connected. Run 'workspace list' first")
        })?;

        let selected = ctx_guard.selected_workspaces();
        if selected.is_empty() {
            return Ok(CommandResult::error("No workspace selected. Use 'workspace select <name>' first"));
        }

        let workspaces = selected.to_vec();

        // Ensure schema registry exists
        ctx_guard.ensure_schema_registry();

        (client, workspaces)
    };

    // Get a clone of the registry for capture
    let registry = {
        let ctx_guard = ctx.read().await;
        ctx_guard.schema_registry().unwrap().clone()
    };

    // Create shared registry for capture
    let registry = Arc::new(TokioRwLock::new(registry));
    let client = Arc::new(client);

    let mut output = String::new();
    output.push_str(&format!("Capturing schema for {} workspace(s)...\n\n", workspaces.len()));

    for ws in &workspaces {
        output.push_str(&format!("  {} ... ", ws.name));

        // Create capture orchestrator
        let capture = SchemaCapture::new(client.clone(), registry.clone());

        match capture.capture_workspace(ws, None).await {
            Ok(result) => {
                output.push_str(&format!(
                    "\x1b[32m✓\x1b[0m {} tables ({} canonical, {} custom)\n",
                    result.tables_captured,
                    result.canonical_count,
                    result.custom_count
                ));
                if !result.errors.is_empty() {
                    output.push_str(&format!("    ({} errors)\n", result.errors.len()));
                }
            }
            Err(e) => {
                output.push_str(&format!("\x1b[31m✗\x1b[0m {}\n", e));
            }
        }
    }

    // Save updated registry back to context and disk
    {
        let mut ctx_guard = ctx.write().await;
        let captured_registry = registry.read().await.clone();
        if let Some(ctx_registry) = ctx_guard.schema_registry_mut() {
            // Copy captured data back
            *ctx_registry = captured_registry;
        }
        ctx_guard.save_schema_registry()?;
    }

    output.push_str("\nSchema capture complete");

    Ok(CommandResult::output(output))
}
