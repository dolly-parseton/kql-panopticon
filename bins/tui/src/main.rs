//! KQL Panopticon TUI
//!
//! Interactive TUI shell for KQL query execution and pack management.
//!

// Following: https://ratatui.rs/tutorials/json-editor/app/
mod app;
mod completion;
pub mod layout;
mod theme;
mod ui;

use anyhow::Result;
use clap::Parser;
use kql_panopticon::tracing::{tui_channel, TuiLayer};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing_subscriber::prelude::*;

use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::Terminal;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// KQL Panopticon TUI - Interactive shell for KQL query execution
#[derive(Parser, Debug)]
#[command(name = "panopticon-tui")]
#[command(about = "Interactive TUI for KQL Panopticon", long_about = None)]
struct Cli {
    /// Additional pack directory to scan (can be specified multiple times).
    /// Recursively searches for valid packs (both file and folder packs).
    /// Always includes ~/.kql-panopticon/packs by default
    #[arg(long = "pack-dir", value_name = "DIR")]
    pack_dirs: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Set up tracing with TUI layer for log event forwarding
    let (log_tx, log_rx) = tui_channel();
    tracing_subscriber::registry()
        .with(TuiLayer::new(log_tx))
        .init();

    // setup terminal
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    // Load settings from disk (or use defaults)
    let mut settings = app::Settings::load().unwrap_or_default();

    // CLI pack directories override settings (if provided)
    if !cli.pack_dirs.is_empty() {
        settings.pack_directories = cli.pack_dirs;
    }

    let mut app = app::App::with_settings(settings);

    let _res = run_app(&mut terminal, &mut app, log_rx);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    println!("Goodbye!");

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut app::App,
    mut log_rx: UnboundedReceiver<kql_panopticon::tracing::TuiEvent>,
) -> Result<bool> {
    loop {
        // Poll for async results (non-blocking)
        app.poll_connection_results();
        app.poll_pack_execution_results();

        // Poll for log events from tracing layer
        while let Ok(event) = log_rx.try_recv() {
            app.push_log_event(event);
        }

        terminal.draw(|f| ui::ui(f, app))?;

        // Non-blocking poll with 50ms timeout
        // Allows loop to continue for async updates (progress channels, etc.)
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Release {
                    continue;
                }
                handle_key_event(app, key);
            }
        }

        if app.should_exit() {
            break;
        }
    }
    Ok(true)
}

