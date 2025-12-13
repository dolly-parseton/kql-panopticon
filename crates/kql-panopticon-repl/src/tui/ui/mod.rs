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

mod ansi_parser;
mod block;
pub mod completion_popup;
mod input_line;
mod output_area;
mod status_bar;
mod syntax_highlight;

use crate::context::SharedContext;
use crate::tui::app::AppState;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};

/// Render the complete UI
pub fn render(frame: &mut Frame, state: &AppState, ctx: &SharedContext) {
    let area = frame.area();

    // Calculate input area height based on continuation lines
    let input_height = 1 + state.input.continuation_lines.len() as u16;

    // Create layout: status bar, output area, input line(s)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),            // Status bar
            Constraint::Min(3),               // Output area
            Constraint::Length(input_height), // Input line(s)
        ])
        .split(area);

    // Render each component
    status_bar::render(frame, chunks[0], ctx);
    output_area::render(frame, chunks[1], state);
    input_line::render(frame, chunks[2], state);

    // Render completion popup (on top of other content)
    if let Some(completion) = &state.completion {
        // Calculate cursor position in the input line
        // Prompt is "> " (2 chars), then cursor position in buffer
        let cursor_x = chunks[2].x + 2 + state.input.cursor as u16;
        let popup_area = completion.calculate_area(chunks[2], cursor_x);
        completion.render(frame, popup_area);
    }
}
