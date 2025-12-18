use std::rc::Rc;
use std::str::Utf8Chunks;

use crate::app::{App, CurrentScreen};
use ratatui::layout::{Constraint, Direction, Layout};

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Widget};
use ratatui::Frame;

pub fn render_prompt(frame: &mut Frame, app: &crate::app::App, chunk: ratatui::layout::Rect) {
    let prompt_area = Layout::default()
        .direction(Direction::Horizontal)
        .horizontal_margin(1)
        .constraints([Constraint::Min(0)])
        .split(chunk)[0];

    let prompt_text = format!("> {}", app.prompt.buffer);
    let prompt = Paragraph::new(prompt_text).block(Block::default().borders(Borders::ALL));
    frame.render_widget(prompt, prompt_area);
}
