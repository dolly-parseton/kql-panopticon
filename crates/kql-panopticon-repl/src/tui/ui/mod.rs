//! UI rendering for the TUI shell
//!
//! Layout:
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │ Status Bar (1 line)                             │
//! ├─────────────────────────────────────────────────┤
//! │                                                 │
//! │ Output Area (flexible height)                   │
//! │                                                 │
//! ├─────────────────────────────────────────────────┤
//! │ > Input Line (1 line)                           │
//! └─────────────────────────────────────────────────┘
//! ```

mod block;
mod input_line;
mod output_area;
mod status_bar;

use crate::context::SharedContext;
use crate::tui::app::AppState;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};

/// Render the complete UI
pub fn render(frame: &mut Frame, state: &AppState, ctx: &SharedContext) {
    let area = frame.area();

    // Create layout: status bar, output area, input line
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // Status bar
            Constraint::Min(3),     // Output area
            Constraint::Length(1),  // Input line
        ])
        .split(area);

    // Render each component
    status_bar::render(frame, chunks[0], ctx);
    output_area::render(frame, chunks[1], state);
    input_line::render(frame, chunks[2], state);
}
