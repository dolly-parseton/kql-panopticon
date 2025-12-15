use std::rc::Rc;
use std::str::Utf8Chunks;

use crate::app::{App, CurrentScreen};
use ratatui::layout::{Constraint, Direction, Layout};

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Widget};
use ratatui::Frame;

pub fn render_title(frame: &mut Frame, app: &crate::app::App, chunk: ratatui::layout::Rect) {
    let title_area = Layout::default()
        .direction(Direction::Horizontal)
        .horizontal_margin(1)
        .constraints([Constraint::Min(0)])
        .split(chunk)[0];

    frame.render_widget(
        Paragraph::new(format!(
            "kql-panopticon{}",
            match app.current_screen {
                CurrentScreen::Interpreter => " (Interpreter)",
                CurrentScreen::Settings => " (Settings)",
                _ => "",
            },
        ))
        .block(Block::default().padding(Padding::horizontal(1)))
        .block(Block::default().borders(Borders::ALL)),
        title_area,
    );
}