fn handle_key_event(app: &mut app::App, key: KeyEvent) {
    // Modal takes priority over all other input
    match app.active_modal {
        app::ActiveModal::WorkspaceSelector(ref mut state) => {
            match key.code {
                KeyCode::Esc => app.cancel_workspace_selection(),
                KeyCode::Enter => app.confirm_workspace_selection(),
                KeyCode::Up => state.move_up(),
                KeyCode::Down => state.move_down(),
                KeyCode::Left => state.collapse(),
                KeyCode::Right => state.expand(),
                KeyCode::Char(' ') => state.toggle_selection(),
                KeyCode::Char('a') => state.select_all(),
                KeyCode::Char('n') => state.select_none(),
                _ => {}
            }
            return;
        }
        app::ActiveModal::ThemeSelector(ref mut state) => {
            match key.code {
                KeyCode::Esc => app.cancel_theme_selection(),
                KeyCode::Enter => app.confirm_theme_selection(),
                KeyCode::Up => {
                    state.move_up();
                    app.preview_selected_theme();
                }
                KeyCode::Down => {
                    state.move_down();
                    app.preview_selected_theme();
                }
                KeyCode::Char(' ') => {
                    state.toggle_override_terminal_background();
                    app.preview_selected_theme();
                }
                _ => {}
            }
            return;
        }
        app::ActiveModal::PackLoader(ref mut state) => {
            match key.code {
                KeyCode::Esc => app.cancel_pack_selection(),
                KeyCode::Enter => app.confirm_pack_selection(),
                KeyCode::Up => state.move_up(),
                KeyCode::Down => state.move_down(),
                _ => {}
            }
            return;
        }
        app::ActiveModal::SettingsConfirmation => {
            match key.code {
                KeyCode::Char('s') | KeyCode::Char('S') => app.save_settings_and_exit(),
                KeyCode::Char('q') | KeyCode::Char('Q') => app.quit_without_saving(),
                KeyCode::Esc => app.active_modal = app::ActiveModal::None,
                _ => {}
            }
            return;
        }
        app::ActiveModal::Logs(ref mut state) => {
            match key.code {
                KeyCode::Esc => app.cancel_logs_modal(),
                KeyCode::Up | KeyCode::Char('k') => state.move_up(),
                KeyCode::Down | KeyCode::Char('j') => state.move_down(),
                KeyCode::PageUp => state.page_up(20),
                KeyCode::PageDown => state.page_down(20),
                KeyCode::Home | KeyCode::Char('g') => state.scroll_to_top(),
                KeyCode::End | KeyCode::Char('G') => state.scroll_to_bottom(),
                KeyCode::Char('l') => state.cycle_level_filter(),
                KeyCode::Char('c') => {
                    state.clear_logs();
                    app.execution_logs.clear();
                }
                _ => {}
            }
            return;
        }
        app::ActiveModal::None => {}
    }

    match app.current_screen {
        app::CurrentScreen::Interpreter => {
            // Handle scroll controls first (Ctrl+j/k, PageUp/PageDown)
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

            match (key.code, ctrl) {
                // Scroll controls
                (KeyCode::Char('j'), true) | (KeyCode::Down, true) => {
                    app.scroll_down(1);
                }
                (KeyCode::Char('k'), true) | (KeyCode::Up, true) => {
                    app.scroll_up(1);
                }
                (KeyCode::PageDown, _) => {
                    app.scroll_page_down();
                }
                (KeyCode::PageUp, _) => {
                    app.scroll_page_up();
                }
                // Tab completion
                (KeyCode::Tab, false) => {
                    if app.has_completion() {
                        app.handle_completion_tab();
                    } else {
                        app.trigger_completion();
                    }
                }
                (KeyCode::BackTab, false) => {
                    // Shift+Tab to cycle backwards
                    if app.has_completion() {
                        app.completion_prev();
                    }
                }
                // Standard controls
                (KeyCode::Esc, _) => {
                    if app.has_completion() {
                        app.dismiss_completion();
                    } else {
                        app.handle_escape();
                    }
                }
                (KeyCode::Enter, _) => {
                    if app.has_completion() {
                        app.accept_completion();
                    } else if let Some(command) = app.submit_prompt() {
                        handle_command(app, &command);
                    }
                }
                // History navigation (non-Ctrl Up/Down)
                (KeyCode::Up, false) => {
                    if app.has_completion() {
                        app.completion_prev();
                    } else {
                        app.history_back();
                    }
                }
                (KeyCode::Down, false) => {
                    if app.has_completion() {
                        app.completion_next();
                    } else {
                        app.history_forward();
                    }
                }
                _ => {
                    // Dismiss completion on any other input
                    if app.has_completion() {
                        app.dismiss_completion();
                    }
                    // Reset history navigation on any edit
                    if app.is_navigating_history() {
                        app.history_reset();
                    }
                    // Let TextArea handle all other input
                    app.prompt.input(key);
                }
            }
        }
        app::CurrentScreen::Settings => match key.code {
            KeyCode::Esc => app.handle_escape(),
            _ => {}
        },
        app::CurrentScreen::Exiting(_) => {}
        app::CurrentScreen::Editor => todo!("Not implemented yet"),
    }
}

