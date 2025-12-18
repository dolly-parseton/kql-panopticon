use crate::app::{ActiveModal, App};
use ratatui::widgets::{Block, Widget};
use ratatui::Frame;

mod completion_popup;
mod interpreter;
mod modals;

pub fn ui(frame: &mut Frame, app: &mut App) {
    // Fill entire frame with theme background color
    Block::default()
        .style(app.theme.background_style())
        .render(frame.area(), frame.buffer_mut());

    // Render base screen
    match app.current_screen {
        crate::app::CurrentScreen::Interpreter => interpreter::interpreter_ui(frame, app),
        _ => {}
    }

    // Render completion popup if active (before modals)
    if let Some(state) = &app.completion_state {
        let chunks = crate::layout::root(frame.area());
        let prompt_rect = chunks[2];
        let (_, cursor_col) = app.prompt.cursor();
        completion_popup::render(frame, state, prompt_rect, cursor_col, &app.theme);
    }

    // Render modal overlay if active
    match &mut app.active_modal {
        ActiveModal::WorkspaceSelector(state) => {
            modals::render_workspace_selector(frame, state, &app.theme);
        }
        ActiveModal::ThemeSelector(state) => {
            modals::render_theme_selector(frame, state, &app.theme);
        }
        ActiveModal::PackLoader(state) => {
            modals::render_pack_loader(frame, state, &app.theme);
        }
        ActiveModal::SettingsConfirmation => {
            modals::render_settings_confirmation(frame, &app.theme);
        }
        ActiveModal::Logs(state) => {
            modals::render_logs_modal(frame, state, &app.theme);
        }
        ActiveModal::None => {}
    }
}
