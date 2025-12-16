use ratatui::widgets::{Block, Borders, Padding, Paragraph};
use ratatui::Frame;

pub fn render_interpreter(frame: &mut Frame, app: &crate::app::App, chunk: ratatui::layout::Rect) {
    // Outer container with left padding
    let interpreter_block = Block::default()
        .borders(Borders::NONE)
        .padding(Padding::left(1));
    let inner_area = interpreter_block.inner(chunk);
    frame.render_widget(interpreter_block, chunk);

    // Calculate how many lines each block takes (for now assume 1 line per block)
    const LINES_PER_BLOCK: usize = 1;

    let visible_lines = inner_area.height as usize;
    let total_lines = app.blocks.len() * LINES_PER_BLOCK;

    // Determine which blocks are visible
    let start_block = app.ui_state.scroll_offset / LINES_PER_BLOCK;
    let start_line_offset = app.ui_state.scroll_offset % LINES_PER_BLOCK;

    let mut y = 0u16;

    for (i, block) in app.blocks.iter().enumerate().skip(start_block) {
        if y >= inner_area.height {
            break; // No more room
        }

        let command_area = ratatui::layout::Rect {
            x: inner_area.x,
            y: inner_area.y + y,
            width: inner_area.width,
            height: 1, // Single line for now
        };

        // [status icon]➜ command
        let command_content = format!(" {}➜ {}", block.status.icon(), block.command);
        let command_para = Paragraph::new(command_content);
        frame.render_widget(command_para, command_area);

        let result_lines = block.results.lines().count();

        let results_area = ratatui::layout::Rect {
            x: inner_area.x,
            y: inner_area.y + y + 1,
            width: inner_area.width - 2,
            height: result_lines as u16, // Single line for now
        };

        let results_para = Paragraph::new(block.results.clone());
        frame.render_widget(results_para, results_area);

        y += LINES_PER_BLOCK as u16 + result_lines as u16 + 1; // +1 for spacing
    }
}
