pub mod controls;
pub mod jobs;
pub mod kql_highlight;
pub mod packs;
pub mod popup;
pub mod query;
pub mod session;
pub mod settings;
pub mod syntax_textarea;
pub mod tabs;
pub mod workspaces;

use crate::tui::message::Tab;
use crate::tui::model::Model;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

/// Main UI rendering function
pub fn ui(f: &mut Frame, model: &mut Model) {
    let size = f.area();

    // Main layout: top bar, content, bottom bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tab bar
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Controls
        ])
        .split(size);

    // Render tab bar
    tabs::render(
        f,
        model.current_tab,
        model.init_state,
        model.spinner_frame,
        chunks[0],
    );

    // Render content based on current tab
    match model.current_tab {
        Tab::Settings => settings::render(f, &mut model.settings, chunks[1]),
        Tab::Workspaces => workspaces::render(f, &mut model.workspaces, chunks[1]),
        Tab::Query => query::render(f, &model.query, &model.jobs, chunks[1]),
        Tab::Jobs => jobs::render(f, &mut model.jobs, chunks[1]),
        Tab::Sessions => session::render(f, model, chunks[1]),
        Tab::Packs => packs::render(f, model, chunks[1]),
    }

    // Render controls bar
    controls::render(f, model.current_tab, chunks[2]);

    // Render popup if any
    if let Some(ref popup) = model.popup {
        popup::render(f, popup, model);
    }
}
