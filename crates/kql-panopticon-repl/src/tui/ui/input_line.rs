//! Input line rendering
//!
//! Shows the command prompt and current input with cursor.
//! Supports multi-line display during line continuation.
//! Includes syntax highlighting for commands.

use super::syntax_highlight;
use crate::tui::app::{AppState, Focus};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Continuation line prompt character
const CONTINUATION_PROMPT: &str = "· ";

/// Render the input line(s)
pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let is_focused = matches!(state.focus, Focus::Input);
    let continuation_lines = &state.input.continuation_lines;

    let mut lines: Vec<Line> = Vec::new();

    // Style definitions
    let primary_prompt_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let continuation_prompt_style = if is_focused {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let text_style = if is_focused {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let backslash_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Render continuation lines (previously entered) with syntax highlighting
    for (i, line_content) in continuation_lines.iter().enumerate() {
        let prompt = if i == 0 {
            Span::styled("> ", primary_prompt_style)
        } else {
            Span::styled(CONTINUATION_PROMPT, continuation_prompt_style)
        };

        // Split the line to highlight trailing backslash
        let (content, has_backslash) = if line_content.trim_end().ends_with('\\') {
            let trimmed = line_content.trim_end();
            let without_backslash = &trimmed[..trimmed.len() - 1];
            (without_backslash.to_string(), true)
        } else {
            (line_content.clone(), false)
        };

        let mut spans = vec![prompt];

        // Apply syntax highlighting to continuation line content
        if is_focused {
            let highlighted = syntax_highlight::highlight(&content);
            for token in highlighted {
                spans.push(Span::styled(token.text.clone(), token.kind.style()));
            }
        } else {
            spans.push(Span::styled(content, text_style));
        }

        if has_backslash {
            spans.push(Span::styled(" \\", backslash_style));
        }

        lines.push(Line::from(spans));
    }

    // Render current input line (with cursor if focused)
    let current_prompt = if continuation_lines.is_empty() {
        Span::styled("> ", primary_prompt_style)
    } else {
        Span::styled(CONTINUATION_PROMPT, continuation_prompt_style)
    };

    let input = &state.input.buffer;
    let cursor_pos = state.input.cursor;

    let current_line = if is_focused {
        // Apply syntax highlighting with cursor
        let mut spans = vec![current_prompt];
        spans.extend(highlight_with_cursor(input, cursor_pos));
        Line::from(spans)
    } else {
        // Dimmed when not focused
        Line::from(vec![
            current_prompt,
            Span::styled(input.clone(), Style::default().fg(Color::DarkGray)),
        ])
    };

    lines.push(current_line);

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Apply syntax highlighting to input and insert cursor at the given position
fn highlight_with_cursor(input: &str, cursor_pos: usize) -> Vec<Span<'static>> {
    if input.is_empty() {
        // Just show cursor on empty input
        return vec![Span::styled(
            " ".to_string(),
            Style::default().bg(Color::White).fg(Color::Black),
        )];
    }

    let tokens = syntax_highlight::highlight(input);
    let mut spans = Vec::new();
    let mut char_offset = 0;

    for token in tokens {
        let token_start = char_offset;
        let token_end = char_offset + token.text.len();

        if cursor_pos >= token_start && cursor_pos < token_end {
            // Cursor is within this token - split it
            let relative_pos = cursor_pos - token_start;
            let before = &token.text[..relative_pos];
            let cursor_char = token
                .text
                .chars()
                .nth(relative_pos)
                .map(|c| c.to_string())
                .unwrap_or_else(|| " ".to_string());
            let after_start = relative_pos + cursor_char.len();
            let after = if after_start < token.text.len() {
                &token.text[after_start..]
            } else {
                ""
            };

            if !before.is_empty() {
                spans.push(Span::styled(before.to_string(), token.kind.style()));
            }
            spans.push(Span::styled(
                cursor_char,
                Style::default().bg(Color::White).fg(Color::Black),
            ));
            if !after.is_empty() {
                spans.push(Span::styled(after.to_string(), token.kind.style()));
            }
        } else {
            // Cursor not in this token
            spans.push(Span::styled(token.text.clone(), token.kind.style()));
        }

        char_offset = token_end;
    }

    // Cursor at end of input
    if cursor_pos >= char_offset {
        spans.push(Span::styled(
            " ".to_string(),
            Style::default().bg(Color::White).fg(Color::Black),
        ));
    }

    spans
}
