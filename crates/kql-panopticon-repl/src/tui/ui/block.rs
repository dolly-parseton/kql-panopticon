//! Output block rendering
//!
//! Renders individual output blocks with content and controls.

use crate::tui::app::{BlockContent, OutputBlock};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Render a block's content to lines
pub fn render_block_content(block: &OutputBlock, is_focused: bool, width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Determine styles based on content type and focus
    let (prefix, prefix_style, content_style) = match &block.content {
        BlockContent::Command(_) => (
            "> ",
            Style::default().fg(Color::Cyan),
            Style::default().fg(Color::White),
        ),
        BlockContent::Text(_) => (
            "  ",
            Style::default(),
            Style::default().fg(Color::White),
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

    // Get content text
    let content = match &block.content {
        BlockContent::Command(s) => s.clone(),
        BlockContent::Text(s) => s.clone(),
        BlockContent::Error(s) => s.clone(),
        BlockContent::System(s) => s.clone(),
    };

    // If minimized, show single line summary
    if block.minimized {
        let summary = content.lines().next().unwrap_or("(empty)");
        let truncated = if summary.len() > 40 {
            format!("{}...", &summary[..40])
        } else {
            summary.to_string()
        };

        let focus_indicator = if is_focused { "▶ " } else { "  " };
        let controls = if is_focused { " [+] [×]" } else { "" };

        lines.push(Line::from(vec![
            Span::styled(focus_indicator, Style::default().fg(Color::Yellow)),
            Span::styled("[−] ", Style::default().fg(Color::DarkGray)),
            Span::styled(prefix, prefix_style),
            Span::styled(truncated, content_style.add_modifier(Modifier::DIM)),
            Span::styled(controls, Style::default().fg(Color::DarkGray)),
        ]));

        return lines;
    }

    // Full render
    let content_lines: Vec<&str> = content.lines().collect();

    for (i, line_text) in content_lines.iter().enumerate() {
        let focus_indicator = if is_focused && i == 0 { "▶ " } else { "  " };
        let line_prefix = if i == 0 { prefix } else { "  " };

        // Add controls to first line when focused
        let controls = if is_focused && i == 0 {
            " [−] [×]"
        } else {
            ""
        };

        let mut spans = vec![
            Span::styled(focus_indicator, Style::default().fg(Color::Yellow)),
            Span::styled(line_prefix, prefix_style),
            Span::styled(line_text.to_string(), content_style),
        ];

        if !controls.is_empty() {
            spans.push(Span::styled(controls, Style::default().fg(Color::DarkGray)));
        }

        lines.push(Line::from(spans));
    }

    // If no content, show placeholder
    if lines.is_empty() {
        let focus_indicator = if is_focused { "▶ " } else { "  " };
        lines.push(Line::from(vec![
            Span::styled(focus_indicator, Style::default().fg(Color::Yellow)),
            Span::styled(prefix, prefix_style),
            Span::styled("(empty)", Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines
}
