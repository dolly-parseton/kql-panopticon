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
mod editor;
mod history;
mod input_form;
mod progress_display;
mod prompt;
mod session;
mod state_graph;
mod validator;

use anyhow::Result;
use clap_repl::{ClapEditor, ReadCommandOutput};
use commands::{CommandResult, ReplCommand};
use completer::PanopticonCompleter;
use context::create_shared_context;
use kql_panopticon_core::{Client, Workspace};
use prompt::DynamicPrompt;
use reedline::ExternalPrinter;
use validator::LineContinuationValidator;
use std::sync::Arc;

const BANNER: &str = r#"
╦╔═╔═╗ ╦    ╔═╗╔═╗╔╗╔╔═╗╔═╗╔╦╗╦╔═╗╔═╗╔╗╔
╠╩╗║═╬╗║    ╠═╝╠═╣║║║║ ║╠═╝ ║ ║║  ║ ║║║║
╩ ╩╚═╝╚╩═╝  ╩  ╩ ╩╝╚╝╚═╝╩   ╩ ╩╚═╝╚═╝╝╚╝
"#;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Global left padding for all REPL output
pub const OUTPUT_INDENT: &str = "    ";

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

    // Create dynamic prompt that reads from context
    let dynamic_prompt = DynamicPrompt::new(ctx.clone());

    // Create external printer for async notifications
    let printer: ExternalPrinter<String> = ExternalPrinter::default();
    let printer = Arc::new(printer.clone());

    // Create REPL editor with custom completer, dynamic prompt, validator, and external printer
    let printer_for_editor = (*printer).clone();
    let line_validator = LineContinuationValidator::new();
    let mut editor = ClapEditor::<ReplCommand>::builder()
        .with_prompt(Box::new(dynamic_prompt))
        .with_editor_hook(move |rl| {
            rl.with_completer(Box::new(completer))
                .with_external_printer(printer_for_editor)
                .with_validator(Box::new(line_validator))
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
                        "\r\x1b[K{}\x1b[32m✓\x1b[0m Discovered {} workspace(s)",
                        OUTPUT_INDENT,
                        count
                    ));
                }
                Err(e) => {
                    let mut ctx = ctx_clone.write().await;
                    ctx.fail_discovery(e.to_string());
                    let _ = printer_clone.print(format!(
                        "\r\x1b[K{}\x1b[31m✗\x1b[0m Discovery failed: {}",
                        OUTPUT_INDENT,
                        truncate_str(&e.to_string(), 50)
                    ));
                }
            }
        });
    }

    // Main REPL loop
    // The DynamicPrompt automatically reads from context on each render,
    // so we don't need to update the prompt manually
    loop {
        // Read and parse command
        let output = editor.read_command();

        match output {
            ReadCommandOutput::Command(cmd) => {
                // Execute command
                match commands::execute(cmd.command, ctx.clone()).await {
                    Ok(CommandResult::Success(Some(msg))) => {
                        // Add indent to each line
                        for line in msg.lines() {
                            println!("{}{}", OUTPUT_INDENT, line);
                        }
                    }
                    Ok(CommandResult::Success(None)) => {
                        // Silent success
                    }
                    Ok(CommandResult::Output(output)) => {
                        // Add indent to each line
                        for line in output.lines() {
                            println!("{}{}", OUTPUT_INDENT, line);
                        }
                    }
                    Ok(CommandResult::Clear) => {
                        // Clear screen
                        print!("\x1B[2J\x1B[1;1H");
                    }
                    Ok(CommandResult::Exit) => {
                        println!("{}Goodbye!", OUTPUT_INDENT);
                        break;
                    }
                    Ok(CommandResult::Error(msg)) => {
                        eprintln!("{}Error: {}", OUTPUT_INDENT, msg);
                    }
                    Err(e) => {
                        eprintln!("{}Error: {}", OUTPUT_INDENT, e);
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
                eprintln!("{}Error: Invalid input syntax", OUTPUT_INDENT);
            }
            ReadCommandOutput::ReedlineError(e) => {
                eprintln!("{}Input error: {}", OUTPUT_INDENT, e);
            }
            ReadCommandOutput::CtrlC => {
                // println!("Goodbye!");
                // break;
            }
            ReadCommandOutput::CtrlD => {
                // Exit on Ctrl+D
                println!("{}Goodbye!", OUTPUT_INDENT);
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
