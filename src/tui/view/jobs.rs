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
    let header = Row::new(vec!["Status", "Workspace", "Query", "Duration"])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

    // Create rows
    // Pre-compute duration strings since they're not stored in the model
    let duration_strings: Vec<String> = model
        .jobs
        .iter()
        .map(|job| {
            job.duration
                .map(|d| format!("{:.2}s", d.as_secs_f64()))
                .unwrap_or_else(|| "-".to_string())
        })
        .collect();

    let rows: Vec<Row> = model
        .jobs
        .iter()
        .enumerate()
        .map(|(idx, job)| {
            Row::new(vec![
                job.status.as_str(),
                job.workspace_name.as_str(),
                job.query_preview.as_str(),
                duration_strings[idx].as_str(),
            ])
            .style(Style::default().fg(job.status.color()))
        })
        .collect();

    // Calculate column widths
    let widths = [
        ratatui::layout::Constraint::Length(12),
        ratatui::layout::Constraint::Percentage(25),
        ratatui::layout::Constraint::Percentage(50),
        ratatui::layout::Constraint::Length(10),
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
