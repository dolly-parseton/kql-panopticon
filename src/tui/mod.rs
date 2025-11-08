pub mod message;
pub mod model;
pub mod update;
pub mod view;

use crate::client::Client;
use crate::error::Result;
use message::{Message, Tab};
use model::{query::EditorMode, Model};
use ratatui::crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

/// Main TUI entry point
pub async fn run_tui(client: Client) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Check minimum terminal size
    let size = terminal.size()?;
    if size.width < 80 || size.height < 24 {
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        return Err(crate::error::KqlPanopticonError::Other(format!(
            "Terminal too small. Minimum size: 80x24, current: {}x{}",
            size.width, size.height
        )));
    }

    // Create model
    let mut model = Model::new(client.clone());

    // Create a channel for initialization messages
    let (init_tx, mut init_rx) = tokio::sync::mpsc::unbounded_channel::<message::Message>();

    // Start async initialization in background
    let init_client = client;
    let tx = init_tx.clone();
    tokio::spawn(async move {
        // Load sessions from disk (no async needed)
        let _ = tx.send(message::Message::SessionsRefresh);

        // Authenticate and load workspaces
        match init_client.force_validate_auth().await {
            Ok(_) => {
                let _ = tx.send(message::Message::AuthCompleted);

                // Now load workspaces
                match init_client.list_workspaces().await {
                    Ok(workspaces) => {
                        let _ = tx.send(message::Message::WorkspacesLoaded(workspaces));
                        let _ = tx.send(message::Message::InitCompleted);
                    }
                    Err(e) => {
                        let _ = tx.send(message::Message::ShowError(format!(
                            "Failed to load workspaces: {}",
                            e
                        )));
                        let _ = tx.send(message::Message::InitCompleted);
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(message::Message::AuthFailed(e.to_string()));
            }
        }
    });

    // Run the application loop with init channel
    let result = run_app(&mut terminal, &mut model, &mut init_rx).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Main application loop
async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    model: &mut Model,
    init_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Message>,
) -> Result<()> {
    loop {
        // Process any pending job updates
        model.process_job_updates();

        // Process any init messages
        while let Ok(msg) = init_rx.try_recv() {
            // Handle SessionsRefresh specially (like in main loop)
            if matches!(msg, Message::SessionsRefresh) {
                match crate::session::Session::list_all() {
                    Ok(sessions) => {
                        model.sessions.refresh_from_disk(sessions);
                    }
                    Err(e) => {
                        log::error!("Failed to refresh sessions during init: {}", e);
                    }
                }
                continue;
            }

            let new_messages = update::update(model, msg);
            for new_msg in new_messages {
                let _ = update::update(model, new_msg);
            }
        }

        // Increment spinner frame for loading animation
        if model.init_state == model::InitState::Initializing {
            model.spinner_frame = model.spinner_frame.wrapping_add(1);
        }

        terminal.draw(|f| view::ui(f, model))?;

        // Handle events with timeout (50ms for smooth spinner animation)
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                let message = handle_key_event(key.code, key.modifiers, model);

                // Process the message and any subsequent messages
                let mut messages_to_process = vec![message];
                while let Some(msg) = messages_to_process.pop() {
                    // Check for quit
                    if matches!(msg, Message::Quit) {
                        return Ok(());
                    }

                    // Handle workspace refresh (async operation)
                    if matches!(msg, Message::WorkspacesRefresh) {
                        match model.client.list_workspaces().await {
                            Ok(workspaces) => {
                                messages_to_process.push(Message::WorkspacesLoaded(workspaces));
                            }
                            Err(e) => {
                                messages_to_process.push(Message::ShowError(format!(
                                    "Failed to refresh workspaces: {}",
                                    e
                                )));
                            }
                        }
                        continue;
                    }

                    // Handle sessions refresh (load from disk)
                    if matches!(msg, Message::SessionsRefresh) {
                        match crate::session::Session::list_all() {
                            Ok(sessions) => {
                                model.sessions.refresh_from_disk(sessions);
                            }
                            Err(e) => {
                                messages_to_process.push(Message::ShowError(format!(
                                    "Failed to refresh sessions: {}",
                                    e
                                )));
                            }
                        }
                        continue;
                    }

                    // Update model and collect new messages
                    let new_messages = update::update(model, msg);
                    messages_to_process.extend(new_messages);
                }
            }
        }
    }
}

