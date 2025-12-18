//! Execution logs modal rendering

use crate::app::logs::{event_level, format_event, wrap_text, LogsState};
use crate::theme::Theme;
use kql_panopticon_core::tracing::LogLevel;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;

/// Render the logs modal as a centered popup
pub fn render_logs_modal(frame: &mut Frame, state: &mut LogsState, theme: &Theme) {
    let area = centered_rect(80, 80, frame.area());

    // Clear the area behind the modal
    frame.render_widget(Clear, area);

    // Header with instructions
    let header = build_header(theme);

    // Footer with stats
    let footer = build_footer(state, theme);

    // Split area into header, content, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(5),    // Content
            Constraint::Length(3), // Footer
        ])
        .split(area);

    // Render header
    let header_block = Block::default()
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .border_type(theme.border_type())
        .border_style(theme.border_style())
        .style(theme.modal_overlay_style())
        .title(" Execution Logs ");
    let header_para = Paragraph::new(header)
        .block(header_block)
        .style(theme.text_style())
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(header_para, chunks[0]);

    // Calculate content area dimensions
    let content_block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_type(theme.border_type())
        .border_style(theme.border_style())
        .style(theme.modal_overlay_style());
    let content_inner = content_block.inner(chunks[1]);

    // Update viewport height in state (subtract 1 for scrollbar column space)
    let viewport_height = content_inner.height as usize;
    let content_width = content_inner.width.saturating_sub(2) as usize; // Reserve space for scrollbar
    state.set_viewport_height(viewport_height);

    // Render content block border
    frame.render_widget(content_block, chunks[1]);

    // Render log content
    if state.is_empty() {
        // Empty state message
        let empty_msg = if state.total_count() == 0 {
            "No execution logs yet. Run a pack to see events."
        } else {
            "No events match the current filter."
        };
        let empty_para = Paragraph::new(empty_msg)
            .style(theme.text_dim_style())
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(empty_para, content_inner);
    } else {
        // Render visible log lines
        render_log_lines(frame, state, content_inner, content_width, theme);

        // Render scrollbar if content exceeds viewport
        let max_scroll = state.max_scroll();
        if max_scroll > 0 {
            let scrollbar_area = Rect {
                x: content_inner.x + content_inner.width.saturating_sub(1),
                y: content_inner.y,
                width: 1,
                height: content_inner.height,
            };

            let mut scrollbar_state =
                ScrollbarState::new(max_scroll).position(state.scroll_offset());

            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    // Render footer
    let footer_block = Block::default()
        .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
        .border_type(theme.border_type())
        .border_style(theme.border_style())
        .style(theme.modal_overlay_style());
    let footer_para = Paragraph::new(footer)
        .block(footer_block)
        .style(theme.text_style())
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(footer_para, chunks[2]);
}

/// Render the log lines with expand-in-place for selected item
fn render_log_lines(
    frame: &mut Frame,
    state: &LogsState,
    area: Rect,
    content_width: usize,
    theme: &Theme,
) {
    let cursor = state.cursor();
    let viewport_height = area.height as usize;

    // Track current Y position (running total)
    let mut current_y: u16 = 0;

    for (filtered_idx, event) in state.visible_events() {
        // Stop if we've filled the viewport
        if current_y >= viewport_height as u16 {
            break;
        }

        let is_selected = filtered_idx == cursor;
        let level = event_level(event);
        let base_style = style_for_level(level, theme);

        // Format the event text
        let text = format_event(event);

        if is_selected {
            // Selected item: expand with word wrapping
            let wrapped = wrap_text(&text, content_width);

            for (wrap_idx, line_text) in wrapped.iter().enumerate() {
                let line_y = area.y + current_y;
                if current_y >= viewport_height as u16 {
                    break;
                }

                // First line gets reversed style, continuation lines keep same color
                let style = if wrap_idx == 0 {
                    base_style.add_modifier(Modifier::REVERSED)
                } else {
                    base_style
                };

                let line_area = Rect {
                    x: area.x,
                    y: line_y,
                    width: area.width.saturating_sub(1), // Leave room for scrollbar
                    height: 1,
                };

                let para = Paragraph::new(line_text.as_str()).style(style);
                frame.render_widget(para, line_area);
                current_y += 1;
            }
        } else {
            // Non-selected: single line, truncated
            let line_y = area.y + current_y;

            let line_area = Rect {
                x: area.x,
                y: line_y,
                width: area.width.saturating_sub(1), // Leave room for scrollbar
                height: 1,
            };

            // Truncate text if too long
            let display_text = if text.chars().count() > content_width {
                let truncated: String = text.chars().take(content_width.saturating_sub(1)).collect();
                format!("{}…", truncated)
            } else {
                text
            };

            let para = Paragraph::new(display_text).style(base_style);
            frame.render_widget(para, line_area);
            current_y += 1;
        }
    }
}

/// Build the header line with instructions
fn build_header(theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled("[↑/↓]", theme.text_dim_style()),
        Span::raw(" Navigate  "),
        Span::styled("[PgUp/Dn]", theme.text_dim_style()),
        Span::raw(" Page  "),
        Span::styled("[g/G]", theme.text_dim_style()),
        Span::raw(" Top/End  "),
        Span::styled("[l]", Style::default().fg(theme.colors.accent.clone().into())),
        Span::raw(" Filter  "),
        Span::styled("[c]", Style::default().fg(theme.colors.warning.clone().into())),
        Span::raw(" Clear  "),
        Span::styled("[Esc]", Style::default().fg(theme.colors.error.clone().into())),
        Span::raw(" Close"),
    ])
}

