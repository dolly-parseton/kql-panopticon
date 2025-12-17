use crate::app::CurrentScreen;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Padding, Paragraph};
use ratatui::Frame;

pub fn render_title(frame: &mut Frame, app: &crate::app::App, chunk: ratatui::layout::Rect) {
    let title_area = Layout::default()
        .direction(Direction::Horizontal)
        .horizontal_margin(1)
        .constraints([Constraint::Min(0)])
        .split(chunk)[0];

    let view_name = match app.current_screen {
        CurrentScreen::Interpreter => "Interpreter",
        CurrentScreen::Settings => "Settings",
        _ => "",
    };

    // Get styled title spans
    let title_spans = app.theme.format_title_styled(view_name);
    let title_line = Line::from(title_spans);

    frame.render_widget(
        Paragraph::new(title_line)
            .block(
                Block::default()
                    .padding(Padding::horizontal(1))
                    .borders(Borders::ALL)
                    .border_type(app.theme.border_type())
                    .border_style(app.theme.border_style())
                    .style(app.theme.background_style()),
            ),
        title_area,
    );
}
