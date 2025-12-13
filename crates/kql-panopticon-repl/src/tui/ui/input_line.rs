//! Input line rendering
//!
//! Shows the command prompt and current input with cursor.

use crate::tui::app::{AppState, Focus};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Render the input line
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let is_focused = matches!(state.focus, Focus::Input);

    // Build prompt
    let prompt_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let prompt = Span::styled("> ", prompt_style);

    // Build input text with cursor
    let input = &state.input.buffer;
    let cursor_pos = state.input.cursor;

    let spans = if is_focused {
        // Show cursor
        let before_cursor = &input[..cursor_pos.min(input.len())];
        let cursor_char = input.chars().nth(cursor_pos).map(|c| c.to_string()).unwrap_or_else(|| " ".to_string());
        let after_cursor = if cursor_pos < input.len() {
            &input[cursor_pos + cursor_char.len()..]
        } else {
            ""
        };

        vec![
            prompt,
            Span::raw(before_cursor.to_string()),
            Span::styled(
                cursor_char,
                Style::default().bg(Color::White).fg(Color::Black),
            ),
            Span::raw(after_cursor.to_string()),
        ]
    } else {
        // Dimmed when not focused
        vec![
            prompt,
            Span::styled(
                input.clone(),
                Style::default().fg(Color::DarkGray),
            ),
        ]
    };

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);

    frame.render_widget(paragraph, area);
}
