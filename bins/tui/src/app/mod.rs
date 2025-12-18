mod block;
mod connection;
mod execution;
pub mod logs;
mod pack_loader;
mod settings;
mod theme_selector;
mod ui_state;
mod workspace_selector;

pub use block::{BlockStatus, InterpreterBlock};
pub use connection::{ConnectionResult, ConnectionResultReceiver, ConnectionStatus};
pub use execution::{spawn_execution_task, PackExecutionReceiver, PackExecutionResult};
pub use logs::LogsState;
pub use pack_loader::PackLoaderState;
pub use settings::Settings;
pub use theme_selector::ThemeSelectorState;
pub use ui_state::UIState;
pub use workspace_selector::WorkspaceSelectorState;

use crate::completion::Suggestion;
use crate::theme::{Theme, ThemeManager};
use kql_panopticon::tracing::TuiEvent;
use kql_panopticon::{Client, Pack, Workspace};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tui_textarea::TextArea;

/// A loaded pack with its file path
#[derive(Debug, Clone)]
pub struct LoadedPack {
    pub pack: Arc<Pack>,
    pub path: PathBuf,
}

/// Completion state for tab completion
#[derive(Debug)]
pub struct CompletionState {
    /// Available suggestions
    pub suggestions: Vec<Suggestion>,
    /// Currently selected suggestion index
    pub selected: usize,
    /// Cursor position when completion was triggered
    pub trigger_position: usize,
    /// Whether the current suggestion is in preview mode
    pub preview_applied: bool,
    /// Original text before completion started
    pub original_text: String,
}

pub struct App<'a> {
    pub current_screen: CurrentScreen,
    pub focus: Focus,
    pub blocks: Vec<InterpreterBlock>,
    pub prompt: TextArea<'a>,
    pub command_history: Vec<String>,
    /// Current position in history navigation (None = new input)
    history_index: Option<usize>,
    /// Preserved input when navigating history
    saved_input: String,
    /// Completion state for tab completion (public for UI rendering)
    pub completion_state: Option<CompletionState>,
    pub ui_state: UIState,

    // Workspace management
    pub workspaces: Vec<Workspace>,
    pub selected_workspace_ids: HashSet<String>,

    // Pack management
    pub loaded_pack: Option<LoadedPack>,
    pub pack_dirs: Vec<String>,

    // User-defined inputs for pack execution
    pub inputs: HashMap<String, String>,

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

    // Azure connection state
    pub client: Option<Client>,
    pub connection_status: ConnectionStatus,
    connection_result_rx: Option<ConnectionResultReceiver>,
    connection_task: Option<tokio::task::JoinHandle<()>>,

    // Pack execution state
    pub pack_execution_rx: Option<PackExecutionReceiver>,
    pub active_execution_task: Option<tokio::task::JoinHandle<()>>,
    pub active_execution_block_id: Option<usize>,

    // Execution logs (FIFO, max 1000 events)
    pub execution_logs: Vec<TuiEvent>,
}

/// Active modal overlay
pub enum ActiveModal {
    None,
    WorkspaceSelector(WorkspaceSelectorState),
    ThemeSelector(ThemeSelectorState),
    PackLoader(PackLoaderState),
    SettingsConfirmation,
    Logs(LogsState),
}