/// Build the footer line with stats
fn build_footer(state: &LogsState, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();

    // Position indicator
    if !state.is_empty() {
        spans.push(Span::raw(format!(
            "{}/{}",
            state.cursor() + 1,
            state.filtered_count()
        )));
        spans.push(Span::raw(" │ "));
    }

    // Event count
    if state.min_level().is_some() {
        spans.push(Span::raw(format!(
            "{}/{} events",
            state.filtered_count(),
            state.total_count()
        )));
    } else {
        spans.push(Span::raw(format!("{} events", state.total_count())));
    }

    spans.push(Span::raw(" │ "));

    // Filter status
    spans.push(Span::raw("Filter: "));
    match state.min_level() {
        None => {
            spans.push(Span::styled("All", theme.text_dim_style()));
        }
        Some(level) => {
            let (level_str, style) = match level {
                LogLevel::Error => (
                    "≥ERROR",
                    Style::default().fg(theme.colors.error.clone().into()),
                ),
                LogLevel::Warn => (
                    "≥WARN",
                    Style::default().fg(theme.colors.warning.clone().into()),
                ),
                LogLevel::Info => (
                    "≥INFO",
                    Style::default().fg(theme.colors.text.clone().into()),
                ),
                LogLevel::Debug => ("≥DEBUG", theme.text_dim_style()),
                LogLevel::Trace => ("≥TRACE", theme.text_dim_style()),
            };
            spans.push(Span::styled(level_str, style));
        }
    }

    // Error count
    let error_count = state.error_count();
    if error_count > 0 {
        spans.push(Span::raw(" │ "));
        spans.push(Span::styled(
            format!("Errors: {}", error_count),
            Style::default()
                .fg(theme.colors.error.clone().into())
                .add_modifier(Modifier::BOLD),
        ));
    }

    Line::from(spans)
}

/// Get style for a log level
fn style_for_level(level: LogLevel, theme: &Theme) -> Style {
    match level {
        LogLevel::Error => Style::default().fg(theme.colors.error.clone().into()),
        LogLevel::Warn => Style::default().fg(theme.colors.warning.clone().into()),
        LogLevel::Info => theme.text_style(),
        LogLevel::Debug | LogLevel::Trace => theme.text_dim_style(),
    }
}

/// Create a centered rectangle of given percentage width and height
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