fn handle_command(app: &mut app::App, command: &str) {
    let command = command.trim();

    // Check for let statement: let {name}: input = {value}
    if command.starts_with("let ") {
        handle_let_statement(app, command);
        return;
    }

    // Parse command and args
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    let cmd = parts[0];
    let args = &parts[1..];

    match cmd {
        ":ws" | ":workspace" => {
            handle_workspace_command(app, command);
        }
        ":theme" => {
            handle_theme_command(app, args);
        }
        ":load" => {
            app.open_pack_loader();
        }
        ":logs" => {
            app.open_logs_modal();
        }
        ":run" => {
            handle_run_command(app, command);
        }
        _ => {
            // Unknown command - create a block with failed status
            let block_id = app.blocks.len();
            let mut block = app::InterpreterBlock::new(block_id, command.to_string());
            block.results = format!("Unknown command: {}", command);
            block.status = app::BlockStatus::Failed;
            app.blocks.push(block);
        }
    }
}

fn handle_workspace_command(app: &mut app::App, command: &str) {
    match &app.connection_status {
        app::ConnectionStatus::Ready { .. } => {
            app.open_workspace_selector();
        }
        app::ConnectionStatus::Authenticating => {
            let block_id = app.blocks.len();
            let mut block = app::InterpreterBlock::new(block_id, command.to_string());
            block.results = "Authenticating with Azure... please wait.".to_string();
            block.status = app::BlockStatus::Failed;
            app.blocks.push(block);
        }
        app::ConnectionStatus::Discovering => {
            let block_id = app.blocks.len();
            let mut block = app::InterpreterBlock::new(block_id, command.to_string());
            block.results = "Discovering workspaces... please wait.".to_string();
            block.status = app::BlockStatus::Failed;
            app.blocks.push(block);
        }
        app::ConnectionStatus::Error { message } => {
            let block_id = app.blocks.len();
            let mut block = app::InterpreterBlock::new(block_id, command.to_string());
            block.results = format!(
                "Cannot select workspaces: {}\n\nPlease run 'az login' and restart the application.",
                message
            );
            block.status = app::BlockStatus::Failed;
            app.blocks.push(block);
        }
    }
}

