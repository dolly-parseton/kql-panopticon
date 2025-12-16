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
use std::thread::sleep;
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
    // Main app loop
    loop {
        terminal.draw(|f| ui::ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Release {
                // Skip events that are not KeyEventKind::Press
                continue;
            }
            match app.current_screen {
                _ => match key.code {
                    KeyCode::Char('q') => {
                        app.set_exit_screen();
                    }
                    _ => {}
                },
            }
        }

        if let app::CurrentScreen::Exiting(start_time) = app.current_screen {
            if start_time.elapsed() >= Duration::from_millis(100) {
                break;
            }
        }
    }
    Ok(true)
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
