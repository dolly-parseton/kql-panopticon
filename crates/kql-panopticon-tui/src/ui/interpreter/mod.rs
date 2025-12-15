use crate::app::{App, CurrentScreen};
use ratatui::layout::{Constraint, Direction, Layout};

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Widget};
use ratatui::Frame;

mod interpreter_block;
mod title;

pub fn interpreter_ui(frame: &mut Frame, app: &crate::app::App) {
    // UI rendering logic for interpreter screen goes here
    let chunks = crate::layout::root(frame.area());

    // Title bar
    title::render_title(frame, app, chunks[0]);

    // Interpreter area - for rendering each block
    interpreter_block::render_interpreter(frame, app, chunks[1]);

    // Prompt
    let prompt_text = format!("> {}", app.prompt.buffer);
    let prompt = Paragraph::new(prompt_text).block(Block::default().borders(Borders::ALL));
    frame.render_widget(prompt, chunks[2]);
}
