use crate::app::App;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

mod interpreter;

pub fn ui(frame: &mut Frame, app: &App) {
    match app.current_screen {
        crate::app::CurrentScreen::Interpreter => interpreter::interpreter_ui(frame, app),
        _ => {}
    }
}
