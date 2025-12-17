use ratatui::widgets::{Block, Borders};
use ratatui::Frame;

mod interpreter_block;
mod title;

pub fn interpreter_ui(frame: &mut Frame, app: &crate::app::App) {
    // Render background
    let bg_block = Block::default().style(app.theme.background_style());
    frame.render_widget(bg_block, frame.area());

    let chunks = crate::layout::root(frame.area());

    // Title bar
    title::render_title(frame, app, chunks[0]);

    // Interpreter area - for rendering each block
    interpreter_block::render_interpreter(frame, app, chunks[1]);

    // Prompt - render TextArea with border
    let prompt_block = Block::default()
        .borders(Borders::ALL)
        .border_type(app.theme.border_type())
        .border_style(app.theme.prompt_border_style())
        .style(app.theme.background_style())
        .title(" Input ");
    let prompt_area = prompt_block.inner(chunks[2]);
    frame.render_widget(prompt_block, chunks[2]);
    frame.render_widget(&app.prompt, prompt_area);
}
