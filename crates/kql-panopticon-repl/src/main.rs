//! KQL Panopticon REPL
//!
//! Interactive shell for KQL query execution and pack management.
//!
//! ## Usage
//!
//! ```bash
//! kql-repl
//! ```
//!
//! ## Commands
//!
//! - `workspace list` - Discover and list available workspaces
//! - `workspace select <name>` - Select a workspace
//! - `pack load <path>` - Load a query pack
//! - `run` - Execute loaded pack
//! - `run --query "..."` - Execute ad-hoc query
//! - `jobs` - View running/completed jobs
//! - `history` - View execution history
//! - `exit` - Exit the REPL

mod commands;
mod completer;
mod context;
mod history;

use anyhow::Result;
use clap_repl::{ClapEditor, ReadCommandOutput};
use commands::{CommandResult, ReplCommand};
use completer::PanopticonCompleter;
use context::create_shared_context;
use kql_panopticon_core::{Client, Workspace};
use reedline::{DefaultPrompt, DefaultPromptSegment, ExternalPrinter};
use std::sync::Arc;

const BANNER: &str = r#"
╦╔═╔═╗ ╦    ╔═╗╔═╗╔╗╔╔═╗╔═╗╔╦╗╦╔═╗╔═╗╔╗╔
╠╩╗║═╬╗║    ╠═╝╠═╣║║║║ ║╠═╝ ║ ║║  ║ ║║║║
╩ ╩╚═╝╚╩═╝  ╩  ╩ ╩╝╚╝╚═╝╩   ╩ ╩╚═╝╚═╝╝╚╝
"#;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging - use error level to avoid cluttering REPL output
    // Users can override with RUST_LOG env var if needed
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();

    // Print banner
    println!("{}", BANNER);
    println!("KQL Panopticon REPL v{}", VERSION);
    println!("Type 'help' for commands, 'exit' to quit.\n");

    // Create shared context
    let ctx = create_shared_context();

    // Create custom completer with context access
    let completer = PanopticonCompleter::new(ctx.clone());

    // Create external printer for async notifications
    let printer: ExternalPrinter<String> = ExternalPrinter::default();
    let printer = Arc::new(printer.clone());

    // Create REPL editor with custom completer and external printer
    let printer_for_editor = (*printer).clone();
    let mut editor = ClapEditor::<ReplCommand>::builder()
        .with_prompt(Box::new(DefaultPrompt::new(
            DefaultPromptSegment::Basic("panopticon".to_string()),
            DefaultPromptSegment::Empty,
        )))
        .with_editor_hook(move |rl| {
            rl.with_completer(Box::new(completer))
                .with_external_printer(printer_for_editor)
        })
        .build();

    // Start background workspace discovery
    {
        let ctx_clone = ctx.clone();
        let printer_clone = printer.clone();
        ctx.write().await.start_discovery();
        tokio::spawn(async move {
            match discover_workspaces_background().await {
                Ok((client, workspaces)) => {
                    let count = workspaces.len();
                    let mut ctx = ctx_clone.write().await;
                    ctx.complete_discovery(client, workspaces);
                    let _ = printer_clone.print(format!(
                        "\r\x1b[K\x1b[32m✓\x1b[0m Discovered {} workspace(s)",
                        count
                    ));
                }
                Err(e) => {
                    let mut ctx = ctx_clone.write().await;
                    ctx.fail_discovery(e.to_string());
                    let _ = printer_clone.print(format!(
                        "\r\x1b[K\x1b[31m✗\x1b[0m Discovery failed: {}",
                        truncate_str(&e.to_string(), 50)
                    ));
                }
            }
        });
    }

    // Main REPL loop
    loop {
        // Update prompt based on context status
        // Left side: context (workspace, pack selection)
        // Right side: background activity indicators
        {
            let ctx_read = ctx.read().await;
            let summary = ctx_read.status_summary();

            // Left prompt: selection context
            let left = if let Some(ws) = &summary.selected_workspace {
                if summary.selected_count > 1 {
                    format!("panopticon ({}+{})", ws, summary.selected_count - 1)
                } else {
                    format!("panopticon ({})", ws)
                }
            } else {
                "panopticon".to_string()
            };

            // Right prompt: activity indicators
            let mut right_parts = Vec::new();

            if summary.discovering {
                right_parts.push("discovering...".to_string());
            } else if let Some(err) = &summary.discovery_error {
                right_parts.push(format!("! {}", truncate_str(err, 20)));
            } else if summary.initialized {
                right_parts.push(format!("{} ws", summary.workspace_count));
            }

            if summary.running_jobs > 0 {
                right_parts.push(format!("{} jobs", summary.running_jobs));
            }

            if let Some(pack) = &summary.loaded_pack {
                right_parts.push(format!("pack:{}", pack));
            }

            let right = if right_parts.is_empty() {
                DefaultPromptSegment::Empty
            } else {
                DefaultPromptSegment::Basic(format!("[{}]", right_parts.join(" | ")))
            };

            editor.set_prompt(Box::new(DefaultPrompt::new(
                DefaultPromptSegment::Basic(left),
                right,
            )));
        }

        // Read and parse command
        let output = editor.read_command();

        match output {
            ReadCommandOutput::Command(cmd) => {
                // Execute command
                match commands::execute(cmd.command, ctx.clone()).await {
                    Ok(CommandResult::Success(Some(msg))) => {
                        println!("{}", msg);
                    }
                    Ok(CommandResult::Success(None)) => {
                        // Silent success
                    }
                    Ok(CommandResult::Output(output)) => {
                        println!("{}", output);
                    }
                    Ok(CommandResult::Clear) => {
                        // Clear screen
                        print!("\x1B[2J\x1B[1;1H");
                    }
                    Ok(CommandResult::Exit) => {
                        println!("Goodbye!");
                        break;
                    }
                    Ok(CommandResult::Error(msg)) => {
                        eprintln!("Error: {}", msg);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
            ReadCommandOutput::EmptyLine => {
                // Do nothing for empty line
            }
            ReadCommandOutput::ClapError(e) => {
                // Print clap error (help, version, or parse error)
                let _ = e.print();
            }
            ReadCommandOutput::ShlexError => {
                eprintln!("Error: Invalid input syntax");
            }
            ReadCommandOutput::ReedlineError(e) => {
                eprintln!("Input error: {}", e);
            }
            ReadCommandOutput::CtrlC => {
                println!("Goodbye!");
                break;
            }
            ReadCommandOutput::CtrlD => {
                // Exit on Ctrl+D
                println!("Goodbye!");
                break;
            }
        }
    }

    Ok(())
}

/// Discover workspaces in the background
async fn discover_workspaces_background() -> Result<(Client, Vec<Workspace>)> {
    let client = Client::new()?;
    let workspaces = client.list_workspaces().await?;
    Ok((client, workspaces))
}

/// Truncate a string to max length, adding "..." if truncated
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