/// Handle let statement: let {name}: input = {value}
fn handle_let_statement(app: &mut app::App, command: &str) {
    let block_id = app.blocks.len();
    let mut block = app::InterpreterBlock::new(block_id, command.to_string());

    // Parse: "let {name}: input = {value}"
    // Strip "let " prefix
    let rest = &command[4..];

    // Find the colon separating name from type
    let Some(colon_pos) = rest.find(':') else {
        block.results =
            "Syntax error: expected ':' after variable name\nUsage: let {name}: input = {value}"
                .to_string();
        block.status = app::BlockStatus::Failed;
        app.blocks.push(block);
        return;
    };

    let name = rest[..colon_pos].trim();

    // Validate name is a valid identifier
    if name.is_empty() || !is_valid_identifier(name) {
        block.results = format!("Invalid variable name: '{}'\nVariable names must start with a letter or underscore and contain only alphanumeric characters and underscores", name);
        block.status = app::BlockStatus::Failed;
        app.blocks.push(block);
        return;
    }

    // Parse type and value: "input = {value}"
    let type_and_value = rest[colon_pos + 1..].trim();

    // Find the equals sign
    let Some(eq_pos) = type_and_value.find('=') else {
        block.results =
            "Syntax error: expected '=' after type\nUsage: let {name}: input = {value}".to_string();
        block.status = app::BlockStatus::Failed;
        app.blocks.push(block);
        return;
    };

    let type_name = type_and_value[..eq_pos].trim();
    let raw_value = type_and_value[eq_pos + 1..].trim();

    // Currently only "input" type is supported
    if type_name != "input" {
        block.results = format!("Unknown type: '{}'\nSupported types: input", type_name);
        block.status = app::BlockStatus::Failed;
        app.blocks.push(block);
        return;
    }

    // Parse and validate the input value
    let value = match kql_panopticon::pack::parse_input_value(raw_value) {
        Ok(v) => v,
        Err(e) => {
            block.results = format!("Invalid input value: {}\n\nSupported formats:\n  - Array: [\"item1\", \"item2\"]\n  - Quoted string: \"value\" or 'value'\n  - Raw value: value", e);
            block.status = app::BlockStatus::Failed;
            app.blocks.push(block);
            return;
        }
    };

    // Validate against loaded pack if available
    if let Some(loaded_pack) = &app.loaded_pack {
        // Check if this input is defined in the pack
        let input_def = loaded_pack
            .pack
            .acquisition
            .inputs
            .iter()
            .find(|input| input.name == name);

        match input_def {
            Some(def) => {
                // Input is recognized - could add type/format validation here
                log::debug!(
                    "Input '{}' matches pack definition: {}",
                    name,
                    def.description.as_deref().unwrap_or("no description")
                );
            }
            None => {
                // Warn about unrecognized input
                let known_inputs: Vec<_> = loaded_pack
                    .pack
                    .acquisition
                    .inputs
                    .iter()
                    .map(|i| i.name.as_str())
                    .collect();

                let warning = if known_inputs.is_empty() {
                    format!(
                        "Warning: Pack '{}' has no defined inputs.\nThis input will be ignored during execution.",
                        loaded_pack.pack.name
                    )
                } else {
                    format!(
                        "Warning: Input '{}' is not defined in pack '{}'.\nKnown inputs: {}\n\nThis input will be ignored during execution.",
                        name,
                        loaded_pack.pack.name,
                        known_inputs.join(", ")
                    )
                };

                log::warn!(
                    "Unrecognized input '{}' for pack '{}'",
                    name,
                    loaded_pack.pack.name
                );
                block.results = warning;
                block.status = app::BlockStatus::Failed;
                app.blocks.push(block);
                return;
            }
        }
    } else {
        // No pack loaded - accept input but warn
        log::debug!("Input '{}' defined without loaded pack", name);
    }

    // Store the input
    let is_update = app.inputs.contains_key(name);
    app.inputs.insert(name.to_string(), value.clone());

    // Create success block with helpful display
    let action = if is_update { "Updated" } else { "Set" };
    let display_value = if raw_value != value {
        format!("{} → {}", raw_value, value)
    } else {
        value
    };
    block.results = format!("{} input '{}' = {}", action, name, display_value);
    block.status = app::BlockStatus::Completed;
    app.blocks.push(block);
}

/// Check if a string is a valid identifier (alphanumeric + underscore, not starting with digit)
fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

fn handle_theme_command(app: &mut app::App, args: &[&str]) {
    if args.is_empty() {
        // Open theme selector modal
        app.open_theme_selector();
    } else {
        // Switch theme directly
        let block_id = app.blocks.len();
        let mut block = app::InterpreterBlock::new(block_id, ":theme".to_string());

        let theme_name = args[0];
        match app.set_theme(theme_name) {
            Ok(()) => {
                block.results = format!("Switched to theme: {}", theme_name);
                block.status = app::BlockStatus::Completed;
            }
            Err(e) => {
                block.results = e;
                block.status = app::BlockStatus::Failed;
            }
        }

        app.blocks.push(block);
    }
}

