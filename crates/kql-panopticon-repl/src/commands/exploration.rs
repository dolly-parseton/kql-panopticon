//! Exploring interpreter commands
//!
//! Commands for backtracking, checkpoints, and state history:
//! - `revert [N]` - Return to state #N (default: previous)
//! - `checkpoint save <name>` - Save named checkpoint
//! - `checkpoint restore <name>` - Restore checkpoint
//! - `checkpoint list` - List all checkpoints
//! - `checkpoint delete <name>` - Delete checkpoint
//! - `trace` - Show execution history tree

use super::CommandResult;
use crate::context::SharedContext;
use crate::state_graph::StateId;
use anyhow::Result;
use clap::Subcommand;

/// Checkpoint subcommand actions
#[derive(Debug, Subcommand)]
pub enum CheckpointAction {
    /// Save current state as a named checkpoint
    Save {
        /// Checkpoint name
        name: String,
    },
    /// Restore a named checkpoint
    Restore {
        /// Checkpoint name
        name: String,
    },
    /// List all checkpoints
    List,
    /// Delete a checkpoint
    Delete {
        /// Checkpoint name
        name: String,
    },
}

/// Execute the `revert` command
pub async fn revert(state_id: Option<u64>, ctx: SharedContext) -> Result<CommandResult> {
    let mut ctx = ctx.write().await;

    match state_id {
        Some(id) => {
            let target = StateId(id);
            if !ctx.state_graph().state_exists(target) {
                return Ok(CommandResult::error(format!(
                    "State #{} does not exist",
                    id
                )));
            }

            ctx.revert_to_state(target);
            Ok(CommandResult::message(format!(
                "Reverted to state #{}",
                id
            )))
        }
        None => {
            // Revert to previous state
            let current = ctx.current_state_id().value();
            if current == 0 {
                return Ok(CommandResult::error(
                    "Already at initial state #0, cannot revert further",
                ));
            }

            if ctx.revert_state() {
                let new_id = ctx.current_state_id().value();
                Ok(CommandResult::message(format!(
                    "Reverted to state #{} (previous)",
                    new_id
                )))
            } else {
                Ok(CommandResult::error("Failed to revert"))
            }
        }
    }
}

/// Execute checkpoint subcommands
pub async fn checkpoint(action: CheckpointAction, ctx: SharedContext) -> Result<CommandResult> {
    match action {
        CheckpointAction::Save { name } => checkpoint_save(name, ctx).await,
        CheckpointAction::Restore { name } => checkpoint_restore(name, ctx).await,
        CheckpointAction::List => checkpoint_list(ctx).await,
        CheckpointAction::Delete { name } => checkpoint_delete(name, ctx).await,
    }
}

/// Save a checkpoint
async fn checkpoint_save(name: String, ctx: SharedContext) -> Result<CommandResult> {
    // Validate name
    if name.is_empty() {
        return Ok(CommandResult::error("Checkpoint name cannot be empty"));
    }

    if name.contains(' ') || name.contains('\t') {
        return Ok(CommandResult::error(
            "Checkpoint name cannot contain whitespace",
        ));
    }

    let mut ctx = ctx.write().await;
    let state_id = ctx.current_state_id().value();

    ctx.save_checkpoint(&name);

    Ok(CommandResult::message(format!(
        "\x1b[32m✓\x1b[0m Checkpoint saved: {} (state #{})",
        name, state_id
    )))
}

/// Restore a checkpoint
async fn checkpoint_restore(name: String, ctx: SharedContext) -> Result<CommandResult> {
    let mut ctx = ctx.write().await;

    // Check if checkpoint exists
    let checkpoints = ctx.list_checkpoints();
    let checkpoint = checkpoints.iter().find(|(n, _)| *n == name);

    match checkpoint {
        Some((_, state_id)) => {
            let id = state_id.value();
            ctx.restore_checkpoint(&name);
            Ok(CommandResult::message(format!(
                "Restored checkpoint: {} (state #{})",
                name, id
            )))
        }
        None => {
            // Suggest similar checkpoints
            let available: Vec<_> = checkpoints.iter().map(|(n, _)| *n).collect();
            if available.is_empty() {
                Ok(CommandResult::error(format!(
                    "Checkpoint '{}' not found. No checkpoints saved.",
                    name
                )))
            } else {
                Ok(CommandResult::error(format!(
                    "Checkpoint '{}' not found.\nAvailable: {}",
                    name,
                    available.join(", ")
                )))
            }
        }
    }
}

