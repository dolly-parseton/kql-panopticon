//! Workspace management commands

use crate::context::SharedContext;
use super::CommandResult;
use anyhow::Result;
use clap::{Subcommand, ValueHint};

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
}

pub async fn execute(action: WorkspaceAction, ctx: SharedContext) -> Result<CommandResult> {
    match action {
        WorkspaceAction::List => list(ctx).await,
        WorkspaceAction::Select { name, all } => select(name, all, ctx).await,
        WorkspaceAction::Current => current(ctx).await,
        WorkspaceAction::Clear => clear(ctx).await,
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
