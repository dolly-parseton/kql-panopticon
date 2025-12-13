//! Output block rendering
//!
//! Renders individual output blocks with content and controls.

use crate::tui::app::{BlockContent, OutputBlock};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Control button styles
struct ControlStyle {
    minimize: &'static str,
    remove: &'static str,
    style: Style,
}

impl ControlStyle {
    fn focused() -> Self {
        Self {
            minimize: " [−] ",
            remove: "[×]",
            style: Style::default().fg(Color::Yellow),
        }
    }

    fn unfocused() -> Self {
        Self {
            minimize: " [−] ",
            remove: "[×]",
            style: Style::default().fg(Color::DarkGray),
        }
    }

    fn focused_minimized() -> Self {
        Self {
            minimize: " [+] ",
            remove: "[×]",
            style: Style::default().fg(Color::Yellow),
        }
    }

    fn unfocused_minimized() -> Self {
        Self {
            minimize: " [+] ",
            remove: "[×]",
            style: Style::default().fg(Color::DarkGray),
        }
    }
}

/// Render a block's content to lines
pub fn render_block_content(block: &OutputBlock, is_focused: bool, max_width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Determine styles based on content type
    let (prefix, prefix_style, content_style) = match &block.content {
        BlockContent::Command(_) => (
            "> ",
            Style::default().fg(Color::Cyan),
            Style::default().fg(Color::White),
        ),
        BlockContent::Text(_) => (
            "  ",
            Style::default(),
            Style::default().fg(Color::Gray),
        ),
        BlockContent::Error(_) => (
            "! ",
            Style::default().fg(Color::Red),
            Style::default().fg(Color::Red),
        ),
        BlockContent::System(_) => (
            "  ",
            Style::default(),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        ),
    };

    // Focus indicator style
    let focus_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Get content text
    let content = match &block.content {
        BlockContent::Command(s) => s.clone(),
        BlockContent::Text(s) => s.clone(),
        BlockContent::Error(s) => s.clone(),
        BlockContent::System(s) => s.clone(),
    };

    let content_lines: Vec<&str> = content.lines().collect();
    let line_count = content_lines.len();

    // If minimized, show single line summary with line count
    if block.minimized {
        let summary = content_lines.first().copied().unwrap_or("(empty)");

        // Calculate available width for summary
        // Format: "▶ [+] > summary... (N lines) [×]"
        let fixed_width = 4 + 5 + 2 + 12 + 3; // focus + expand + prefix + line count + remove
        let available = (max_width as usize).saturating_sub(fixed_width);

        let truncated = if summary.len() > available {
            format!("{}...", &summary[..available.saturating_sub(3)])
        } else {
            summary.to_string()
        };

        let focus_indicator = if is_focused { "▶ " } else { "  " };
        let controls = if is_focused {
            ControlStyle::focused_minimized()
        } else {
            ControlStyle::unfocused_minimized()
        };

        let line_info = if line_count > 1 {
            format!(" ({} lines)", line_count)
        } else {
            String::new()
        };

        lines.push(Line::from(vec![
            Span::styled(focus_indicator, focus_style),
            Span::styled(controls.minimize, controls.style),
            Span::styled(prefix, prefix_style.add_modifier(Modifier::DIM)),
            Span::styled(truncated, content_style.add_modifier(Modifier::DIM)),
            Span::styled(line_info, Style::default().fg(Color::DarkGray)),
            Span::styled(" ", Style::default()),
            Span::styled(controls.remove, controls.style),
        ]));

        return lines;
    }

    // Full render - first line with controls
    let controls = if is_focused {
        ControlStyle::focused()
    } else {
        ControlStyle::unfocused()
    };

    for (i, line_text) in content_lines.iter().enumerate() {
        let focus_indicator = if is_focused && i == 0 { "▶ " } else { "  " };
        let line_prefix = if i == 0 { prefix } else { "  " };

        let mut spans = vec![
            Span::styled(focus_indicator, focus_style),
        ];

        // First line gets controls on the left
        if i == 0 {
            spans.push(Span::styled(controls.minimize, controls.style));
        } else {
            spans.push(Span::styled("     ", Style::default())); // Spacer to align content
        }

        spans.push(Span::styled(line_prefix, prefix_style));
        spans.push(Span::styled(line_text.to_string(), content_style));

        // First line gets remove button on the right
        if i == 0 {
            spans.push(Span::styled(" ", Style::default()));
            spans.push(Span::styled(controls.remove, controls.style));
        }

        lines.push(Line::from(spans));
    }

    // If no content, show placeholder
    if lines.is_empty() {
        let focus_indicator = if is_focused { "▶ " } else { "  " };
        lines.push(Line::from(vec![
            Span::styled(focus_indicator, focus_style),
            Span::styled(controls.minimize, controls.style),
            Span::styled(prefix, prefix_style),
            Span::styled("(empty)", Style::default().fg(Color::DarkGray)),
            Span::styled(" ", Style::default()),
            Span::styled(controls.remove, controls.style),
        ]));
    }

    lines
}

/// Get the border style for a block based on focus state
pub fn block_border_style(is_focused: bool) -> Style {
    if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

/// Get the border type character for focused/unfocused blocks
pub fn block_border_char(is_focused: bool) -> char {
    if is_focused {
        '│'
    } else {
        '│'
    }
}
