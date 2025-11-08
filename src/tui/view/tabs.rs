use crate::tui::message::Tab;
use crate::tui::model::InitState;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the tab bar with loading spinner
pub fn render(
    f: &mut Frame,
    current_tab: Tab,
    init_state: InitState,
    spinner_frame: usize,
    area: Rect,
) {
    let tabs = [Tab::Query, Tab::Packs, Tab::Workspaces, Tab::Settings, Tab::Jobs, Tab::Sessions];
    let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

    let tab_spans: Vec<Span> = tabs
        .iter()
        .map(|tab| {
            let style = if *tab == current_tab {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(Color::White)
            };

            // Add spinner to Workspaces tab when initializing
            let text = if *tab == Tab::Workspaces && init_state == InitState::Initializing {
                let spinner = spinner_chars[spinner_frame % spinner_chars.len()];
                format!(" {} {} ", tab.as_str(), spinner)
            } else {
                format!(" {} ", tab.as_str())
            };

            Span::styled(text, style)
        })
        .collect();

    let tabs_line = Line::from(tab_spans);
    let tabs_paragraph = Paragraph::new(tabs_line).block(
        Block::default()
            .borders(Borders::ALL)
            .title("KQL Panopticon"),
    );

    f.render_widget(tabs_paragraph, area);
}
