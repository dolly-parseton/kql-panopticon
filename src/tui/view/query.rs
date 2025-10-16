use crate::tui::model::{jobs::JobsModel, query::{EditorMode, QueryModel}};
use crate::tui::view::syntax_textarea::SyntaxTextArea;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

/// Render the Query tab
pub fn render(f: &mut Frame, model: &QueryModel, jobs_model: &JobsModel, area: Rect) {
    let mode_indicator = match model.mode {
        EditorMode::Normal => " [NORMAL] ",
        EditorMode::Insert => " [INSERT] ",
        EditorMode::Visual => " [VISUAL] ",
    };

    let mode_style = match model.mode {
        EditorMode::Normal => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        EditorMode::Insert => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        EditorMode::Visual => Style::default()
            .fg(Color::LightMagenta)
            .add_modifier(Modifier::BOLD),
    };

    // Create the block with mode indicator
    let help_text = match model.mode {
        EditorMode::Normal => " | l:LOAD i:INSERT v:VISUAL ^J:EXECUTE ^U:UNDO ^R:REDO",
        EditorMode::Insert => " | esc:NORMAL ^J:EXECUTE ^U:UNDO ^R:REDO",
        EditorMode::Visual => " | y:YANK d:DELETE esc:NORMAL",
    };

    let block = Block::default().borders(Borders::ALL).title(vec![
        Span::raw("Query "),
        Span::styled(mode_indicator, mode_style),
        Span::raw(help_text),
    ]);

    // Render the textarea with syntax highlighting
    let widget = SyntaxTextArea::new(&model.textarea).block(block);
    f.render_widget(widget, area);

    // Render load panel if open
    if let Some(panel_state) = &model.load_panel {
        render_load_panel(f, panel_state, jobs_model, area);
    }
}

/// Render the load query panel (right-aligned overlay)
fn render_load_panel(
    f: &mut Frame,
    panel_state: &crate::tui::model::query::LoadPanelState,
    jobs_model: &JobsModel,
    parent_area: Rect,
) {
    // Create right-aligned area (40% width, full height)
    let panel_width = (parent_area.width * 40) / 100;
    let panel_area = Rect {
        x: parent_area.x + parent_area.width - panel_width,
        y: parent_area.y,
        width: panel_width,
        height: parent_area.height,
    };

    // Use the sorted indices from panel state
    let sorted_indices = &panel_state.sorted_indices;

    // Create list items with job name and status
    let items: Vec<ListItem> = sorted_indices
        .iter()
        .enumerate()
        .filter_map(|(display_idx, &original_idx)| {
            let job = jobs_model.jobs.get(original_idx)?;
            let status_indicator = format!("[{}]", job.status.as_str());
            let job_name = format!("Job #{}", original_idx + 1);

            let line = Line::from(vec![
                Span::styled(
                    status_indicator,
                    Style::default().fg(job.status.color()),
                ),
                Span::raw(" "),
                Span::raw(job_name),
                Span::raw(" - "),
                Span::raw(&job.workspace_name),
            ]);

            let mut item = ListItem::new(line);
            if display_idx == panel_state.selected {
                item = item.style(Style::default().bg(Color::DarkGray));
            }
            Some(item)
        })
        .collect();

    // Create sort indicator text
    let sort_text = format!(
        "Sort: {} {}",
        panel_state.sort.as_str(),
        if panel_state.inverted { "↓" } else { "↑" }
    );

    let title = format!(
        "Load Query ({}) | {}",
        sorted_indices.len(),
        sort_text
    );

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_bottom("↑↓:Navigate Tab:Sort i:Invert Enter:Load Esc:Cancel")
            .style(Style::default().bg(Color::Black)),
    );

    // Render with stateful highlighting
    let mut list_state = ListState::default();
    list_state.select(Some(panel_state.selected));

    f.render_widget(Clear, panel_area);
    f.render_stateful_widget(list, panel_area, &mut list_state);
}
