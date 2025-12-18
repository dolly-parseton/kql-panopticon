use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn root(frame: Rect) -> [Rect; 3] {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title bar
            Constraint::Min(1),    // Blocks area (flexible)
            Constraint::Length(3), // Prompt
        ])
        .split(frame);
    [chunks[0], chunks[1], chunks[2]]
}

// pub fn interpreter_area(chunk: Rect) -> Rect {}
