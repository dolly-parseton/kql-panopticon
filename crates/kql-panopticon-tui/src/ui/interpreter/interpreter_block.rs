use std::rc::Rc;
use std::str::Utf8Chunks;

use crate::app::{App, CurrentScreen};
use ratatui::layout::{Constraint, Direction, Layout};

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Widget};
use ratatui::Frame;

pub fn render_interpreter(frame: &mut Frame, app: &crate::app::App, chunk: ratatui::layout::Rect) {
    // Outer container with left padding
    let interpreter_block = Block::default()
        .borders(Borders::NONE)
        .padding(Padding::left(1));
    let inner_area = interpreter_block.inner(chunk);
    frame.render_widget(interpreter_block, chunk);

    // Calculate how many lines each block takes (for now assume 1 line per block)
    const LINES_PER_BLOCK: usize = 1;

    let visible_lines = inner_area.height as usize;
    let total_lines = app.blocks.len() * LINES_PER_BLOCK;

    // Determine which blocks are visible
    let start_block = app.ui_state.scroll_offset / LINES_PER_BLOCK;
    let start_line_offset = app.ui_state.scroll_offset % LINES_PER_BLOCK;

    let mut y = 0u16;

    for (i, block) in app.blocks.iter().enumerate().skip(start_block) {
        if y >= inner_area.height {
            break; // No more room
        }

        let block_area = ratatui::layout::Rect {
            x: inner_area.x,
            y: inner_area.y + y,
            width: inner_area.width,
            height: 1, // Single line for now
        };

        let block_content = format!("> {}", block.command);
        let block_para = Paragraph::new(block_content);
        frame.render_widget(block_para, block_area);

        y += LINES_PER_BLOCK as u16;
    }
}
