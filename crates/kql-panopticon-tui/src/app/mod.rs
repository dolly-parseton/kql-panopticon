mod block;
mod prompt;
mod ui_state;

pub use block::InterpreterBlock;
pub use prompt::PromptState;
pub use ui_state::UIState;

pub struct App {
    pub current_screen: CurrentScreen,
    pub focus: Focus,
    pub blocks: Vec<InterpreterBlock>,
    pub prompt: prompt::PromptState,

    pub ui_state: UIState,
}

pub enum CurrentScreen {
    /// Settings interface
    Settings,
    /// Main interpreter interface
    Interpreter,
    /// Exiting the application
    Exiting(std::time::Instant),
}

pub enum Focus {
    /// No focused element
    None,
    /// Focused on the command input
    Prompt,
    /// Focused on a block (by id)
    Block(usize),
}

// Methods for App
impl App {
    /// Create new App instance
    pub fn new() -> Self {
        Self {
            current_screen: CurrentScreen::Interpreter,
            focus: Focus::None,
            blocks: Vec::new(),
            prompt: prompt::PromptState::default(),
            ui_state: UIState { scroll_offset: 0 },
        }
    }

    pub fn set_exit_screen(&mut self) {
        self.current_screen = CurrentScreen::Exiting(std::time::Instant::now());
        self.focus = Focus::None;
    }

    pub fn should_exit(&self) -> bool {
        matches!(self.current_screen, CurrentScreen::Exiting(instant) if instant.elapsed() >= std::time::Duration::from_millis(100))
    }
}
