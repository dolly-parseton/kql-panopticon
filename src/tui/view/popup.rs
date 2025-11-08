use crate::tui::model::{
    jobs::JobState, query::QueryModel, session::SessionModel, settings::SettingsModel, Model,
    Popup,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

// Popup size constants (percentage of screen)
const ERROR_POPUP_WIDTH: u16 = 60;
const ERROR_POPUP_HEIGHT: u16 = 30;
const SETTINGS_EDIT_POPUP_WIDTH: u16 = 60;
const SETTINGS_EDIT_POPUP_HEIGHT: u16 = 25;
const JOB_NAME_INPUT_POPUP_WIDTH: u16 = 50;
const JOB_NAME_INPUT_POPUP_HEIGHT: u16 = 20;
const SESSION_NAME_INPUT_POPUP_WIDTH: u16 = 50;
const SESSION_NAME_INPUT_POPUP_HEIGHT: u16 = 20;
const JOB_DETAILS_POPUP_WIDTH: u16 = 80;
const JOB_DETAILS_POPUP_HEIGHT: u16 = 80;

/// Render a popup window
pub fn render(f: &mut Frame, popup: &Popup, model: &Model) {
    match popup {
        Popup::Error(msg) => render_error(f, msg),
        Popup::SettingsEdit => render_settings_edit(f, &model.settings),
        Popup::JobNameInput => render_job_name_input(f, &model.query),
        Popup::SessionNameInput => render_session_name_input(f, &model.sessions),
        Popup::JobDetails(job_idx) => {
            if let Some(job) = model.jobs.jobs.get(*job_idx) {
                render_job_details(f, job);
            }
        }
    }
}

/// Render an error popup
fn render_error(f: &mut Frame, msg: &str) {
    let area = centered_rect(ERROR_POPUP_WIDTH, ERROR_POPUP_HEIGHT, f.area());

    let paragraph = Paragraph::new(msg)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Error")
                .style(Style::default().bg(Color::Black).fg(Color::Red)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

/// Render the settings edit popup
fn render_settings_edit(f: &mut Frame, settings: &SettingsModel) {
    let area = centered_rect(
        SETTINGS_EDIT_POPUP_WIDTH,
        SETTINGS_EDIT_POPUP_HEIGHT,
        f.area(),
    );

    let input = settings.editing.as_deref().unwrap_or("");
    let text = format!(
        "Edit {}\n\nValue: {}_\n\nPress Enter to save, Esc to cancel",
        settings.get_selected_name(),
        input
    );

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Edit Setting")
            .style(Style::default().bg(Color::Black)),
    );

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

/// Render the job name input popup
fn render_job_name_input(f: &mut Frame, query: &QueryModel) {
    let area = centered_rect(
        JOB_NAME_INPUT_POPUP_WIDTH,
        JOB_NAME_INPUT_POPUP_HEIGHT,
        f.area(),
    );

    let input = query.job_name_input.as_deref().unwrap_or("");
    let text = format!("Job Name: {}_", input);
    let paragraph = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Enter Job Name")
            .style(Style::default().bg(Color::Black)),
    );

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

/// Render the session name input popup
fn render_session_name_input(f: &mut Frame, sessions: &SessionModel) {
    let area = centered_rect(
        SESSION_NAME_INPUT_POPUP_WIDTH,
        SESSION_NAME_INPUT_POPUP_HEIGHT,
        f.area(),
    );

    let input = sessions.name_input.as_deref().unwrap_or("");
    let text = format!(
        "Session Name: {}_\n\nPress Enter to save, Esc to cancel",
        input
    );
    let paragraph = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("New Session")
            .style(Style::default().bg(Color::Black)),
    );

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

/// Render the job details popup
fn render_job_details(f: &mut Frame, job: &JobState) {
    use crate::tui::model::jobs::JobStatus;
    let area = centered_rect(JOB_DETAILS_POPUP_WIDTH, JOB_DETAILS_POPUP_HEIGHT, f.area());

    // Determine if job can be retried
    let can_retry = matches!(job.status, JobStatus::Failed | JobStatus::Completed)
        && job.retry_context.is_some();

    // Calculate max width for text wrapping (popup width - borders (2) - some margin (4))
    let max_text_width = area.width.saturating_sub(6) as usize;

    // Style constants
    let label_style = Style::default().fg(Color::Rgb(255, 191, 0)); // Amber color
    let value_style = Style::default().fg(Color::White);

    let mut lines = vec![Line::from("")]; // Empty line for top padding

    // Status line with colored status value
    lines.push(Line::from(vec![
        Span::styled("  Status: ", label_style),
        Span::styled(
            job.status.as_str(),
            Style::default()
                .fg(job.status.color())
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    if let Some(ref result) = job.result {
        // Workspace line
        lines.push(Line::from(vec![
            Span::styled("  Workspace: ", label_style),
            Span::styled(
                format!("{} ({})", result.workspace_name, job.workspace_name),
                value_style,
            ),
        ]));

        // Workspace ID line
        lines.push(Line::from(vec![
            Span::styled("  Workspace ID: ", label_style),
            Span::styled(&result.workspace_id, value_style),
        ]));

        // Query line - label on its own line, then indented wrapped content
        lines.push(Line::from(Span::styled("  Query:", label_style)));
        let wrapped_query = wrap_text_with_indent(&result.query, 4, max_text_width);
        for wrapped_line in wrapped_query {
            lines.push(Line::from(Span::styled(wrapped_line, value_style)));
        }

        // Duration line
        lines.push(Line::from(vec![
            Span::styled("  Duration: ", label_style),
            Span::styled(format!("{:.2}s", result.elapsed.as_secs_f64()), value_style),
        ]));

        // Timestamp line
        lines.push(Line::from(vec![
            Span::styled("  Timestamp: ", label_style),
            Span::styled(result.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(), value_style),
        ]));

        match &result.result {
            Ok(success) => {
                // Rows line
                lines.push(Line::from(vec![
                    Span::styled("  Rows: ", label_style),
                    Span::styled(success.row_count.to_string(), value_style),
                ]));

                // Output line
                lines.push(Line::from(vec![
                    Span::styled("  Output: ", label_style),
                    Span::styled(success.output_path.display().to_string(), value_style),
                ]));

                // Size line
                lines.push(Line::from(vec![
                    Span::styled("  Size: ", label_style),
                    Span::styled(format!("{} bytes", success.file_size), value_style),
                ]));
            }
            Err(_) => {
                // Use structured error if available, otherwise fallback to raw error
                let error_message = if let Some(ref error) = job.error {
                    error.detailed_description()
                } else {
                    result.result.as_ref().unwrap_err().to_string()
                };

                // Error label on its own line, then indented wrapped content
                lines.push(Line::from(Span::styled("  Error:", label_style)));
                let wrapped_error = wrap_text_with_indent(&error_message, 4, max_text_width);
                for wrapped_line in wrapped_error {
                    lines.push(Line::from(Span::styled(
                        wrapped_line,
                        Style::default().fg(Color::Red),
                    )));
                }
            }
        }
    } else {
        // No result available yet (queued/running)
        lines.push(Line::from(vec![
            Span::styled("  Workspace: ", label_style),
            Span::styled(&job.workspace_name, value_style),
        ]));

        // Query preview - label on its own line, then indented wrapped content
        lines.push(Line::from(Span::styled("  Query:", label_style)));
        let wrapped_query = wrap_text_with_indent(&job.query_preview, 4, max_text_width);
        for wrapped_line in wrapped_query {
            lines.push(Line::from(Span::styled(wrapped_line, value_style)));
        }
    }

    // Add retry hint with smart retryability checking
    if can_retry {
        lines.push(Line::from(""));

        // Check if error is retryable
        let (retry_text, retry_color) = if let Some(error) = &job.error {
            if error.is_retryable() {
                ("  Press 'r' to retry this job", Color::Yellow)
            } else {
                ("  (Cannot retry: query syntax error - fix query first)", Color::DarkGray)
            }
        } else {
            // No error details - allow retry (backwards compatibility)
            ("  Press 'r' to retry this job", Color::Yellow)
        };

        lines.push(Line::from(Span::styled(
            retry_text,
            Style::default().fg(retry_color),
        )));
    } else if matches!(job.status, JobStatus::Failed | JobStatus::Completed) {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  (Cannot retry: missing context)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Job Details")
                .style(Style::default().bg(Color::Black)),
        );
        // Note: No .wrap() - we manually wrap text to maintain indentation

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

/// Helper to wrap text with indentation, respecting line width
fn wrap_text_with_indent(text: &str, indent: usize, max_width: usize) -> Vec<String> {
    let mut wrapped_lines = Vec::new();
    let indent_str = " ".repeat(indent);

    for line in text.lines() {
        if line.is_empty() {
            wrapped_lines.push(indent_str.clone());
            continue;
        }

        let available_width = max_width.saturating_sub(indent);
        if available_width == 0 {
            wrapped_lines.push(format!("{}{}", indent_str, line));
            continue;
        }

        let mut remaining = line;
        while !remaining.is_empty() {
            if remaining.len() <= available_width {
                wrapped_lines.push(format!("{}{}", indent_str, remaining));
                break;
            }

            // Find a good break point (prefer space)
            let mut split_at = available_width;
            if let Some(pos) = remaining[..available_width].rfind(' ') {
                split_at = pos;
            }

            wrapped_lines.push(format!("{}{}", indent_str, &remaining[..split_at].trim_end()));
            remaining = remaining[split_at..].trim_start();
        }
    }

    wrapped_lines
}

/// Helper to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
