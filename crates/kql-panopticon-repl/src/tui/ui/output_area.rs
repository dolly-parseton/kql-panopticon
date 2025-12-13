//! Output area rendering
//!
//! Scrollable area containing output blocks.

use crate::tui::app::{AppState, BlockContent, Focus};
use crate::tui::widgets::Widget;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};

use super::block::render_block_content;

/// Render the output area with all blocks
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    // Clear the entire output area first to prevent rendering artifacts
    // when widgets are removed or content shrinks
    frame.render_widget(Clear, area);

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

    // Check if there's an active widget that needs special rendering
    let active_widget_id = match &state.focus {
        Focus::Widget(id) => Some(*id),
        _ => None,
    };

    // Calculate total content height and build rendered lines
    let mut all_lines: Vec<(Line, Option<usize>)> = Vec::new(); // (line, block_index)
    let mut widget_block_idx: Option<usize> = None;
    let mut widget_start_line: usize = 0;

    for (idx, output_block) in state.output.iter().enumerate() {
        let is_block_focused = matches!(state.focus, Focus::Block(id) if id == output_block.id);
        let is_widget_focused = active_widget_id == Some(output_block.id);

        // For active widgets, we'll render them separately with full interactivity
        if is_widget_focused {
            widget_block_idx = Some(idx);
            widget_start_line = all_lines.len();

            // Add placeholder lines for the widget's height
            let widget_height = match &output_block.content {
                BlockContent::Editor(ref editor) => editor.height() as usize,
                BlockContent::Results(ref results) => results.height() as usize,
                BlockContent::InputForm(ref form) => form.height() as usize,
                BlockContent::InputDef(ref def) => def.height() as usize,
                BlockContent::RunProgress(ref progress) => progress.height() as usize,
                BlockContent::WorkspaceSelector(ref selector) => selector.height() as usize,
                _ => 0,
            };
            for _ in 0..widget_height {
                all_lines.push((Line::default(), Some(idx)));
            }
        } else {
            // Render as lines (including non-focused editor widgets)
            let block_lines = render_block_content(output_block, is_block_focused, inner_area.width);

            for line in block_lines {
                all_lines.push((line, Some(idx)));
            }
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

    // Render active widget if visible
    if let Some(widget_idx) = widget_block_idx {
        // Check if widget is in visible range
        if widget_start_line >= start_line && widget_start_line < end_line {
            // Calculate the widget's position within the visible area
            let widget_y = inner_area.y + (widget_start_line - start_line) as u16;

            if let Some(output_block) = state.output.get(widget_idx) {
                match &output_block.content {
                    BlockContent::Editor(ref editor) => {
                        let widget_height = editor.height().min(inner_area.height - (widget_y - inner_area.y));
                        let widget_area = Rect::new(
                            inner_area.x,
                            widget_y,
                            inner_area.width,
                            widget_height,
                        );

                        // Clone editor for rendering (we need mutable access)
                        let mut editor_clone = editor.clone();
                        editor_clone.render(frame, widget_area);
                    }
                    BlockContent::Results(ref results) => {
                        let widget_height = results.height().min(inner_area.height - (widget_y - inner_area.y));
                        let widget_area = Rect::new(
                            inner_area.x,
                            widget_y,
                            inner_area.width,
                            widget_height,
                        );

                        // Clone results for rendering (we need mutable access)
                        let mut results_clone = results.clone();
                        results_clone.render(frame, widget_area);
                    }
                    BlockContent::InputForm(ref form) => {
                        let widget_height = form.height().min(inner_area.height - (widget_y - inner_area.y));
                        let widget_area = Rect::new(
                            inner_area.x,
                            widget_y,
                            inner_area.width,
                            widget_height,
                        );

                        // Clone form for rendering (we need mutable access)
                        let mut form_clone = form.clone();
                        form_clone.render(frame, widget_area);
                    }
                    BlockContent::InputDef(ref def) => {
                        let widget_height = def.height().min(inner_area.height - (widget_y - inner_area.y));
                        let widget_area = Rect::new(
                            inner_area.x,
                            widget_y,
                            inner_area.width,
                            widget_height,
                        );

                        // Clone def for rendering (we need mutable access)
                        let mut def_clone = def.clone();
                        def_clone.render(frame, widget_area);
                    }
                    BlockContent::RunProgress(ref progress) => {
                        let widget_height = progress.height().min(inner_area.height - (widget_y - inner_area.y));
                        let widget_area = Rect::new(
                            inner_area.x,
                            widget_y,
                            inner_area.width,
                            widget_height,
                        );

                        // Clone progress for rendering (we need mutable access)
                        let mut progress_clone = progress.clone();
                        progress_clone.render(frame, widget_area);
                    }
                    BlockContent::WorkspaceSelector(ref selector) => {
                        let widget_height = selector.height().min(inner_area.height - (widget_y - inner_area.y));
                        let widget_area = Rect::new(
                            inner_area.x,
                            widget_y,
                            inner_area.width,
                            widget_height,
                        );

                        // Clone selector for rendering (we need mutable access)
                        let mut selector_clone = selector.clone();
                        selector_clone.render(frame, widget_area);
                    }
                    _ => {}
                }
            }
        }
    }

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