/// Convert key events into messages
fn handle_key_event(key: KeyCode, modifiers: KeyModifiers, model: &Model) -> Message {
    // Handle popup interactions first
    if let Some(popup) = &model.popup {
        return handle_popup_key(key, popup, model);
    }

    // Check if we're in query edit mode (blocks most global keys)
    let in_query_edit_mode = model.current_tab == Tab::Query
        && (model.query.mode == EditorMode::Insert || model.query.mode == EditorMode::Visual);

    // Handle global keys (only work outside query edit mode)
    if !in_query_edit_mode {
        match key {
            KeyCode::Char('q') => return Message::Quit,
            KeyCode::Char('r') => {
                if model.current_tab == Tab::Workspaces {
                    return Message::WorkspacesRefresh;
                } else if model.current_tab == Tab::Sessions {
                    return Message::SessionsRefresh;
                }
            }
            KeyCode::Char('1') => return Message::SwitchTab(Tab::Query),
            KeyCode::Char('2') => return Message::SwitchTab(Tab::Workspaces),
            KeyCode::Char('3') => return Message::SwitchTab(Tab::Settings),
            KeyCode::Char('4') => return Message::SwitchTab(Tab::Jobs),
            KeyCode::Char('5') => return Message::SwitchTab(Tab::Sessions),
            _ => {}
        }
    }

    // Tab key always works
    if key == KeyCode::Tab {
        if modifiers.contains(KeyModifiers::SHIFT) {
            return Message::SwitchTab(model.current_tab.previous());
        } else {
            return Message::SwitchTab(model.current_tab.next());
        }
    }

    // Ctrl+J for query execution (works in any mode)
    if modifiers.contains(KeyModifiers::CONTROL)
        && key == KeyCode::Char('j')
        && model.current_tab == Tab::Query
    {
        return Message::QueryStartExecution;
    }

    // Handle tab-specific keys
    match model.current_tab {
        Tab::Settings => handle_settings_key(key),
        Tab::Workspaces => handle_workspaces_key(key),
        Tab::Query => handle_query_key(key, modifiers, model),
        Tab::Jobs => handle_jobs_key(key),
        Tab::Sessions => handle_sessions_key(key, modifiers),
    }
}