/// Pre-flight validation for pack execution
fn validate_pack_execution(app: &app::App) -> Result<(), String> {
    // Check for concurrent execution
    if app.active_execution_block_id.is_some() {
        return Err(
            "A pack execution is already in progress. Please wait for it to complete.".to_string(),
        );
    }

    // Check Azure client ready
    if app.client.is_none() || !app.is_connection_ready() {
        return Err(
            "Azure connection not ready. Please wait for authentication to complete.".to_string(),
        );
    }

    // Check pack loaded
    let loaded_pack = app
        .loaded_pack
        .as_ref()
        .ok_or_else(|| "No pack loaded. Use ':load' to select a pack.".to_string())?;

    // Check workspaces selected
    if app.selected_workspace_ids.is_empty() {
        return Err("No workspaces selected. Use ':ws' to select workspaces.".to_string());
    }

    // Get all required inputs (required=true AND no default)
    let required_inputs = loaded_pack.pack.required_inputs();

    log::info!(
        "Pack '{}' has {} required inputs: {:?}",
        loaded_pack.pack.name,
        required_inputs.len(),
        required_inputs.iter().map(|i| &i.name).collect::<Vec<_>>()
    );

    // Check which required inputs are missing
    let missing: Vec<_> = required_inputs
        .iter()
        .filter(|input| {
            let has_input = app.inputs.contains_key(&input.name);
            log::debug!(
                "Checking input '{}': required={}, has_value={}",
                input.name,
                input.required,
                has_input
            );
            !has_input
        })
        .collect();

    if !missing.is_empty() {
        let missing_names: Vec<_> = missing.iter().map(|i| i.name.as_str()).collect();
        let examples: Vec<_> = missing
            .iter()
            .map(|input| {
                format!(
                    "  let {}: input = {}  # {}",
                    input.name,
                    input.example.as_deref().unwrap_or("<value>"),
                    input.description.as_deref().unwrap_or("No description")
                )
            })
            .collect();

        return Err(format!(
            "Missing required inputs: {}\n\nDefine using 'let' statements:\n{}",
            missing_names.join(", "),
            examples.join("\n")
        ));
    }

    log::info!("Pre-flight validation passed");
    Ok(())
}

fn handle_run_command(app: &mut app::App, command: &str) {
    let block_id = app.blocks.len();
    let mut block = app::InterpreterBlock::new(block_id, command.to_string());

    // Validate prerequisites
    match validate_pack_execution(app) {
        Ok(()) => {
            // Extract state (validation guarantees these exist)
            let loaded_pack = app.loaded_pack.as_ref().unwrap();
            let client = app.client.as_ref().unwrap().clone();

            // Build full input map: pack defaults + user inputs
            let mut inputs = std::collections::HashMap::new();

            // Apply defaults first
            for input_def in &loaded_pack.pack.acquisition.inputs {
                if let Some(ref default) = input_def.default {
                    inputs.insert(input_def.name.clone(), default.clone());
                }
            }

            // Override with user inputs
            for (key, value) in &app.inputs {
                inputs.insert(key.clone(), value.clone());
            }

            // Filter workspaces by selected IDs
            let selected_workspaces: Vec<_> = app
                .workspaces
                .iter()
                .filter(|ws| app.selected_workspace_ids.contains(&ws.workspace_id))
                .cloned()
                .collect();

            // Log execution details
            log::info!(
                "Starting pack execution: {} across {} workspaces with {} inputs",
                loaded_pack.pack.name,
                selected_workspaces.len(),
                inputs.len()
            );

            // Set initial status message
            let initial_msg = format!(
                "Starting pack execution...\n\nPack: {}\nWorkspaces: {}\nSteps: {}\nInputs: {:?}",
                loaded_pack.pack.name,
                selected_workspaces.len(),
                loaded_pack.pack.acquisition.steps.len(),
                inputs.keys().collect::<Vec<_>>()
            );
            block.update_results(initial_msg);

            // Spawn execution task (returns handle and receiver)
            let (task_handle, result_rx) = app::spawn_execution_task(
                client,
                Arc::clone(&loaded_pack.pack), // Cheap Arc clone (8 bytes)
                Some(loaded_pack.path.clone()),
                inputs,
                selected_workspaces,
                Some(PathBuf::from(&app.initial_settings.output_directory)),
            );

            // Store state
            app.pack_execution_rx = Some(result_rx);
            app.active_execution_task = Some(task_handle);
            app.active_execution_block_id = Some(block_id);

            app.blocks.push(block);
        }
        Err(validation_error) => {
            // Validation failed - create failed block
            block.fail(validation_error);
            app.blocks.push(block);
        }
    }
}
