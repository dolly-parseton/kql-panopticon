//! Output area rendering
//!
//! Scrollable area containing output blocks.

use crate::tui::app::{AppState, Focus, OutputBlock};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};

use super::block::render_block_content;

/// Render the output area with all blocks
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    // Border for output area
    let block = Block::default()
        .borders(Borders::NONE);

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if state.output.is_empty() {
        // Empty state
        let empty = Paragraph::new("  No output yet. Type a command to begin.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner_area);
        return;
    }

    // Calculate total content height and build rendered lines
    let mut all_lines: Vec<(Line, Option<usize>)> = Vec::new(); // (line, block_index)

    for (idx, output_block) in state.output.iter().enumerate() {
        let is_focused = matches!(state.focus, Focus::Block(id) if id == output_block.id);
        let block_lines = render_block_content(output_block, is_focused, inner_area.width);

        for line in block_lines {
            all_lines.push((line, Some(idx)));
        }

        // Add spacing between blocks
        all_lines.push((Line::default(), None));
    }

    // Calculate visible range based on scroll offset
    let visible_height = inner_area.height as usize;
    let total_lines = all_lines.len();

    // scroll_offset = 0 means viewing bottom
    // scroll_offset = N means viewing N lines from bottom
    let start_line = if total_lines > visible_height {
        total_lines - visible_height - state.scroll_offset.min(total_lines - visible_height)
    } else {
        0
    };
    let end_line = (start_line + visible_height).min(total_lines);

    // Get visible lines
    let visible_lines: Vec<Line> = all_lines[start_line..end_line]
        .iter()
        .map(|(line, _)| line.clone())
        .collect();

    let paragraph = Paragraph::new(visible_lines);
    frame.render_widget(paragraph, inner_area);

    // Render scrollbar if content exceeds visible area
    if total_lines > visible_height {
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::new(total_lines)
            .position(start_line);

        frame.render_stateful_widget(
            scrollbar,
            area,
            &mut scrollbar_state,
        );
    }
}
