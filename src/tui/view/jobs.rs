use crate::tui::model::jobs::JobsModel;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table},
    Frame,
};

/// Render the Jobs tab
pub fn render(f: &mut Frame, model: &mut JobsModel, area: Rect) {
    // Create header
    let header = Row::new(vec![
        "Status",
        "Workspace",
        "Query",
        "Duration",
        "Timestamp",
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .bottom_margin(1);

    // Create rows
    // Pre-compute duration strings, status strings, and timestamp strings
    let duration_strings: Vec<String> = model
        .jobs
        .iter()
        .map(|job| {
            job.duration
                .map(|d| format!("{:.2}s", d.as_secs_f64()))
                .unwrap_or_else(|| "-".to_string())
        })
        .collect();

    let status_strings: Vec<String> = model
        .jobs
        .iter()
        .map(|job| {
            // For failed jobs, show error description if available
            if job.status == crate::tui::model::jobs::JobStatus::Failed {
                if let Some(ref error) = job.error {
                    format!("FAILED ({})", error.short_description())
                } else {
                    job.status.as_str().to_string()
                }
            } else {
                job.status.as_str().to_string()
            }
        })
        .collect();

    let timestamp_strings: Vec<String> = model
        .jobs
        .iter()
        .map(|job| {
            job.result
                .as_ref()
                .map(|r| r.timestamp.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "-".to_string())
        })
        .collect();

    let rows: Vec<Row> = model
        .jobs
        .iter()
        .enumerate()
        .map(|(idx, job)| {
            Row::new(vec![
                status_strings[idx].as_str(),
                job.workspace_name.as_str(),
                job.query_preview.as_str(),
                duration_strings[idx].as_str(),
                timestamp_strings[idx].as_str(),
            ])
            .style(Style::default().fg(job.status.color()))
        })
        .collect();

    // Calculate column widths
    let widths = [
        ratatui::layout::Constraint::Length(28), // Status - fits "FAILED (Query Error)" etc.
        ratatui::layout::Constraint::Percentage(20), // Workspace
        ratatui::layout::Constraint::Percentage(30), // Query
        ratatui::layout::Constraint::Length(10), // Duration
        ratatui::layout::Constraint::Length(19), // Timestamp - "YYYY-MM-DD HH:MM:SS"
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Jobs ({})", model.jobs.len())),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(table, area, &mut model.table_state);
}