/// Handle key events when a popup is open
fn handle_popup_key(key: KeyCode, popup: &model::Popup, model: &Model) -> Message {
    match popup {
        model::Popup::Error(_) => {
            if matches!(key, KeyCode::Esc | KeyCode::Enter) {
                Message::ClosePopup
            } else {
                Message::NoOp
            }
        }
        model::Popup::SettingsEdit => match key {
            KeyCode::Esc => Message::SettingsCancel,
            KeyCode::Enter => Message::SettingsSave,
            KeyCode::Backspace => Message::SettingsInputBackspace,
            KeyCode::Char(c) => Message::SettingsInputChar(c),
            _ => Message::NoOp,
        },
        model::Popup::JobNameInput => match key {
            KeyCode::Esc => Message::ClosePopup,
            KeyCode::Enter => {
                if let Some(ref job_name) = model.query.job_name_input {
                    if !job_name.trim().is_empty() {
                        return Message::ExecuteQuery(job_name.clone());
                    }
                }
                Message::ClosePopup
            }
            KeyCode::Backspace => Message::JobNameInputBackspace,
            KeyCode::Char(c) => Message::JobNameInputChar(c),
            _ => Message::NoOp,
        },
        model::Popup::SessionNameInput => match key {
            KeyCode::Esc => Message::ClosePopup,
            KeyCode::Enter => {
                if let Some(ref name) = model.sessions.name_input {
                    if !name.trim().is_empty() {
                        return Message::SessionsSave(None);
                    }
                }
                Message::ClosePopup
            }
            KeyCode::Backspace => Message::SessionNameInputBackspace,
            KeyCode::Char(c) => Message::SessionNameInputChar(c),
            _ => Message::NoOp,
        },
        model::Popup::JobDetails(job_idx) => {
            match key {
                KeyCode::Esc | KeyCode::Enter => Message::ClosePopup,
                KeyCode::Char('r') => {
                    // Validate that the job can and should be retried
                    if let Some(job) = model.jobs.jobs.get(*job_idx) {
                        use crate::tui::model::jobs::JobStatus;

                        // Check basic retry eligibility
                        let can_retry = matches!(job.status, JobStatus::Failed | JobStatus::Completed)
                            && job.retry_context.is_some();

                        if !can_retry {
                            return Message::ShowError(
                                "Job cannot be retried (missing context)".to_string()
                            );
                        }

                        // Check if error type is retryable
                        if let Some(error) = &job.error {
                            if !error.is_retryable() {
                                return Message::ShowError(
                                    "Cannot retry: query syntax error - fix query first".to_string()
                                );
                            }
                        }

                        // Error is retryable - close popup and trigger retry
                        // Note: We can't return Vec<Message> from this function,
                        // so we'll just trigger retry and let the update handler close the popup
                        return Message::JobsRetry;
                    }
                    Message::NoOp
                }
                _ => Message::NoOp,
            }
        }
    }
}

/// Handle key events for the Settings tab
fn handle_settings_key(key: KeyCode) -> Message {
    match key {
        KeyCode::Up => Message::SettingsPrevious,
        KeyCode::Down => Message::SettingsNext,
        KeyCode::Enter | KeyCode::Char(' ') => Message::SettingsStartEdit,
        _ => Message::NoOp,
    }
}

/// Handle key events for the Workspaces tab
fn handle_workspaces_key(key: KeyCode) -> Message {
    match key {
        KeyCode::Up => Message::WorkspacesPrevious,
        KeyCode::Down => Message::WorkspacesNext,
        KeyCode::Char(' ') => Message::WorkspacesToggle,
        KeyCode::Char('a') => Message::WorkspacesSelectAll,
        KeyCode::Char('n') => Message::WorkspacesSelectNone,
        _ => Message::NoOp,
    }
}