pub enum CurrentScreen {
    /// Settings interface, todo
    Settings,
    /// Editor view, todo
    Editor,
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
        Self::with_settings(Settings::default())
    }

    /// Create new App instance with provided settings
    pub fn with_settings(settings: Settings) -> Self {
        let mut theme_manager = ThemeManager::default();
        let _ = theme_manager.discover();

        // Try to create sample theme file
        let _ = theme_manager.create_sample_theme();

        let mut theme = Theme::default();

        // Apply settings to theme
        if let Some(loaded_theme) = theme_manager.get(&settings.theme_name) {
            theme = loaded_theme.clone();
        }
        theme.override_terminal_background = settings.override_terminal_background;

        let mut prompt = TextArea::default();
        prompt.set_cursor_line_style(ratatui::style::Style::default());
        prompt.set_placeholder_text("Enter command...");
        prompt.set_cursor_style(theme.cursor_style());
        prompt.set_selection_style(theme.selection_style());

        // Initialize Azure client and spawn connection task
        let (client, connection_status, connection_result_rx, connection_task) =
            Self::init_azure_connection();

        Self {
            current_screen: CurrentScreen::Interpreter,
            focus: Focus::Prompt,
            blocks: Vec::new(),
            prompt,
            command_history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
            completion_state: None,
            ui_state: UIState::default(),
            workspaces: Vec::new(),
            selected_workspace_ids: HashSet::new(),
            loaded_pack: None,
            pack_dirs: settings.pack_directories.clone(),
            inputs: HashMap::new(),
            active_modal: ActiveModal::None,
            theme,
            theme_manager,
            backup_theme: None,
            initial_settings: settings,
            animation_start: std::time::Instant::now(),
            client,
            connection_status,
            connection_result_rx,
            connection_task,
            pack_execution_rx: None,
            active_execution_task: None,
            active_execution_block_id: None,
            execution_logs: Vec::new(),
        }
    }

    /// Initialize Azure client and spawn background connection task
    fn init_azure_connection() -> (
        Option<Client>,
        ConnectionStatus,
        Option<ConnectionResultReceiver>,
        Option<tokio::task::JoinHandle<()>>,
    ) {
        match Client::new() {
            Ok(client) => {
                // Spawn background task for auth validation + workspace discovery
                let (task_handle, result_rx) = connection::spawn_connection_task(client.clone());

                (
                    Some(client),
                    ConnectionStatus::Authenticating,
                    Some(result_rx),
                    Some(task_handle),
                )
            }
            Err(e) => {
                // Client creation failed (rare - usually HTTP client issues)
                let status = ConnectionStatus::Error {
                    message: format!("Failed to create client: {}", e),
                };
                (None, status, None, None)
            }
        }
    }

    /// Get current animation frame index based on elapsed time
    pub fn animation_frame(&self) -> usize {
        let elapsed_ms = self.animation_start.elapsed().as_millis() as u64;
        let frame_duration = self.theme.styles.spinners.speed_ms;
        (elapsed_ms / frame_duration) as usize
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Connection Management
    // ─────────────────────────────────────────────────────────────────────────────

    /// Poll for connection results from background task (non-blocking)
    pub fn poll_connection_results(&mut self) {
        // Collect all available messages first
        let mut messages = Vec::new();
        let mut channel_closed = false;

        if let Some(rx) = &mut self.connection_result_rx {
            loop {
                match rx.try_recv() {
                    Ok(result) => {
                        messages.push(result);
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                        // No more messages available
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        // Channel closed (task completed)
                        log::debug!("Connection channel closed");
                        channel_closed = true;
                        break;
                    }
                }
            }
        }

        // Now process all messages (no longer borrowing self.connection_result_rx)
        for result in messages {
            self.handle_connection_result(result);
        }

        // Clean up if channel closed
        if channel_closed {
            self.connection_result_rx = None;
        }
    }

    /// Handle a connection result from the background task
    fn handle_connection_result(&mut self, result: ConnectionResult) {
        match result {
            ConnectionResult::AuthSuccess => {
                self.connection_status = ConnectionStatus::Discovering;
            }
            ConnectionResult::AuthFailed(message) => {
                self.connection_status = ConnectionStatus::Error { message };
            }
            ConnectionResult::WorkspacesDiscovered(workspaces) => {
                let workspace_count = workspaces.len();
                self.workspaces = workspaces;
                self.connection_status = ConnectionStatus::Ready { workspace_count };
            }
            ConnectionResult::DiscoveryFailed(message) => {
                self.connection_status = ConnectionStatus::Error { message };
            }
        }
    }

    /// Check if connection is ready for workspace operations
    pub fn is_connection_ready(&self) -> bool {
        matches!(self.connection_status, ConnectionStatus::Ready { .. })
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Pack Execution Management
    // ─────────────────────────────────────────────────────────────────────────────

    /// Poll for pack execution results (non-blocking)
    /// Drains all available messages to ensure no updates are lost
    pub fn poll_pack_execution_results(&mut self) {
        // Collect all available messages first
        let mut messages = Vec::new();
        let mut channel_closed = false;

        if let Some(rx) = &mut self.pack_execution_rx {
            loop {
                match rx.try_recv() {
                    Ok(result) => {
                        messages.push(result);
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                        // No more messages available
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        // Channel closed (task completed)
                        log::debug!("Pack execution channel closed");
                        channel_closed = true;
                        break;
                    }
                }
            }
        }

        // Now process all messages (no longer borrowing self.pack_execution_rx)
        for result in messages {
            self.handle_pack_execution_result(result);
        }

        // Clean up if channel closed
        if channel_closed {
            self.pack_execution_rx = None;
        }
    }

    /// Handle pack execution result from background task
    fn handle_pack_execution_result(&mut self, result: PackExecutionResult) {
        let block_id = match self.active_execution_block_id {
            Some(id) => id,
            None => {
                log::warn!("Received pack execution result but no active execution block");
                return;
            }
        };

        let block = match self.blocks.get_mut(block_id) {
            Some(b) => b,
            None => {
                log::warn!(
                    "Received pack execution result for block {} but block doesn't exist",
                    block_id
                );
                return;
            }
        };

        match result {
            PackExecutionResult::Progress(message) => {
                // Only process progress if block is still running
                // (prevents race condition where progress arrives after completion)
                if matches!(block.status, BlockStatus::Running) {
                    block.append_results(&message);
                } else {
                    log::debug!(
                        "Ignoring progress update for block {} (status: {:?})",
                        block_id,
                        block.status
                    );
                }
            }
            PackExecutionResult::Completed(exec_result) => {
                // Format summary and mark as completed
                let summary = format_execution_result(&exec_result, &self.inputs);
                block.complete(summary);
                self.active_execution_block_id = None;
                log::info!("Pack execution completed for block {}", block_id);
            }
            PackExecutionResult::Failed(error) => {
                let error_msg = format!("Execution failed:\n{}", error);
                block.fail(error_msg);
                self.active_execution_block_id = None;
                log::error!("Pack execution failed for block {}: {}", block_id, error);
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────

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
        self.prompt
            .set_selection_style(self.theme.selection_style());
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
            self.theme.override_terminal_background,
        );
        self.active_modal = ActiveModal::ThemeSelector(state);

        // Preview the initially selected theme
        self.preview_selected_theme();
    }

    /// Close modal and apply theme selection
    pub fn confirm_theme_selection(&mut self) {
        let (theme_name, force_black_bg) =
            if let ActiveModal::ThemeSelector(state) = &self.active_modal {
                (state.selected_theme(), state.override_terminal_background())
            } else {
                (None, false)
            };

        if let Some(theme_name) = theme_name {
            let _ = self.set_theme(&theme_name);
            self.theme.override_terminal_background = force_black_bg;
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
        let (theme_name, force_black_bg) =
            if let ActiveModal::ThemeSelector(state) = &self.active_modal {
                (state.selected_theme(), state.override_terminal_background())
            } else {
                return;
            };

        if let Some(theme_name) = theme_name {
            // Apply theme temporarily (backup is already stored)
            let _ = self.set_theme(&theme_name);
            self.theme.override_terminal_background = force_black_bg;
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

                    // Inputs first (user needs to know what to provide)
                    if !pack.acquisition.inputs.is_empty() {
                        lines.push("".to_string());
                        lines.push(format!("Inputs ({}):", pack.acquisition.inputs.len()));
                        for input in &pack.acquisition.inputs {
                            let required = if input.required { " (required)" } else { "" };
                            lines.push(format!("  - {}{}", input.name, required));
                        }
                    }

                    // Acquisition steps
                    lines.push("".to_string());
                    lines.push(format!("Steps ({}):", pack.acquisition.steps.len()));
                    for step in &pack.acquisition.steps {
                        lines.push(format!("  - {}", step.name));
                    }

                    // Processing steps
                    if let Some(ref processing) = pack.processing {
                        if !processing.steps.is_empty() {
                            lines.push("".to_string());
                            lines.push(format!("Processing ({}):", processing.steps.len()));
                            for step in &processing.steps {
                                lines.push(format!("  - {}", step.name));
                            }
                        }
                    }

                    // Reporting definitions
                    if let Some(ref reporting) = pack.reporting {
                        if !reporting.reports.is_empty() {
                            lines.push("".to_string());
                            lines.push(format!("Reports ({}):", reporting.reports.len()));
                            for report in &reporting.reports {
                                lines.push(format!("  - {} ({:?})", report.name, report.format));
                            }
                        }
                    }

                    block.results = lines.join("\n");
                    block.status = BlockStatus::Completed;

                    // Store the loaded pack (wrapped in Arc for cheap cloning)
                    self.loaded_pack = Some(LoadedPack {
                        pack: Arc::new(pack),
                        path,
                    });
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

    /// Open the logs modal
    pub fn open_logs_modal(&mut self) {
        let state = LogsState::new(self.execution_logs.clone());
        self.active_modal = ActiveModal::Logs(state);
    }

    /// Close logs modal
    pub fn cancel_logs_modal(&mut self) {
        self.active_modal = ActiveModal::None;
    }

    /// Add a log event with FIFO eviction at MAX_LOG_EVENTS
    pub fn push_log_event(&mut self, event: TuiEvent) {
        use logs::MAX_LOG_EVENTS;
        if self.execution_logs.len() >= MAX_LOG_EVENTS {
            self.execution_logs.remove(0);
        }
        self.execution_logs.push(event);
    }

    /// Submit the current prompt content and return it
    pub fn submit_prompt(&mut self) -> Option<String> {
        let lines: Vec<&str> = self.prompt.lines().iter().map(|s| s.as_str()).collect();
        let content = lines.join("\n").trim().to_string();

        if content.is_empty() {
            return None;
        }

        // Add to history and reset navigation
        self.command_history.push(content.clone());
        self.history_index = None;

        // Clear the prompt
        self.prompt.select_all();
        self.prompt.cut();

        Some(content)
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // History Navigation
    // ─────────────────────────────────────────────────────────────────────────────

    /// Navigate to previous history entry (Up arrow)
    pub fn history_back(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        match self.history_index {
            None => {
                // Save current input before navigating
                self.saved_input = self.prompt_content();
                self.history_index = Some(self.command_history.len() - 1);
            }
            Some(0) => {
                // Already at oldest entry, do nothing
                return;
            }
            Some(i) => {
                self.history_index = Some(i - 1);
            }
        }

        // Set prompt to history entry
        if let Some(i) = self.history_index {
            let content = self.command_history[i].clone();
            self.set_prompt_content(&content);
        }
    }

    /// Navigate to next history entry (Down arrow)
    pub fn history_forward(&mut self) {
        match self.history_index {
            None => (), // Already at new input
            Some(i) => {
                if i + 1 >= self.command_history.len() {
                    // Return to new input
                    self.history_index = None;
                    let saved = self.saved_input.clone();
                    self.set_prompt_content(&saved);
                } else {
                    self.history_index = Some(i + 1);
                    let content = self.command_history[i + 1].clone();
                    self.set_prompt_content(&content);
                }
            }
        }
    }

    /// Reset history navigation (called on edit)
    pub fn history_reset(&mut self) {
        self.history_index = None;
    }

    /// Check if currently navigating history
    pub fn is_navigating_history(&self) -> bool {
        self.history_index.is_some()
    }

    /// Get current prompt content as String
    fn prompt_content(&self) -> String {
        self.prompt.lines().join("\n")
    }

    /// Set prompt content, replacing all text
    fn set_prompt_content(&mut self, content: &str) {
        self.prompt.select_all();
        self.prompt.cut();
        self.prompt.insert_str(content);
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Completion
    // ─────────────────────────────────────────────────────────────────────────────

    /// Trigger completion based on current cursor position
    pub fn trigger_completion(&mut self) {
        let content = self.prompt_content();
        let (_, col) = self.prompt.cursor();

        let suggestions = crate::completion::get_completions(
            &content,
            col,
            self.loaded_pack.as_ref(),
            &self.blocks,
        );

        if !suggestions.is_empty() {
            self.completion_state = Some(CompletionState {
                suggestions,
                selected: 0,
                trigger_position: col,
                preview_applied: false,
                original_text: content,
            });
            // Show first suggestion in preview
            self.show_completion_preview();
        }
    }

    /// Handle Tab key when completion is active
    pub fn handle_completion_tab(&mut self) {
        let should_accept = self
            .completion_state
            .as_ref()
            .map(|s| s.preview_applied)
            .unwrap_or(false);

        if should_accept {
            // Already previewing - accept this completion
            self.accept_completion();
        } else if let Some(state) = &mut self.completion_state {
            // First tab - mark as preview applied
            state.preview_applied = true;
        }
    }

    /// Check if completion is active
    pub fn has_completion(&self) -> bool {
        self.completion_state.is_some()
    }

    /// Cycle to next completion suggestion
    pub fn completion_next(&mut self) {
        if let Some(state) = &mut self.completion_state {
            state.selected = (state.selected + 1) % state.suggestions.len();
            state.preview_applied = false;
            self.show_completion_preview();
        }
    }

    /// Cycle to previous completion suggestion
    pub fn completion_prev(&mut self) {
        if let Some(state) = &mut self.completion_state {
            state.selected = if state.selected == 0 {
                state.suggestions.len() - 1
            } else {
                state.selected - 1
            };
            state.preview_applied = false;
            self.show_completion_preview();
        }
    }

    /// Dismiss completion and restore original text
    pub fn dismiss_completion(&mut self) {
        if let Some(state) = &self.completion_state {
            // Restore original text
            let original = state.original_text.clone();
            self.set_prompt_content(&original);
        }
        self.completion_state = None;
    }

    /// Accept current completion
    pub fn accept_completion(&mut self) {
        // Preview text is already in prompt, just clear completion state
        self.completion_state = None;
    }

    /// Show current completion suggestion as preview
    fn show_completion_preview(&mut self) {
        if let Some(state) = &self.completion_state {
            let suggestion = &state.suggestions[state.selected];
            let original = &state.original_text;

            // Build new content: original up to replace_start + suggestion value + rest
            let replace_start = suggestion.replace_start.min(original.len());
            let replace_end = suggestion.replace_end.min(original.len());

            let new_content = format!(
                "{}{}{}",
                &original[..replace_start],
                suggestion.value,
                &original[replace_end..]
            );
            self.set_prompt_content(&new_content);
        }
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

    // ─────────────────────────────────────────────────────────────────────────────
    // Scroll Management
    // ─────────────────────────────────────────────────────────────────────────────

    /// Calculate total height of all blocks in lines
    pub fn total_content_height(&self) -> usize {
        self.blocks.iter().map(|b| b.rendered_height()).sum()
    }

    /// Scroll to show the bottom of the content
    /// If content fits in viewport, scroll_offset stays at 0
    pub fn scroll_to_bottom(&mut self, viewport_height: usize) {
        let total = self.total_content_height();
        if total > viewport_height {
            self.ui_state.scroll_offset = total.saturating_sub(viewport_height);
        } else {
            self.ui_state.scroll_offset = 0;
        }
        self.ui_state.auto_scroll = true;
    }

    /// Update scroll position when new content is added
    /// Only scrolls if auto_scroll is enabled
    pub fn update_scroll_for_new_content(&mut self, viewport_height: usize) {
        if self.ui_state.auto_scroll {
            self.scroll_to_bottom(viewport_height);
        }
    }

    /// Check if currently scrolled to the bottom
    pub fn is_at_bottom(&self, viewport_height: usize) -> bool {
        let total = self.total_content_height();
        if total <= viewport_height {
            true
        } else {
            self.ui_state.scroll_offset >= total.saturating_sub(viewport_height)
        }
    }

    /// Get the maximum scroll offset
    pub fn max_scroll_offset(&self, viewport_height: usize) -> usize {
        self.total_content_height().saturating_sub(viewport_height)
    }

    /// Scroll up by the specified number of lines
    /// Disables auto_scroll when scrolling up
    pub fn scroll_up(&mut self, lines: usize) {
        if self.ui_state.scroll_offset > 0 {
            self.ui_state.scroll_offset = self.ui_state.scroll_offset.saturating_sub(lines);
            self.ui_state.auto_scroll = false;
        }
    }

    /// Scroll down by the specified number of lines
    /// Re-enables auto_scroll if we reach the bottom
    pub fn scroll_down(&mut self, lines: usize) {
        let max_offset = self.max_scroll_offset(self.ui_state.viewport_height);
        self.ui_state.scroll_offset = (self.ui_state.scroll_offset + lines).min(max_offset);

        // Re-enable auto_scroll if we've scrolled to bottom
        if self.ui_state.scroll_offset >= max_offset {
            self.ui_state.auto_scroll = true;
        }
    }

    /// Scroll up by one page
    pub fn scroll_page_up(&mut self) {
        let page_size = self.ui_state.viewport_height.saturating_sub(2); // Leave some overlap
        self.scroll_up(page_size.max(1));
    }

    /// Scroll down by one page
    pub fn scroll_page_down(&mut self) {
        let page_size = self.ui_state.viewport_height.saturating_sub(2);
        self.scroll_down(page_size.max(1));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Format PackExecutorResult into human-readable summary
fn format_execution_result(
    result: &kql_panopticon::execution::PackExecutorResult,
    inputs: &std::collections::HashMap<String, String>,
) -> String {
    use kql_panopticon::execution::{ExecutionPhase, ExecutionStatus, StepStatus};

    let mut lines = vec![
        format!("✓ Pack execution complete: {}", result.pack_name),
        format!("Status: {}", result.status),
        format!("Duration: {}ms", result.duration_ms),
        format!(
            "Workspaces: {} succeeded, {} failed",
            result.success_count(),
            result.failure_count()
        ),
    ];

    // Show input values used for this execution
    if !inputs.is_empty() {
        lines.push(String::new());
        lines.push("Inputs:".to_string());
        for (name, value) in inputs {
            let display_value = if value.len() > 25 {
                format!("{}...", &value[..25])
            } else {
                value.clone()
            };
            lines.push(format!("  {} = {}", name, display_value));
        }
    }

    lines.push(String::new());

    if let Some(ref dir) = result.output_dir {
        lines.push(format!("Output: {}", dir.display()));
        lines.push(String::new());
    }

    lines.push("Per-Workspace Results:".to_string());

    for (ws_name, ws_result) in &result.workspace_results {
        let icon = match ws_result.status {
            ExecutionStatus::Success => "✓",
            ExecutionStatus::Failed => "✗",
            ExecutionStatus::Partial => "⚠",
            _ => "•",
        };

        lines.push(format!(
            "  {} {} ({}ms)",
            icon, ws_name, ws_result.duration_ms
        ));

        // Group steps by phase
        let acquisition_steps: Vec<_> = ws_result
            .step_results
            .values()
            .filter(|s| s.phase == ExecutionPhase::Acquisition)
            .collect();
        let processing_steps: Vec<_> = ws_result
            .step_results
            .values()
            .filter(|s| s.phase == ExecutionPhase::Processing)
            .collect();
        let reporting_steps: Vec<_> = ws_result
            .step_results
            .values()
            .filter(|s| s.phase == ExecutionPhase::Reporting)
            .collect();

        // Helper to format a step
        let format_step = |step: &kql_panopticon::execution::StepResult| -> Vec<String> {
            let mut step_lines = Vec::new();
            let step_icon = if step.status == StepStatus::Success {
                "+"
            } else {
                "x"
            };
            let rows = step
                .row_count
                .map(|n| format!(" ({} rows)", n))
                .unwrap_or_default();
            step_lines.push(format!("        [{}] {}{}", step_icon, step.name, rows));

            if let Some(ref err) = step.error {
                step_lines.push(format!("            Error: {}", err));
            }
            step_lines
        };

        // Render each phase
        if !acquisition_steps.is_empty() {
            lines.push("    Acquisition:".to_string());
            for step in acquisition_steps {
                lines.extend(format_step(step));
            }
        }

        if !processing_steps.is_empty() {
            lines.push("    Processing:".to_string());
            for step in processing_steps {
                lines.extend(format_step(step));
            }
        }

        if !reporting_steps.is_empty() {
            lines.push("    Reporting:".to_string());
            for step in reporting_steps {
                lines.extend(format_step(step));
            }
        }
    }

    lines.join("\n")
}
