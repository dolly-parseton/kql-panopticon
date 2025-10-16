use crate::tui::model::settings::SettingsModel;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

/// Render the Settings tab
pub fn render(f: &mut Frame, model: &mut SettingsModel, area: Rect) {
    let settings_items = model.get_all_settings();

    let items: Vec<ListItem> = settings_items
        .iter()
        .enumerate()
        .map(|(i, setting)| {
            let style = if Some(i) == model.list_state.selected() {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(setting.as_str()).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Settings"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(list, area, &mut model.list_state);
}
