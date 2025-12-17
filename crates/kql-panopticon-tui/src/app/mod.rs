mod block;
mod ui_state;
mod workspace_selector;
mod theme_selector;
mod settings;
mod pack_loader;

pub use block::{BlockStatus, InterpreterBlock};
pub use ui_state::UIState;
pub use workspace_selector::WorkspaceSelectorState;
pub use theme_selector::ThemeSelectorState;
pub use settings::Settings;
pub use pack_loader::PackLoaderState;

use crate::theme::{Theme, ThemeManager};
use kql_panopticon_core::{Pack, Workspace};
use std::collections::HashSet;
use std::path::PathBuf;
use tui_textarea::TextArea;

/// A loaded pack with its file path
#[derive(Debug, Clone)]
pub struct LoadedPack {
    pub pack: Pack,
    pub path: PathBuf,
}

pub struct App<'a> {
    pub current_screen: CurrentScreen,
    pub focus: Focus,
    pub blocks: Vec<InterpreterBlock>,
    pub prompt: TextArea<'a>,
    pub command_history: Vec<String>,
    pub ui_state: UIState,

    // Workspace management
    pub workspaces: Vec<Workspace>,
    pub selected_workspace_ids: HashSet<String>,

    // Pack management
    pub loaded_pack: Option<LoadedPack>,
    pub pack_dirs: Vec<String>,

    // Modal state
    pub active_modal: ActiveModal,

    // Theming
    pub theme: Theme,
    pub theme_manager: ThemeManager,
    backup_theme: Option<Theme>,

    // Settings
    pub initial_settings: Settings,

    // Animation state
    animation_start: std::time::Instant,
}

/// Active modal overlay
pub enum ActiveModal {
    None,
    WorkspaceSelector(WorkspaceSelectorState),
    ThemeSelector(ThemeSelectorState),
    PackLoader(PackLoaderState),
    SettingsConfirmation,
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

impl<'a> App<'a> {
    /// Create new App instance with default settings
    pub fn new() -> Self {
        Self::with_settings(Settings::default(), Vec::new())
    }

    /// Create new App instance with provided settings and pack directories
    pub fn with_settings(settings: Settings, pack_dirs: Vec<String>) -> Self {
        let mut theme_manager = ThemeManager::default();
        let _ = theme_manager.discover();

        // Try to create sample theme file
        let _ = theme_manager.create_sample_theme();

        let mut theme = Theme::default();

        // Apply settings to theme
        if let Some(loaded_theme) = theme_manager.get(&settings.theme_name) {
            theme = loaded_theme.clone();
        }
        theme.force_black_background = settings.force_black_background;

        let mut prompt = TextArea::default();
        prompt.set_cursor_line_style(ratatui::style::Style::default());
        prompt.set_placeholder_text("Enter command...");
        prompt.set_cursor_style(theme.cursor_style());
        prompt.set_selection_style(theme.selection_style());

        Self {
            current_screen: CurrentScreen::Interpreter,
            focus: Focus::Prompt,
            blocks: Vec::new(),
            prompt,
            command_history: Vec::new(),
            ui_state: UIState { scroll_offset: 0 },
            workspaces: Vec::new(),
            selected_workspace_ids: HashSet::new(),
            loaded_pack: None,
            pack_dirs,
            active_modal: ActiveModal::None,
            theme,
            theme_manager,
            backup_theme: None,
            initial_settings: settings,
            animation_start: std::time::Instant::now(),
        }
    }

    /// Get current animation frame index based on elapsed time
    pub fn animation_frame(&self) -> usize {
        let elapsed_ms = self.animation_start.elapsed().as_millis() as u64;
        let frame_duration = self.theme.styles.spinners.speed_ms;
        (elapsed_ms / frame_duration) as usize
    }

    pub fn set_exit_screen(&mut self) {
        self.current_screen = CurrentScreen::Exiting(std::time::Instant::now());
        self.focus = Focus::None;
    }

    pub fn should_exit(&self) -> bool {
        matches!(self.current_screen, CurrentScreen::Exiting(instant) if instant.elapsed() >= std::time::Duration::from_millis(100))
    }

    /// Check if a modal is active
    pub fn has_modal(&self) -> bool {
        !matches!(self.active_modal, ActiveModal::None)
    }