/// List all checkpoints
async fn checkpoint_list(ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let checkpoints = ctx.list_checkpoints();

    if checkpoints.is_empty() {
        return Ok(CommandResult::message("No checkpoints saved"));
    }

    let current_id = ctx.current_state_id();

    let mut output = format!("Checkpoints ({}):\n", checkpoints.len());
    for (name, state_id) in checkpoints {
        let current_marker = if state_id == current_id { " ← current" } else { "" };
        output.push_str(&format!(
            "  {:<20} state #{}{}\n",
            name,
            state_id.value(),
            current_marker
        ));
    }

    Ok(CommandResult::output(output.trim_end().to_string()))
}

/// Delete a checkpoint
async fn checkpoint_delete(name: String, ctx: SharedContext) -> Result<CommandResult> {
    let mut ctx = ctx.write().await;

    if ctx.delete_checkpoint(&name) {
        Ok(CommandResult::message(format!(
            "\x1b[32m✓\x1b[0m Checkpoint deleted: {}",
            name
        )))
    } else {
        Ok(CommandResult::error(format!(
            "Checkpoint '{}' not found",
            name
        )))
    }
}

/// Execute the `trace` command - show execution history
pub async fn trace(tree: bool, ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let entries = ctx.state_graph().trace();

    if entries.is_empty() {
        return Ok(CommandResult::message("No execution history"));
    }

    let mut output = String::new();

    if tree {
        // Tree view - show branching structure
        output.push_str("Execution History (tree view):\n");
        output.push_str(&render_tree(&entries));
    } else {
        // Linear view - show ancestry chain to current
        output.push_str("Execution History:\n");
        output.push_str(&"-".repeat(50));
        output.push('\n');

        for entry in &entries {
            // Only show states in current chain for linear view
            if !entry.in_current_chain {
                continue;
            }

            let current_marker = if entry.is_current { " ← current" } else { "" };
            let checkpoint_marker = entry
                .checkpoint
                .as_ref()
                .map(|c| format!(" [{}]", c))
                .unwrap_or_default();

            let command = entry
                .command
                .as_deref()
                .unwrap_or("(initial)");

            output.push_str(&format!(
                "  #{:<3} {}{}{}\n",
                entry.state_id.value(),
                command,
                checkpoint_marker,
                current_marker
            ));
        }
    }

    Ok(CommandResult::output(output.trim_end().to_string()))
}

/// Render the state graph as a tree
fn render_tree(entries: &[crate::state_graph::TraceEntry]) -> String {
    use std::collections::HashMap;

    // Build children map
    let mut children: HashMap<Option<StateId>, Vec<&crate::state_graph::TraceEntry>> =
        HashMap::new();

    for entry in entries {
        children
            .entry(entry.parent)
            .or_default()
            .push(entry);
    }

    let mut output = String::new();

    // Render starting from root (None parent = state #0)
    fn render_node(
        entry: &crate::state_graph::TraceEntry,
        children: &HashMap<Option<StateId>, Vec<&crate::state_graph::TraceEntry>>,
        prefix: &str,
        is_last: bool,
        output: &mut String,
    ) {
        let connector = if prefix.is_empty() {
            ""
        } else if is_last {
            "└─"
        } else {
            "├─"
        };

        let current_marker = if entry.is_current { " ← current" } else { "" };
        let checkpoint_marker = entry
            .checkpoint
            .as_ref()
            .map(|c| format!(" [{}]", c))
            .unwrap_or_default();

        let command = entry.command.as_deref().unwrap_or("(initial)");

        output.push_str(&format!(
            "{}{}#{} {}{}{}\n",
            prefix,
            connector,
            entry.state_id.value(),
            command,
            checkpoint_marker,
            current_marker
        ));

        // Get children of this node
        if let Some(child_entries) = children.get(&Some(entry.state_id)) {
            let child_prefix = if prefix.is_empty() {
                "  ".to_string()
            } else if is_last {
                format!("{}  ", prefix)
            } else {
                format!("{}│ ", prefix)
            };

            for (i, child) in child_entries.iter().enumerate() {
                let is_last_child = i == child_entries.len() - 1;
                render_node(child, children, &child_prefix, is_last_child, output);
            }
        }
    }

    // Start with root nodes (parent = None)
    if let Some(roots) = children.get(&None) {
        for (i, root) in roots.iter().enumerate() {
            let is_last = i == roots.len() - 1;
            render_node(root, &children, "", is_last, &mut output);
        }
    }

    output
}
