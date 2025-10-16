use crate::tui::model::Model;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

/// Render the sessions tab
pub fn render(f: &mut Frame, model: &Model, area: Rect) {
    let selected_index = model.sessions.table_state.selected();

    // Create table rows
    let rows: Vec<Row> = model
        .sessions
        .sessions
        .iter()
        .enumerate()
        .map(|(idx, session)| {
            let is_selected = Some(idx) == selected_index;
            let fg_color = session.state.color(is_selected);

            let name_cell = Cell::from(session.name.clone()).style(Style::default().fg(fg_color));

            let status_cell =
                Cell::from(session.state.indicator()).style(Style::default().fg(fg_color));

            let last_saved = session
                .last_saved
                .clone()
                .unwrap_or_else(|| "Never".to_string());
            let saved_cell = Cell::from(last_saved).style(Style::default().fg(fg_color));

            Row::new(vec![name_cell, status_cell, saved_cell])
        })
        .collect();

    // Header row
    let header = Row::new(vec![
        Cell::from("Session Name").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Status").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Last Saved").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    // Create table widget
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title("Sessions")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White)),
    )
    .highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .add_modifier(Modifier::BOLD),
    );

    // Render the table
    f.render_stateful_widget(table, area, &mut model.sessions.table_state.clone());

    // Note: Help text is shown in the control bar at the bottom of the screen
}