/// Handle key events for the Query tab
fn handle_query_key(key: KeyCode, modifiers: KeyModifiers, model: &Model) -> Message {
    // If load panel is open, handle panel-specific keys
    if model.query.load_panel.is_some() {
        match key {
            KeyCode::Esc => return Message::QueryLoadPanelCancel,
            KeyCode::Enter => return Message::QueryLoadPanelConfirm,
            KeyCode::Up => return Message::QueryLoadPanelNavigate(-1),
            KeyCode::Down => return Message::QueryLoadPanelNavigate(1),
            KeyCode::Tab => return Message::QueryLoadPanelCycleSort,
            KeyCode::Char('i') => return Message::QueryLoadPanelInvertSort,
            _ => return Message::NoOp,
        }
    }

    match model.query.mode {
        EditorMode::Normal => {
            // Normal mode - vim-style navigation and commands
            match key {
                KeyCode::Char('i') => Message::QueryEnterInsertMode,
                KeyCode::Char('v') => Message::QueryEnterVisualMode, // Enter visual mode
                KeyCode::Char('a') => Message::QueryAppend,          // Insert after cursor
                KeyCode::Char('A') => Message::QueryAppendEnd,       // Insert at end of line
                KeyCode::Char('o') => Message::QueryOpenBelow,       // Open new line below
                KeyCode::Char('O') => Message::QueryOpenAbove,       // Open new line above
                KeyCode::Char('x') => Message::QueryDeleteChar, // Delete character under cursor
                KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                    Message::QueryDeleteLine
                } // Delete line
                KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                    Message::QueryUndo
                }
                KeyCode::Char('r') if modifiers.contains(KeyModifiers::CONTROL) => {
                    Message::QueryRedo
                }
                KeyCode::Char('c') => Message::QueryClear, // Clear all text
                KeyCode::Char('l') => Message::QueryOpenLoadPanel, // Load query from job
                // Navigation in normal mode
                KeyCode::Char('h') | KeyCode::Left => Message::QueryMoveCursor(KeyCode::Left),
                KeyCode::Char('j') | KeyCode::Down => Message::QueryMoveCursor(KeyCode::Down),
                KeyCode::Char('k') | KeyCode::Up => Message::QueryMoveCursor(KeyCode::Up),
                KeyCode::Right => Message::QueryMoveCursor(KeyCode::Right),
                KeyCode::Char('0') => Message::QueryMoveCursor(KeyCode::Home),
                KeyCode::Char('$') => Message::QueryMoveCursor(KeyCode::End),
                KeyCode::Char('g') => Message::QueryMoveTop,
                KeyCode::Char('G') => Message::QueryMoveBottom,
                _ => Message::NoOp,
            }
        }
        EditorMode::Insert => {
            // Insert mode - pass most keys to tui-textarea
            match key {
                KeyCode::Esc => Message::QueryExitInsertMode,
                _ => Message::QueryInput(ratatui::crossterm::event::KeyEvent::new(key, modifiers)),
            }
        }
        EditorMode::Visual => {
            // Visual mode - text selection
            match key {
                KeyCode::Esc => Message::QueryExitVisualMode,
                KeyCode::Char('y') => Message::QueryYank, // Copy selected text
                KeyCode::Char('d') | KeyCode::Char('x') => Message::QueryDeleteSelection, // Delete selection
                // Navigation extends selection
                KeyCode::Char('h') | KeyCode::Left => Message::QueryMoveCursor(KeyCode::Left),
                KeyCode::Char('j') | KeyCode::Down => Message::QueryMoveCursor(KeyCode::Down),
                KeyCode::Char('k') | KeyCode::Up => Message::QueryMoveCursor(KeyCode::Up),
                KeyCode::Char('l') | KeyCode::Right => Message::QueryMoveCursor(KeyCode::Right),
                KeyCode::Char('0') => Message::QueryMoveCursor(KeyCode::Home),
                KeyCode::Char('$') => Message::QueryMoveCursor(KeyCode::End),
                KeyCode::Char('g') => Message::QueryMoveTop,
                KeyCode::Char('G') => Message::QueryMoveBottom,
                _ => Message::NoOp,
            }
        }
    }
}

/// Handle key events for the Jobs tab
fn handle_jobs_key(key: KeyCode) -> Message {
    match key {
        KeyCode::Up => Message::JobsPrevious,
        KeyCode::Down => Message::JobsNext,
        KeyCode::Enter => Message::JobsViewDetails,
        KeyCode::Char('c') => Message::JobsClearCompleted,
        KeyCode::Char('r') => Message::JobsRetry,
        _ => Message::NoOp,
    }
}

/// Handle key events for the Sessions tab
fn handle_sessions_key(key: KeyCode, modifiers: KeyModifiers) -> Message {
    match key {
        KeyCode::Up => Message::SessionsPrevious,
        KeyCode::Down => Message::SessionsNext,
        KeyCode::Char('n') => Message::SessionsStartNew,
        KeyCode::Char('s') => {
            // 's' = save current session
            // 'S' (shift+s) = save as new name
            if modifiers.contains(KeyModifiers::SHIFT) {
                Message::SessionsStartNew
            } else {
                Message::SessionsSave(None)
            }
        }
        KeyCode::Char('l') => Message::SessionsLoad,
        KeyCode::Char('d') => Message::SessionsDelete,
        _ => Message::NoOp,
    }
}
