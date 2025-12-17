use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};
use ratatui::Frame;

pub fn render_interpreter(frame: &mut Frame, app: &crate::app::App, chunk: ratatui::layout::Rect) {
    // Outer container with left padding
    let interpreter_block = Block::default()
        .borders(Borders::NONE)
        .padding(Padding::left(1))
        .style(app.theme.background_style());
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

        let command_area = ratatui::layout::Rect {
            x: inner_area.x,
            y: inner_area.y + y,
            width: inner_area.width,
            height: 1, // Single line for now
        };

        // [status icon]➜ command
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
        let command_para = Paragraph::new(command_line);
        frame.render_widget(command_para, command_area);

        let result_lines = block.results.lines().count();

        let results_area = ratatui::layout::Rect {
            x: inner_area.x,
            y: inner_area.y + y + 1,
            width: inner_area.width - 2,
            height: result_lines as u16,
        };

        let results_para = Paragraph::new(block.results.clone()).style(app.theme.text_style());
        frame.render_widget(results_para, results_area);

        y += LINES_PER_BLOCK as u16 + result_lines as u16 + 1; // +1 for spacing
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
