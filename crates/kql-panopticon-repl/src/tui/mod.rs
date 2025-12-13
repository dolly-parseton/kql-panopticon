//! TUI Shell for KQL Panopticon
//!
//! Full terminal user interface that wraps the interpreter,
//! providing inline widgets, scrollable output, and unified rendering.
//!
//! Enable with `cargo run --features tui`

mod active_job;
mod app;
mod event;
pub mod events;
pub mod ui;
mod undo_stack;
pub mod widgets;

pub use active_job::{ActiveJob, ActiveJobManager, CancellationToken};
pub use undo_stack::{UndoStack, UndoableOperation};

pub use app::App;

use crate::context::create_shared_context;
use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{self, stdout};

/// Run the TUI shell application
pub async fn run() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Clear screen
    terminal.clear()?;

    // Create shared context
    let ctx = create_shared_context();

    // Create and run app
    let mut app = App::new(ctx).await;
    let result = app.run(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    result
}
