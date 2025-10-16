use crate::tui::model::workspaces::WorkspacesModel;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table},
    Frame,
};

/// Render the Workspaces tab
pub fn render(f: &mut Frame, model: &mut WorkspacesModel, area: Rect) {
    // Create header
    let header = Row::new(vec!["Selected", "Name", "Location"])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

    // Create rows
    let rows: Vec<Row> = model
        .workspaces
        .iter()
        .map(|ws| {
            let checkbox = if ws.selected { "[X]" } else { "[ ]" };
            Row::new(vec![
                checkbox,
                ws.workspace.name.as_str(),
                ws.workspace.location.as_str(),
            ])
        })
        .collect();

    // Calculate column widths
    let widths = [
        ratatui::layout::Constraint::Length(10),
        ratatui::layout::Constraint::Percentage(45),
        ratatui::layout::Constraint::Percentage(45),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Workspaces ({} selected)", model.selected_count())),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(table, area, &mut model.table_state);
}
