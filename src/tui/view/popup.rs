use crate::tui::model::{
    jobs::JobState, query::QueryModel, session::SessionModel, settings::SettingsModel, Model,
    Popup,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
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

    let retry_hint = if can_retry {
        "\n\nPress 'r' to retry this job"
    } else if matches!(job.status, JobStatus::Failed | JobStatus::Completed) {
        "\n\n(Cannot retry: missing context)"
    } else {
        ""
    };

    let details = if let Some(ref result) = job.result {
        match &result.result {
            Ok(success) => format!(
                "Status: {}\n\
                 Workspace: {} ({})\n\
                 Workspace ID: {}\n\
                 Query: {}\n\
                 Duration: {:.2}s\n\
                 Rows: {}\n\
                 Output: {}\n\
                 Size: {} bytes{}",
                job.status.as_str(),
                result.workspace_name,
                job.workspace_name,
                result.workspace_id,
                result.query,
                result.elapsed.as_secs_f64(),
                success.row_count,
                success.output_path.display(),
                success.file_size,
                retry_hint
            ),
            Err(e) => format!(
                "Status: FAILED\n\
                 Workspace: {} ({})\n\
                 Workspace ID: {}\n\
                 Query: {}\n\
                 Duration: {:.2}s\n\
                 Error: {}{}",
                result.workspace_name,
                job.workspace_name,
                result.workspace_id,
                result.query,
                result.elapsed.as_secs_f64(),
                e,
                retry_hint
            ),
        }
    } else {
        format!(
            "Status: {}\n\
             Workspace: {}\n\
             Query: {}{}",
            job.status.as_str(),
            job.workspace_name,
            job.query_preview,
            retry_hint
        )
    };

    let paragraph = Paragraph::new(details)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Job Details")
                .style(Style::default().bg(Color::Black)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
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
