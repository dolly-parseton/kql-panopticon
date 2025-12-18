use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

pub fn render_interpreter(frame: &mut Frame, app: &mut crate::app::App, chunk: Rect) {
    // Outer container with left padding, reserve 1 column for scrollbar
    let interpreter_block = Block::default()
        .borders(Borders::NONE)
        .padding(Padding::new(1, 1, 0, 0)) // left, right, top, bottom
        .style(app.theme.background_style());
    let inner_area = interpreter_block.inner(chunk);
    frame.render_widget(interpreter_block, chunk);

    let viewport_height = inner_area.height as usize;

    // Update stored viewport height for auto-scroll calculations
    app.ui_state.viewport_height = viewport_height;

    // Auto-scroll to bottom if enabled and content exceeds viewport
    if app.ui_state.auto_scroll {
        app.update_scroll_for_new_content(viewport_height);
    }

    let scroll_offset = app.ui_state.scroll_offset;
    let total_content_height = app.total_content_height();

    // Calculate cumulative positions and find visible blocks
    let mut cumulative_y = 0usize;

    for block in &app.blocks {
        let block_height = block.rendered_height();
        let block_start = cumulative_y;
        let block_end = cumulative_y + block_height;

        // Check if this block intersects with visible viewport
        let viewport_start = scroll_offset;
        let viewport_end = scroll_offset + viewport_height;

        if block_end > viewport_start && block_start < viewport_end {
            // This block is at least partially visible
            // Calculate where it should render relative to viewport
            let render_start = block_start as i32 - scroll_offset as i32;
            render_block(frame, app, block, inner_area, render_start, viewport_height);
        }

        cumulative_y += block_height;

        // Early exit if we've passed the viewport
        if cumulative_y > scroll_offset + viewport_height {
            break;
        }
    }

    // Render scrollbar if content exceeds viewport
    if total_content_height > viewport_height {
        // Align scrollbar with the content area (inner_area)
        let scrollbar_area = Rect {
            x: chunk.x + chunk.width.saturating_sub(2), // 1 unit from edge
            y: inner_area.y,
            width: 1,
            height: inner_area.height,
        };

        // Calculate max scroll position for accurate thumb positioning
        let max_scroll = total_content_height.saturating_sub(viewport_height);

        let mut scrollbar_state = ScrollbarState::new(max_scroll)
            .position(scroll_offset);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█");

        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }
}

/// Render a single block at the specified position
/// render_y can be negative if the block is partially scrolled off the top
fn render_block(
    frame: &mut Frame,
    app: &crate::app::App,
    block: &crate::app::InterpreterBlock,
    area: Rect,
    render_y: i32,
    viewport_height: usize,
) {
    let frame_idx = app.animation_frame();
    let (icon, icon_style) = get_status_icon_and_style(&block.status, &app.theme, frame_idx);

    // Build command line with syntax highlighting
    let mut command_spans = vec![
        Span::raw(" "),
        Span::styled(icon, icon_style),
        Span::styled(" ➜ ", app.theme.text_dim_style()),
    ];
    command_spans.extend(app.theme.highlight_command(&block.command));
    let command_line = Line::from(command_spans);

    // Build result lines
    let result_lines: Vec<Line> = block
        .results
        .lines()
        .map(|l| Line::from(l.to_string()).style(app.theme.text_style()))
        .collect();

    // Combine into a single set of lines for this block
    let mut all_lines = vec![command_line];
    all_lines.extend(result_lines);
    all_lines.push(Line::from("")); // spacing line

    for (line_idx, line) in all_lines.into_iter().enumerate() {
        let screen_y = render_y + line_idx as i32;

        // Skip if above viewport
        if screen_y < 0 {
            continue;
        }

        // Stop if below viewport
        if screen_y >= viewport_height as i32 {
            break;
        }

        let line_area = Rect {
            x: area.x,
            y: area.y + screen_y as u16,
            width: area.width,
            height: 1,
        };

        let para = Paragraph::new(line);
        frame.render_widget(para, line_area);
    }
}

/// Get the status icon and style based on theme spinner config
fn get_status_icon_and_style(
    status: &crate::app::BlockStatus,
    theme: &crate::theme::Theme,
    frame_idx: usize,
) -> (String, Style) {
    let spinners = &theme.styles.spinners;

    match status {
        crate::app::BlockStatus::Running => {
            let frames = &spinners.running.frames;
            if frames.is_empty() {
                // Fallback to static icon
                return ("⟳".to_string(), theme.status_style(status));
            }

            let idx = frame_idx % frames.len();
            let icon = frames[idx].clone();

            // Check if this frame has a custom color
            let style = if idx < spinners.running.colors.len() {
                if let Some(color) = &spinners.running.colors[idx] {
                    Style::default()
                        .fg(color.clone().into())
                        .add_modifier(Modifier::BOLD)
                } else {
                    theme.status_style(status)
                }
            } else {
                theme.status_style(status)
            };

            (icon, style)
        }
        crate::app::BlockStatus::Completed => {
            (spinners.completed.clone(), theme.status_style(status))
        }
        crate::app::BlockStatus::Failed => {
            (spinners.failed.clone(), theme.status_style(status))
        }
    }
}
