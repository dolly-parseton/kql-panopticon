use crate::tui::model::investigations::{InvestigationStatus, InvestigationsModel, StepState};
use crate::tui::model::Model;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

/// Render the Investigations tab
pub fn render(f: &mut Frame, model: &mut Model, area: Rect) {
    // Check if we're collecting inputs
    if model.investigations.is_collecting_inputs() {
        render_input_collection(f, &model.investigations, area);
        return;
    }

    // Check if there's an active investigation
    if model.investigations.active.is_some() {
        render_active_investigation(f, &model.investigations, area);
        return;
    }

    // Normal view: pack list + details
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    render_pack_list(f, &mut model.investigations, chunks[0]);
    render_pack_details(f, &model.investigations, chunks[1]);
}

/// Render the list of investigation packs
fn render_pack_list(f: &mut Frame, model: &mut InvestigationsModel, area: Rect) {
    // Show loading or error state
    if model.loading {
        let loading_paragraph = Paragraph::new("Loading investigation packs...")
            .block(Block::default().borders(Borders::ALL).title("Investigations"))
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(loading_paragraph, area);
        return;
    }

    if let Some(error) = &model.error {
        let error_paragraph = Paragraph::new(format!("Error: {}", error))
            .block(Block::default().borders(Borders::ALL).title("Investigations"))
            .style(Style::default().fg(Color::Red));
        f.render_widget(error_paragraph, area);
        return;
    }

    // Show empty state
    if model.packs.is_empty() {
        let empty_lines = vec![
            Line::from(""),
            Line::from("No investigation packs found"),
            Line::from(""),
            Line::from(vec![
                Span::raw("Create packs in: "),
                Span::styled(
                    "~/.kql-panopticon/investigations/",
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(""),
            Line::from("Press 'r' to refresh"),
        ];

        let empty_paragraph = Paragraph::new(empty_lines)
            .block(Block::default().borders(Borders::ALL).title("Investigations"))
            .style(Style::default().fg(Color::Gray))
            .wrap(Wrap { trim: true });
        f.render_widget(empty_paragraph, area);
        return;
    }

    // Create header
    let header = Row::new(vec!["Investigation", "Steps", "Inputs"])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

    // Create rows
    let rows: Vec<Row> = model
        .packs
        .iter()
        .map(|entry| {
            let name = entry.get_display_name();
            let step_count = entry
                .get_step_count()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "?".to_string());
            let input_count = entry
                .get_input_count()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "?".to_string());

            // Show error indicator if pack has issues
            let name_with_indicator = if entry.has_error() {
                format!("! {}", name)
            } else {
                name
            };

            let style = if entry.has_error() {
                Style::default().fg(Color::Red)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(name_with_indicator).style(style),
                Cell::from(step_count),
                Cell::from(input_count),
            ])
        })
        .collect();

    // Calculate column widths
    let widths = [
        Constraint::Percentage(60),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Investigations ({})", model.pack_count())),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(table, area, &mut model.table_state);
}

/// Render details for the selected pack
fn render_pack_details(f: &mut Frame, model: &InvestigationsModel, area: Rect) {
    let selected_entry = model.get_selected_entry();

    if selected_entry.is_none() {
        let help_paragraph = Paragraph::new(vec![
            Line::from(""),
            Line::from("No investigation selected"),
            Line::from(""),
            Line::from("Use Up/Down to select an investigation"),
        ])
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                .title("Investigation Details"),
        )
        .style(Style::default().fg(Color::Gray));
        f.render_widget(help_paragraph, area);
        return;
    }

    let entry = selected_entry.unwrap();

    // Show error if pack has issues
    if let Some(error) = entry.get_error() {
        let error_paragraph = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Failed to load investigation",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(error),
            Line::from(""),
            Line::from(Span::styled(
                format!("File: {}", entry.relative_path),
                Style::default().fg(Color::Gray),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                .title("Investigation Details"),
        )
        .wrap(Wrap { trim: true });
        f.render_widget(error_paragraph, area);
        return;
    }

    // Show loading state if pack not loaded yet
    if entry.pack.is_none() {
        let loading_paragraph =
            Paragraph::new("Press Enter to load investigation details...")
                .block(
                    Block::default()
                        .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                        .title("Investigation Details"),
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

    // Add version if present
    if let Some(version) = &pack.version {
        lines.push(Line::from(vec![
            Span::styled("Version: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(version),
        ]));
        lines.push(Line::from(""));
    }

    // Add inputs section if present
    if !pack.inputs.is_empty() {
        lines.push(Line::from(Span::styled(
            "Inputs:",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        for input in &pack.inputs {
            let required = if input.required { " *" } else { "" };
            let default_text = input
                .default
                .as_ref()
                .map(|d| format!(" [{}]", d))
                .unwrap_or_default();

            lines.push(Line::from(vec![
                Span::styled(format!("  {}", input.name), Style::default().fg(Color::Cyan)),
                Span::styled(required, Style::default().fg(Color::Red)),
                Span::styled(default_text, Style::default().fg(Color::Gray)),
            ]));

            if let Some(desc) = &input.description {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(desc, Style::default().fg(Color::Gray)),
                ]));
            }
        }
        lines.push(Line::from(""));
    }

    // Add steps section
    lines.push(Line::from(Span::styled(
        "Steps:",
        Style::default().add_modifier(Modifier::BOLD),
    )));

    for (i, step) in pack.steps.iter().enumerate() {
        let deps = if step.depends_on.is_empty() {
            String::new()
        } else {
            format!(" <- {}", step.depends_on.join(", "))
        };

        let extracts = if step.extract.is_empty() {
            String::new()
        } else {
            format!(
                " -> {}",
                step.extract.keys().cloned().collect::<Vec<_>>().join(", ")
            )
        };

        lines.push(Line::from(vec![
            Span::styled(format!("  {}. ", i + 1), Style::default().fg(Color::Yellow)),
            Span::raw(&step.name),
            Span::styled(deps, Style::default().fg(Color::Gray)),
            Span::styled(extracts, Style::default().fg(Color::Green)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(""));

    // Add controls
    lines.push(Line::from(Span::styled(
        "Controls:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from("  Enter - View details / Load pack"));
    lines.push(Line::from("  e - Execute investigation"));
    lines.push(Line::from("  r - Refresh list"));

    let details_paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                .title("Investigation Details"),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(details_paragraph, area);
}

/// Render input collection dialog
fn render_input_collection(f: &mut Frame, model: &InvestigationsModel, area: Rect) {
    let Some(state) = &model.input_collection else {
        return;
    };

    let entry = model.packs.iter().find(|e| e.path == state.pack_path);
    let pack = entry.and_then(|e| e.pack.as_ref());

    let mut lines = vec![
        Line::from(Span::styled(
            "Configure Investigation Inputs",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        )),
        Line::from(""),
    ];

    if let Some(pack) = pack {
        for (i, input) in pack.inputs.iter().enumerate() {
            let is_current = i == state.current_input;
            let value = if is_current {
                &state.current_value
            } else {
                state.inputs.get(&input.name).map(|s| s.as_str()).unwrap_or("")
            };

            let prefix = if is_current { "> " } else { "  " };
            let required = if input.required { " *" } else { "" };

            let name_style = if is_current {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };

            lines.push(Line::from(vec![
                Span::raw(prefix),
                Span::styled(&input.name, name_style),
                Span::styled(required, Style::default().fg(Color::Red)),
                Span::raw(": "),
                Span::styled(
                    if is_current {
                        format!("{}|", value)
                    } else {
                        value.to_string()
                    },
                    if is_current {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                ),
            ]));

            if let Some(desc) = &input.description {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(desc, Style::default().fg(Color::DarkGray)),
                ]));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Controls:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from("  Tab/Down - Next input"));
    lines.push(Line::from("  Shift+Tab/Up - Previous input"));
    lines.push(Line::from("  Enter - Execute investigation"));
    lines.push(Line::from("  Esc - Cancel"));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Investigation Inputs"),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render active investigation progress
fn render_active_investigation(f: &mut Frame, model: &InvestigationsModel, area: Rect) {
    let Some(active) = &model.active else {
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left side: Steps progress
    let mut step_lines = vec![
        Line::from(Span::styled(
            &active.name,
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Steps:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];

    for step in &active.steps {
        let (icon, style) = match &step.status {
            StepState::Pending => ("[ ]", Style::default().fg(Color::Gray)),
            StepState::Running => ("[~]", Style::default().fg(Color::Yellow)),
            StepState::Completed { rows } => {
                step_lines.push(Line::from(vec![
                    Span::styled("[+] ", Style::default().fg(Color::Green)),
                    Span::raw(&step.name),
                    Span::styled(format!(" ({} rows)", rows), Style::default().fg(Color::Gray)),
                ]));
                continue;
            }
            StepState::Failed { .. } => ("[X]", Style::default().fg(Color::Red)),
            StepState::Skipped => ("[-]", Style::default().fg(Color::DarkGray)),
        };

        step_lines.push(Line::from(vec![
            Span::styled(format!("{} ", icon), style),
            Span::styled(&step.name, style),
        ]));
    }

    let steps_paragraph = Paragraph::new(step_lines)
        .block(Block::default().borders(Borders::ALL).title("Progress"))
        .wrap(Wrap { trim: true });

    f.render_widget(steps_paragraph, chunks[0]);

    // Right side: Workspace progress
    let mut ws_lines = vec![
        Line::from(Span::styled(
            "Workspaces:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (name, progress) in &active.workspace_progress {
        let status_icon = match progress.status {
            InvestigationStatus::Pending => "[ ]",
            InvestigationStatus::Running => "[~]",
            InvestigationStatus::Completed => "[+]",
            InvestigationStatus::Failed => "[X]",
        };

        let status_style = match progress.status {
            InvestigationStatus::Pending => Style::default().fg(Color::Gray),
            InvestigationStatus::Running => Style::default().fg(Color::Yellow),
            InvestigationStatus::Completed => Style::default().fg(Color::Green),
            InvestigationStatus::Failed => Style::default().fg(Color::Red),
        };

        ws_lines.push(Line::from(vec![
            Span::styled(format!("{} ", status_icon), status_style),
            Span::raw(name),
            Span::styled(
                format!(" ({}/{})", progress.completed_steps, progress.total_steps),
                Style::default().fg(Color::Gray),
            ),
        ]));

        if let Some(current_step) = &progress.current_step {
            ws_lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(current_step, Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    let ws_paragraph = Paragraph::new(ws_lines)
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                .title("Workspace Status"),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(ws_paragraph, chunks[1]);
}
