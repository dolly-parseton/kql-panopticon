//! KQL Panopticon TUI
//!
//! Interactive TUI shell for KQL query execution and pack management.
//!

// Following: https://ratatui.rs/tutorials/json-editor/app/
mod app;
pub mod layout;
mod ui;

use anyhow::Result;

use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::Terminal;
use std::io;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // setup terminal
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let mut app = app::App::new();

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
                handle_key_event(app, key.code);
            }
        }

        if app.should_exit() {
            break;
        }
    }
    Ok(true)
}

fn handle_key_event(app: &mut app::App, key: KeyCode) {
    match app.current_screen {
        app::CurrentScreen::Interpreter => match key {
            KeyCode::Char('q') => app.set_exit_screen(),
            _ => {}
        },
        app::CurrentScreen::Settings => match key {
            KeyCode::Char('q') => app.set_exit_screen(),
            _ => {}
        },
        app::CurrentScreen::Exiting(_) => {}
    }
}

fn set_dummydata(app: &mut app::App) {
    for i in 0..50 {
        let mut block = app::InterpreterBlock::new(i, format!("Block {}", i + 1));

        // random dummy results, some multiline
        match i % 5 {
            0 => block.results = "Result: OK".to_string(),
            1 => block.results = "Result:\nLine 1\nLine 2".to_string(),
            2 => block.results = "Result:\nLine 1\nLine 2\nLine 3".to_string(),
            3 => block.results = "Result: ERROR".to_string(),
            _ => block.results = format!("Results for block {}", i + 1),
        }
        app.blocks.push(block);
    }
    app.prompt.buffer = "dummy command".to_string();
}