    /// Open the workspace selector modal
    pub fn open_workspace_selector(&mut self) {
        let state = WorkspaceSelectorState::new(
            self.workspaces.clone(),
            self.selected_workspace_ids.clone(),
        );
        self.active_modal = ActiveModal::WorkspaceSelector(state);
    }

    /// Close modal and apply workspace selection, creating a result block
    pub fn confirm_workspace_selection(&mut self) {
        if let ActiveModal::WorkspaceSelector(state) = &self.active_modal {
            self.selected_workspace_ids = state.selected.clone();

            // Build result string showing selected workspaces
            let selected_ws: Vec<&Workspace> = self
                .workspaces
                .iter()
                .filter(|ws| self.selected_workspace_ids.contains(&ws.workspace_id))
                .collect();

            let results = if selected_ws.is_empty() {
                "No workspaces selected".to_string()
            } else {
                let mut lines = vec![format!("Selected {} workspace(s):", selected_ws.len())];
                for ws in &selected_ws {
                    lines.push(format!("  - {} ({})", ws.name, ws.subscription_name));
                }
                lines.join("\n")
            };

            // Create a block showing the selection
            let block_id = self.blocks.len();
            let mut block = InterpreterBlock::new(block_id, ":ws".to_string());
            block.results = results;
            block.status = BlockStatus::Completed;
            self.blocks.push(block);
        }
        self.active_modal = ActiveModal::None;
    }

    /// Close modal and revert workspace selection
    pub fn cancel_workspace_selection(&mut self) {
        if let ActiveModal::WorkspaceSelector(state) = &mut self.active_modal {
            state.cancel();
        }
        self.active_modal = ActiveModal::None;
    }

    /// Get selected workspaces
    pub fn get_selected_workspaces(&self) -> Vec<&Workspace> {
        self.workspaces
            .iter()
            .filter(|ws| self.selected_workspace_ids.contains(&ws.workspace_id))
            .collect()
    }

    /// Apply a theme to the application
    fn apply_theme(&mut self, theme: Theme) {
        self.theme = theme;
        // Update TextArea styles to match theme
        self.prompt.set_cursor_style(self.theme.cursor_style());
        self.prompt.set_selection_style(self.theme.selection_style());
    }

    /// Switch to a different theme by name
    pub fn set_theme(&mut self, name: &str) -> Result<(), String> {
        if let Some(theme) = self.theme_manager.get(name) {
            self.apply_theme(theme.clone());
            Ok(())
        } else {
            Err(format!("Theme '{}' not found", name))
        }
    }

    /// List available themes
    pub fn list_themes(&self) -> Vec<String> {
        self.theme_manager.list()
    }

    /// Open the theme selector modal
    pub fn open_theme_selector(&mut self) {
        // Store backup theme for cancel/preview
        self.backup_theme = Some(self.theme.clone());

        // Reload all themes from disk (with soft validation)
        let _ = self.theme_manager.discover();

        let themes_by_category = self.theme_manager.list_by_category();
        let state = ThemeSelectorState::new(
            themes_by_category,
            &self.theme.name,
            self.theme.force_black_background,
        );
        self.active_modal = ActiveModal::ThemeSelector(state);

        // Preview the initially selected theme
        self.preview_selected_theme();
    }

    /// Close modal and apply theme selection
    pub fn confirm_theme_selection(&mut self) {
        let (theme_name, force_black_bg) = if let ActiveModal::ThemeSelector(state) = &self.active_modal {
            (
                state.selected_theme(),
                state.force_black_background(),
            )
        } else {
            (None, false)
        };

        if let Some(theme_name) = theme_name {
            let _ = self.set_theme(&theme_name);
            self.theme.force_black_background = force_black_bg;
        }

        // Clear backup - we're keeping this theme
        self.backup_theme = None;
        self.active_modal = ActiveModal::None;
    }

    /// Close modal without applying theme selection
    pub fn cancel_theme_selection(&mut self) {
        // Restore backup theme
        if let Some(backup) = self.backup_theme.take() {
            self.apply_theme(backup);
        }
        self.active_modal = ActiveModal::None;
    }

