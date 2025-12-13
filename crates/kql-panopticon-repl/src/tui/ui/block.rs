//! Output block rendering
//!
//! Renders individual output blocks with content and controls.

use super::ansi_parser;
use super::syntax_highlight;
use crate::tui::app::{BlockContent, OutputBlock};
use crate::tui::widgets::editor::EditorMode;
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
        BlockContent::Editor(editor) => {
            let mode_indicator = match editor.mode {
                EditorMode::Kql => "📝 ",
                EditorMode::Yaml => "📄 ",
                EditorMode::Plain => "📃 ",
            };
            (
                mode_indicator,
                Style::default().fg(Color::Cyan),
                Style::default().fg(Color::Gray),
            )
        }
        BlockContent::Results(_) => (
            "📊 ",
            Style::default().fg(Color::Yellow),
            Style::default().fg(Color::Gray),
        ),
        BlockContent::InputForm(_) => (
            "📝 ",
            Style::default().fg(Color::Magenta),
            Style::default().fg(Color::Gray),
        ),
        BlockContent::InputDef(_) => (
            "⚙ ",
            Style::default().fg(Color::Cyan),
            Style::default().fg(Color::Gray),
        ),
        BlockContent::RunProgress(_) => (
            "⏳ ",
            Style::default().fg(Color::Yellow),
            Style::default().fg(Color::Gray),
        ),
        BlockContent::WorkspaceSelector(_) => (
            "🌐 ",
            Style::default().fg(Color::Blue),
            Style::default().fg(Color::Gray),
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
        BlockContent::Editor(editor) => {
            // Show editor name and preview of content
            let preview = editor.lines.iter().take(3).cloned().collect::<Vec<_>>().join(" | ");
            let truncated = if preview.len() > 50 {
                format!("{}...", &preview[..47])
            } else {
                preview
            };
            format!("{}: {}", editor.name, truncated)
        }
        BlockContent::Results(results) => {
            // Show results name and row count
            format!("{} ({} rows)", results.name, results.rows.len())
        }
        BlockContent::InputForm(form) => {
            // Show form name and field count
            format!("{} (input form)", form.name)
        }
        BlockContent::InputDef(def) => {
            // Show input definition name
            format!("Define input: {}", def.name)
        }
        BlockContent::RunProgress(progress) => {
            // Show progress name and status
            let status = if progress.is_running() {
                "running"
            } else {
                "complete"
            };
            format!("{} ({})", progress.name, status)
        }
        BlockContent::WorkspaceSelector(selector) => {
            // Show selector name and selection count
            format!("{} ({} selected)", selector.name, selector.selected_count())
        }
    };

    let content_lines: Vec<&str> = content.lines().collect();
    let line_count = content_lines.len();

    // If minimized, show single line summary with line count
    if block.minimized {
        let summary = content_lines.first().copied().unwrap_or("(empty)");
        let is_command_block = matches!(&block.content, BlockContent::Command(_));
        let is_text_or_error_block = matches!(&block.content, BlockContent::Text(_) | BlockContent::Error(_));

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

        let mut spans = vec![
            Span::styled(focus_indicator, focus_style),
            Span::styled(controls.minimize, controls.style),
            Span::styled(prefix, prefix_style.add_modifier(Modifier::DIM)),
        ];

        // Apply appropriate styling to minimized blocks (with dim modifier)
        if is_command_block {
            let highlighted = syntax_highlight::highlight(&truncated);
            for token in highlighted {
                let dimmed_style = token.kind.style().add_modifier(Modifier::DIM);
                spans.push(Span::styled(token.text.clone(), dimmed_style));
            }
        } else if is_text_or_error_block {
            // Parse ANSI codes and apply dim modifier
            let ansi_spans = ansi_parser::parse_ansi(&truncated);
            for span in ansi_spans {
                let dimmed_style = span.style.add_modifier(Modifier::DIM);
                spans.push(Span::styled(span.content.into_owned(), dimmed_style));
            }
        } else {
            spans.push(Span::styled(truncated, content_style.add_modifier(Modifier::DIM)));
        }

        spans.push(Span::styled(line_info, Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(" ", Style::default()));
        spans.push(Span::styled(controls.remove, controls.style));

        lines.push(Line::from(spans));

        return lines;
    }

    // Full render - first line with controls
    let controls = if is_focused {
        ControlStyle::focused()
    } else {
        ControlStyle::unfocused()
    };

    // Determine block type for specialized rendering
    let is_command_block = matches!(&block.content, BlockContent::Command(_));
    let is_text_or_error_block = matches!(&block.content, BlockContent::Text(_) | BlockContent::Error(_));

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

        // Apply appropriate styling based on block type
        if is_command_block {
            // Syntax highlighting for commands
            let highlighted = syntax_highlight::highlight(line_text);
            for token in highlighted {
                spans.push(Span::styled(token.text.clone(), token.kind.style()));
            }
        } else if is_text_or_error_block {
            // Parse ANSI codes for text/error output
            let ansi_spans = ansi_parser::parse_ansi(line_text);
            spans.extend(ansi_spans);
        } else {
            spans.push(Span::styled(line_text.to_string(), content_style));
        }

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
