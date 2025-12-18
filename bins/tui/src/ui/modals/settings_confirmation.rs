//! Settings confirmation modal rendering

use crate::theme::Theme;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Render the settings confirmation modal as a centered popup
pub fn render_settings_confirmation(frame: &mut Frame, theme: &Theme) {
    let area = centered_rect(40, 25, frame.area());

    // Clear the area behind the modal
    frame.render_widget(Clear, area);

    // Create main block with border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme.border_type())
        .border_style(theme.border_style())
        .style(theme.modal_overlay_style())
        .title(" Unsaved Settings ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner area vertically: top space, S button, Q button, bottom space
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top space
            Constraint::Length(1), // S button
            Constraint::Length(1), // Q button
            Constraint::Length(1), // Bottom space
        ])
        .split(inner);

    // Save button
    let save_button = Line::from(vec![
        Span::raw(" "), // 1 space padding from left border
        Span::styled(
            "[S]",
            Style::default()
                .fg(theme.colors.success.clone().into())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Save & Quit"),
    ]);
    let save_para = Paragraph::new(save_button)
        .alignment(Alignment::Left)
        .style(theme.text_style());
    frame.render_widget(save_para, chunks[1]);

    // Quit button
    let quit_button = Line::from(vec![
        Span::raw(" "), // 1 space padding from left border
        Span::styled(
            "[Q]",
            Style::default()
                .fg(theme.colors.error.clone().into())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Quit Without Saving"),
    ]);
    let quit_para = Paragraph::new(quit_button)
        .alignment(Alignment::Left)
        .style(theme.text_style());
    frame.render_widget(quit_para, chunks[2]);
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
