use crate::tui::model::{packs::PacksModel, Model};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

/// Render the Query Packs tab
pub fn render(f: &mut Frame, model: &mut Model, area: Rect) {
    // Split area: left side for pack list, right side for details
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    render_pack_list(f, model, chunks[0]);
    render_pack_details(f, &model.packs, chunks[1]);
}

/// Render the list of query packs
fn render_pack_list(f: &mut Frame, model: &mut Model, area: Rect) {
    let packs_model = &mut model.packs;

    // Get currently loaded pack path from query context
    let loaded_pack_path = model
        .query
        .pack_context
        .as_ref()
        .map(|ctx| ctx.pack_path.as_str());

    // Show loading or error state
    if packs_model.loading {
        let loading_paragraph = Paragraph::new("Loading query packs...")
            .block(Block::default().borders(Borders::ALL).title("Query Packs"))
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(loading_paragraph, area);
        return;
    }

    if let Some(error) = &packs_model.error {
        let error_paragraph = Paragraph::new(format!("Error: {}", error))
            .block(Block::default().borders(Borders::ALL).title("Query Packs"))
            .style(Style::default().fg(Color::Red));
        f.render_widget(error_paragraph, area);
        return;
    }

    // Show empty state
    if packs_model.packs.is_empty() {
        let empty_lines = vec![
            Line::from(""),
            Line::from("No query packs found"),
            Line::from(""),
            Line::from(vec![
                Span::raw("Create packs in: "),
                Span::styled("~/.kql-panopticon/packs/", Style::default().fg(Color::Cyan)),
            ]),
            Line::from(""),
            Line::from("Press 'r' to refresh"),
        ];

        let empty_paragraph = Paragraph::new(empty_lines)
            .block(Block::default().borders(Borders::ALL).title("Query Packs"))
            .style(Style::default().fg(Color::Gray))
            .wrap(Wrap { trim: true });
        f.render_widget(empty_paragraph, area);
        return;
    }

    // Create header
    let header = Row::new(vec!["Pack", "Status", "Queries"])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

    // Create rows
    let rows: Vec<Row> = packs_model
        .packs
        .iter()
        .map(|entry| {
            let name = entry.get_display_name();
            let query_count = entry
                .get_query_count()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "?".to_string());

            // Show error indicator if pack failed to load
            let name_with_indicator = if entry.load_error.is_some() {
                format!("⚠ {}", name)
            } else {
                name
            };

            // Check if this pack is currently loaded
            let is_loaded = loaded_pack_path
                .map(|loaded| loaded == entry.relative_path)
                .unwrap_or(false);

            let status = if is_loaded {
                Cell::from("[LOADED]").style(Style::default().fg(Color::Green))
            } else {
                Cell::from("")
            };

            Row::new(vec![
                Cell::from(name_with_indicator),
                status,
                Cell::from(query_count),
            ])
        })
        .collect();

    // Calculate column widths
    let widths = [
        Constraint::Percentage(55),
        Constraint::Percentage(20),
        Constraint::Percentage(25),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Query Packs ({})", packs_model.pack_count())),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(table, area, &mut packs_model.table_state);
}

/// Render details for the selected pack
fn render_pack_details(f: &mut Frame, model: &PacksModel, area: Rect) {
    let selected_entry = model.get_selected_entry();

    if selected_entry.is_none() {
        let help_paragraph = Paragraph::new(vec![
            Line::from(""),
            Line::from("No pack selected"),
            Line::from(""),
            Line::from("Use Up/Down to select a pack"),
        ])
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                .title("Pack Details"),
        )
        .style(Style::default().fg(Color::Gray));
        f.render_widget(help_paragraph, area);
        return;
    }

    let entry = selected_entry.unwrap();

    // Show load error if pack failed to parse
    if let Some(error) = &entry.load_error {
        let error_paragraph = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Failed to load pack",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(error.as_str()),
            Line::from(""),
            Line::from(Span::styled(
                format!("File: {}", entry.relative_path),
                Style::default().fg(Color::Gray),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                .title("Pack Details"),
        )
        .wrap(Wrap { trim: true });
        f.render_widget(error_paragraph, area);
        return;
    }

    // Show loading state if pack not loaded yet
    if entry.pack.is_none() {
        let loading_paragraph = Paragraph::new("Press Enter to load pack details...")
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                    .title("Pack Details"),
            )
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(loading_paragraph, area);
        return;
    }

    // Render pack details
    let pack = entry.pack.as_ref().unwrap();
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&pack.name),
        ]),
        Line::from(""),
    ];

    // Add description if present
    if let Some(description) = &pack.description {
        lines.push(Line::from(vec![
            Span::styled(
                "Description: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(description),
        ]));
        lines.push(Line::from(""));
    }

    // Add author if present
    if let Some(author) = &pack.author {
        lines.push(Line::from(vec![
            Span::styled("Author: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(author),
        ]));
    }

    // Add version if present
    if let Some(version) = &pack.version {
        lines.push(Line::from(vec![
            Span::styled("Version: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(version),
        ]));
    }

    lines.push(Line::from(""));

    // Add queries section
    let queries = pack.get_queries();
    lines.push(Line::from(vec![
        Span::styled("Queries: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!("{}", queries.len())),
    ]));
    lines.push(Line::from(""));

    // List queries
    for (i, query) in queries.iter().enumerate() {
        lines.push(Line::from(vec![
            Span::styled(format!("  {}. ", i + 1), Style::default().fg(Color::Yellow)),
            Span::raw(&query.name),
        ]));

        if let Some(description) = &query.description {
            lines.push(Line::from(vec![
                Span::raw("     "),
                Span::styled(description, Style::default().fg(Color::Gray)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(""));

    // Add controls
    lines.push(Line::from(Span::styled(
        "Controls:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from("  Enter - Load first query into editor"));
    lines.push(Line::from("  s - Save current query changes to pack"));
    lines.push(Line::from("  e - Execute pack on selected workspaces"));
    lines.push(Line::from("  r - Refresh pack list"));

    let details_paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                .title("Pack Details"),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(details_paragraph, area);
}
