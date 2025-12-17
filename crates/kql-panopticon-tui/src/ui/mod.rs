use crate::app::{ActiveModal, App};
use ratatui::Frame;

mod interpreter;
mod modals;

pub fn ui(frame: &mut Frame, app: &App) {
    // Render base screen
    match app.current_screen {
        crate::app::CurrentScreen::Interpreter => interpreter::interpreter_ui(frame, app),
        _ => {}
    }

    // Render modal overlay if active
    match &app.active_modal {
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
        ActiveModal::None => {}
    }
}
