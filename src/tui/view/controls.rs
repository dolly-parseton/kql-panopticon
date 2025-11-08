use crate::tui::message::Tab;
use ratatui::{
    layout::{Alignment, Rect},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the controls bar at the bottom
pub fn render(f: &mut Frame, current_tab: Tab, area: Rect) {
    let controls = match current_tab {
        Tab::Settings => {
            "1-6: Select Tab | Up/Down: Navigate | Enter: Edit | Tab: Next Tab | q: Quit"
        }
        Tab::Workspaces => {
            "1-6: Select Tab | Up/Down: Navigate | Space: Toggle | a: Select All | n: Select None | r: Refresh | Tab: Next Tab | q: Quit"
        }
        Tab::Query => {
            "1-6: Select Tab | i: INSERT mode | c: Clear | Ctrl+J: Execute | Tab: Next Tab | q: Quit"
        }
        Tab::Jobs => {
            "1-6: Select Tab | Up/Down: Navigate | Enter: View Details | r: Retry | c: Clear Completed | Tab: Next Tab | q: Quit"
        }
        Tab::Sessions => {
            "1-6: Select Tab | Up/Down: Navigate | s: Save | S: Save As | l: Load | d: Delete | p: Export as Pack | n: New | r: Refresh | Tab: Next Tab | q: Quit"
        }
        Tab::Packs => {
            "1-6: Select Tab | Up/Down: Navigate | Enter: Load Query | e: Execute Pack | r: Refresh | Tab: Next Tab | q: Quit"
        }
    };

    let paragraph = Paragraph::new(controls)
        .block(Block::default().borders(Borders::ALL).title("Controls"))
        .alignment(Alignment::Center);

    f.render_widget(paragraph, area);
}