    /// Preview the currently selected theme in the modal (for hover effect)
    pub fn preview_selected_theme(&mut self) {
        let (theme_name, force_black_bg) = if let ActiveModal::ThemeSelector(state) = &self.active_modal {
            (
                state.selected_theme(),
                state.force_black_background(),
            )
        } else {
            return;
        };

        if let Some(theme_name) = theme_name {
            // Apply theme temporarily (backup is already stored)
            let _ = self.set_theme(&theme_name);
            self.theme.force_black_background = force_black_bg;
        }
    }

    /// Open the pack loader modal
    pub fn open_pack_loader(&mut self) {
        let state = PackLoaderState::new(self.pack_dirs.clone());
        self.active_modal = ActiveModal::PackLoader(state);
    }

    /// Close modal and load selected pack, creating a result block
    pub fn confirm_pack_selection(&mut self) {
        let pack_path = if let ActiveModal::PackLoader(state) = &self.active_modal {
            state.selected_pack_path()
        } else {
            None
        };

        if let Some(path) = pack_path {
            let block_id = self.blocks.len();
            let mut block = InterpreterBlock::new(block_id, ":load".to_string());

            // Attempt to load the pack using core API
            match Pack::load(&path) {
                Ok(pack) => {
                    // Build summary of pack
                    let mut lines = vec![format!("Loaded pack: {}", pack.name)];

                    if let Some(desc) = &pack.description {
                        lines.push(format!("Description: {}", desc));
                    }

                    if let Some(version) = &pack.version {
                        lines.push(format!("Version: {}", version));
                    }

                    lines.push("".to_string());
                    lines.push(format!("Steps ({}):", pack.acquisition.steps.len()));
                    for step in &pack.acquisition.steps {
                        lines.push(format!("  - {}", step.name));
                    }

                    if !pack.acquisition.inputs.is_empty() {
                        lines.push("".to_string());
                        lines.push(format!("Inputs ({}):", pack.acquisition.inputs.len()));
                        for input in &pack.acquisition.inputs {
                            let required = if input.required { " (required)" } else { "" };
                            lines.push(format!("  - {}{}", input.name, required));
                        }
                    }

                    block.results = lines.join("\n");
                    block.status = BlockStatus::Completed;

                    // Store the loaded pack
                    self.loaded_pack = Some(LoadedPack { pack, path });
                }
                Err(e) => {
                    block.results = format!("Failed to load pack: {}", e);
                    block.status = BlockStatus::Failed;
                }
            }

            self.blocks.push(block);
        }

        self.active_modal = ActiveModal::None;
    }

    /// Close modal without loading a pack
    pub fn cancel_pack_selection(&mut self) {
        self.active_modal = ActiveModal::None;
    }

    /// Submit the current prompt content and return it
    pub fn submit_prompt(&mut self) -> Option<String> {
        let lines: Vec<&str> = self.prompt.lines().iter().map(|s| s.as_str()).collect();
        let content = lines.join("\n").trim().to_string();

        if content.is_empty() {
            return None;
        }

        // Add to history
        self.command_history.push(content.clone());

        // Clear the prompt
        self.prompt.select_all();
        self.prompt.cut();

        Some(content)
    }

    /// Check if current settings have changed from initial settings
    pub fn settings_have_changed(&self) -> bool {
        let current = Settings::read_from(self);
        current.has_changed_from(&self.initial_settings)
    }

    /// Show settings confirmation modal (for when user presses ESC with unsaved changes)
    pub fn show_settings_confirmation(&mut self) {
        self.active_modal = ActiveModal::SettingsConfirmation;
    }

    /// Save current settings and exit
    pub fn save_settings_and_exit(&mut self) {
        let current = Settings::read_from(self);
        if let Err(e) = current.save() {
            log::warn!("Failed to save settings: {}", e);
        }
        self.set_exit_screen();
    }

    /// Exit without saving settings
    pub fn quit_without_saving(&mut self) {
        self.set_exit_screen();
    }

    /// Handle ESC key press - shows confirmation if settings changed, otherwise exits
    pub fn handle_escape(&mut self) {
        if self.settings_have_changed() {
            self.show_settings_confirmation();
        } else {
            self.set_exit_screen();
        }
    }
}
