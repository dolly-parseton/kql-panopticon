//! Command-mode shell for kql-panopticon
//!
//! A REPL-based interface with contextual TUI popups, combining the best of
//! command-line and graphical interfaces.

mod commands;
mod context;
mod handlers;
mod prompt;

pub use context::ShellContext;

use crate::client::Client;
use crate::error::Result;
use clap_repl::{ClapEditor, ReadCommandOutput};
use commands::ShellCommand;
use handlers::handle_command;
use prompt::PanopticonPrompt;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Run the interactive shell REPL
pub async fn run_shell(client: Client) -> Result<()> {
    println!("kql-panopticon v{}", env!("CARGO_PKG_VERSION"));
    println!("Type 'help' for available commands, 'exit' to quit.\n");

    // Create shared context
    let context = Arc::new(RwLock::new(ShellContext::new(client)));

    // Discover workspaces in background
    {
        let ctx = context.clone();
        tokio::spawn(async move {
            if let Ok(mut ctx) = ctx.try_write() {
                if let Err(e) = ctx.discover_workspaces().await {
                    eprintln!("Warning: Failed to discover workspaces: {}", e);
                }
            }
        });
    }

    // Create the REPL editor
    let prompt = PanopticonPrompt::new(context.clone());
    let mut editor = ClapEditor::<ShellCommand>::builder()
        .with_prompt(Box::new(prompt))
        .build();

    // Main REPL loop
    loop {
        // Poll for background job events
        {
            let mut ctx = context.write().await;
            ctx.poll_background_events();
        }

        // Read next command
        match editor.read_command() {
            ReadCommandOutput::Command(cmd) => {
                let should_exit = handle_command(cmd, context.clone()).await?;
                if should_exit {
                    break;
                }
            }
            ReadCommandOutput::EmptyLine => {
                // Just show prompt again
                continue;
            }
            ReadCommandOutput::ClapError(err) => {
                // clap error - display it
                err.print().ok();
            }
            ReadCommandOutput::CtrlC => {
                println!("^C");
                continue;
            }
            ReadCommandOutput::CtrlD => {
                println!("exit");
                break;
            }
            ReadCommandOutput::ShlexError => {
                println!("Error: Invalid input syntax");
                continue;
            }
            ReadCommandOutput::ReedlineError(e) => {
                println!("Error: {}", e);
                break;
            }
        }
    }

    println!("Goodbye!");
    Ok(())
}
