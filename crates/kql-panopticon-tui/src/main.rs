//! KQL Panopticon TUI
//!
//! Interactive TUI shell for KQL query execution and pack management.
//!

// Following: https://ratatui.rs/tutorials/json-editor/app/
mod app;
pub mod layout;
mod theme;
mod ui;

use anyhow::Result;
use clap::Parser;

use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::Terminal;
use std::io;
use std::time::Duration;

/// KQL Panopticon TUI - Interactive shell for KQL query execution
#[derive(Parser, Debug)]
#[command(name = "kql-panopticon-tui")]
#[command(about = "Interactive TUI for KQL Panopticon", long_about = None)]
struct Cli {
    /// Additional pack directory or glob pattern (can be specified multiple times).
    /// Always includes ~/.kql-panopticon/packs/*.yaml by default
    #[arg(long = "pack-dir", value_name = "PATTERN")]
    pack_dirs: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // setup terminal
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    // Load settings from disk (or use defaults)
    let settings = app::Settings::load().unwrap_or_default();
    let mut app = app::App::with_settings(settings, cli.pack_dirs);

    // For demo purposes, set some dummy data
    set_dummydata(&mut app);

    let res = run_app(&mut terminal, &mut app);

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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut app::App) -> Result<bool> {
    loop {
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
                    state.toggle_force_black_background();
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
        app::ActiveModal::None => {}
    }

    match app.current_screen {
        app::CurrentScreen::Interpreter => match key.code {
            KeyCode::Esc => app.handle_escape(),
            KeyCode::Enter => {
                if let Some(command) = app.submit_prompt() {
                    handle_command(app, &command);
                }
            }
            _ => {
                // Let TextArea handle all other input
                app.prompt.input(key);
            }
        },
        app::CurrentScreen::Settings => match key.code {
            KeyCode::Esc => app.handle_escape(),
            _ => {}
        },
        app::CurrentScreen::Exiting(_) => {}
    }
}

fn handle_command(app: &mut app::App, command: &str) {
    let command = command.trim();

    // Parse command and args
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    let cmd = parts[0];
    let args = &parts[1..];

    match cmd {
        ":ws" | ":workspace" => {
            app.open_workspace_selector();
        }
        ":theme" => {
            handle_theme_command(app, args);
        }
        ":load" => {
            app.open_pack_loader();
        }
        _ => {
            // Unknown command - create a block
            let block_id = app.blocks.len();
            let mut block = app::InterpreterBlock::new(block_id, command.to_string());
            block.results = format!("Unknown command: {}", command);
            app.blocks.push(block);
        }
    }
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

fn set_dummydata(app: &mut app::App) {
    use kql_panopticon_core::Workspace;

    // Add some dummy workspaces for testing
    let workspaces = vec![
        Workspace {
            workspace_id: "ws-1".to_string(),
            resource_id: "/subscriptions/sub1/resourceGroups/rg1/providers/Microsoft.OperationalInsights/workspaces/prod-sentinel-eastus".to_string(),
            name: "prod-sentinel-eastus".to_string(),
            location: "eastus".to_string(),
            subscription_id: "sub1".to_string(),
            resource_group: "rg1".to_string(),
            tenant_id: "tenant1".to_string(),
            subscription_name: "Production".to_string(),
        },
        Workspace {
            workspace_id: "ws-2".to_string(),
            resource_id: "/subscriptions/sub1/resourceGroups/rg1/providers/Microsoft.OperationalInsights/workspaces/prod-sentinel-westus".to_string(),
            name: "prod-sentinel-westus".to_string(),
            location: "westus".to_string(),
            subscription_id: "sub1".to_string(),
            resource_group: "rg1".to_string(),
            tenant_id: "tenant1".to_string(),
            subscription_name: "Production".to_string(),
        },
        Workspace {
            workspace_id: "ws-3".to_string(),
            resource_id: "/subscriptions/sub1/resourceGroups/rg2/providers/Microsoft.OperationalInsights/workspaces/prod-logs-analytics".to_string(),
            name: "prod-logs-analytics".to_string(),
            location: "eastus".to_string(),
            subscription_id: "sub1".to_string(),
            resource_group: "rg2".to_string(),
            tenant_id: "tenant1".to_string(),
            subscription_name: "Production".to_string(),
        },
        Workspace {
            workspace_id: "ws-4".to_string(),
            resource_id: "/subscriptions/sub2/resourceGroups/rg3/providers/Microsoft.OperationalInsights/workspaces/dev-sentinel-test".to_string(),
            name: "dev-sentinel-test".to_string(),
            location: "eastus".to_string(),
            subscription_id: "sub2".to_string(),
            resource_group: "rg3".to_string(),
            tenant_id: "tenant1".to_string(),
            subscription_name: "Development".to_string(),
        },
        Workspace {
            workspace_id: "ws-5".to_string(),
            resource_id: "/subscriptions/sub2/resourceGroups/rg3/providers/Microsoft.OperationalInsights/workspaces/dev-logs-sandbox".to_string(),
            name: "dev-logs-sandbox".to_string(),
            location: "westus".to_string(),
            subscription_id: "sub2".to_string(),
            resource_group: "rg3".to_string(),
            tenant_id: "tenant1".to_string(),
            subscription_name: "Development".to_string(),
        },
    ];
    app.workspaces = workspaces;

    // Add a sample block
    let mut block = app::InterpreterBlock::new(0, "Type :ws to select workspaces".to_string());
    block.results = "Hint: Use :ws or :workspace command".to_string();
    app.blocks.push(block);
}
