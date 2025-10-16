use ratatui::{style::Color, widgets::TableState};

/// Session state in the UI
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    /// Current session, saved (green)
    CurrentSaved,
    /// Current session, has unsaved changes (yellow)
    CurrentUnsaved,
    /// Current session, never been saved (red)
    CurrentNeverSaved,
    /// Loadable session (not current) - grey
    Loadable,
}

impl SessionState {
    /// Get the color for this session state
    pub fn color(&self, selected: bool) -> Color {
        match self {
            SessionState::CurrentSaved => Color::Green,
            SessionState::CurrentUnsaved => Color::Yellow,
            SessionState::CurrentNeverSaved => Color::Red,
            SessionState::Loadable => {
                if selected {
                    Color::DarkGray
                } else {
                    Color::Rgb(100, 100, 100) // Lighter grey for unselected
                }
            }
        }
    }

    /// Get a status indicator string
    pub fn indicator(&self) -> &'static str {
        match self {
            SessionState::CurrentSaved => "[CURRENT]",
            SessionState::CurrentUnsaved => "[CURRENT*]",
            SessionState::CurrentNeverSaved => "[CURRENT - UNSAVED]",
            SessionState::Loadable => "",
        }
    }
}

/// Session entry in the UI table
#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub name: String,
    pub state: SessionState,
    pub last_saved: Option<String>, // Timestamp or "Never" for unsaved
}

/// Sessions tab state
#[derive(Debug, Clone)]
pub struct SessionModel {
    /// List of session entries
    pub sessions: Vec<SessionEntry>,
    /// Table state for scrolling
    pub table_state: TableState,
    /// Name of the current session (if any)
    pub current_session_name: Option<String>,
    /// Whether the current session has unsaved changes
    pub has_unsaved_changes: bool,
    /// Input buffer for new session name
    pub name_input: Option<String>,
}

impl SessionModel {
    /// Create a new SessionModel
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            table_state: TableState::default(),
            current_session_name: None,
            has_unsaved_changes: false,
            name_input: None,
        }
    }

    /// Mark that changes have been made (sets unsaved flag)
    pub fn mark_dirty(&mut self) {
        self.has_unsaved_changes = true;
        self.refresh_session_states();
    }

    /// Mark that the current session has been saved
    pub fn mark_saved(&mut self) {
        self.has_unsaved_changes = false;
        self.refresh_session_states();
    }

    /// Set the current session name
    pub fn set_current_session(&mut self, name: Option<String>) {
        self.current_session_name = name;
        self.has_unsaved_changes = false;
        self.refresh_session_states();
    }

    /// Check if a session name is the current session
    fn is_current(&self, name: &str) -> bool {
        self.current_session_name.as_deref() == Some(name)
    }

    /// Determine the state for a session
    fn determine_state(&self, name: &str, exists_on_disk: bool) -> SessionState {
        if self.is_current(name) {
            if !exists_on_disk {
                SessionState::CurrentNeverSaved
            } else if self.has_unsaved_changes {
                SessionState::CurrentUnsaved
            } else {
                SessionState::CurrentSaved
            }
        } else {
            SessionState::Loadable
        }
    }

    /// Refresh the sessions list from disk
    pub fn refresh_from_disk(&mut self, available_sessions: Vec<String>) {
        // Keep track of current selection
        let selected_name = self.table_state.selected().and_then(|i| {
            self.sessions.get(i).map(|s| s.name.clone())
        });

        self.sessions.clear();

        // Add current session if it exists and isn't in the list
        if let Some(ref current_name) = self.current_session_name {
            if !available_sessions.contains(current_name) {
                self.sessions.push(SessionEntry {
                    name: current_name.clone(),
                    state: SessionState::CurrentNeverSaved,
                    last_saved: None,
                });
            }
        }

        // Add all sessions from disk
        for name in available_sessions {
            let exists_on_disk = true;
            let state = self.determine_state(&name, exists_on_disk);

            // Try to load the session to get last_saved timestamp
            let last_saved = crate::session::Session::load(&name)
                .ok()
                .map(|s| s.last_saved);

            self.sessions.push(SessionEntry {
                name,
                state,
                last_saved,
            });
        }

        // Sort: current session first, then alphabetically
        self.sessions.sort_by(|a, b| {
            match (&a.state, &b.state) {
                // Current sessions always come first
                (SessionState::CurrentSaved | SessionState::CurrentUnsaved | SessionState::CurrentNeverSaved, SessionState::Loadable) => std::cmp::Ordering::Less,
                (SessionState::Loadable, SessionState::CurrentSaved | SessionState::CurrentUnsaved | SessionState::CurrentNeverSaved) => std::cmp::Ordering::Greater,
                // Otherwise sort by name
                _ => a.name.cmp(&b.name),
            }
        });

        // Restore selection if possible
        if let Some(name) = selected_name {
            if let Some(idx) = self.sessions.iter().position(|s| s.name == name) {
                self.table_state.select(Some(idx));
            } else if !self.sessions.is_empty() {
                self.table_state.select(Some(0));
            }
        } else if !self.sessions.is_empty() {
            self.table_state.select(Some(0));
        }
    }

    /// Refresh session states (call after changing current session or dirty flag)
    fn refresh_session_states(&mut self) {
        // First pass: collect new states
        let new_states: Vec<(usize, SessionState)> = self.sessions
            .iter()
            .enumerate()
            .map(|(idx, session)| {
                let exists_on_disk = session.last_saved.is_some();
                let new_state = self.determine_state(&session.name, exists_on_disk);
                (idx, new_state)
            })
            .collect();

        // Second pass: update states
        for (idx, new_state) in new_states {
            if let Some(session) = self.sessions.get_mut(idx) {
                session.state = new_state;
            }
        }

        // Re-sort to ensure current session is at top
        self.sessions.sort_by(|a, b| {
            match (&a.state, &b.state) {
                (SessionState::CurrentSaved | SessionState::CurrentUnsaved | SessionState::CurrentNeverSaved, SessionState::Loadable) => std::cmp::Ordering::Less,
                (SessionState::Loadable, SessionState::CurrentSaved | SessionState::CurrentUnsaved | SessionState::CurrentNeverSaved) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
    }

    /// Get the currently selected session
    pub fn get_selected_session(&self) -> Option<&SessionEntry> {
        self.table_state.selected().and_then(|i| self.sessions.get(i))
    }

    /// Get the index of the current session
    #[allow(dead_code)]
    pub fn current_session_index(&self) -> Option<usize> {
        self.current_session_name.as_ref().and_then(|name| {
            self.sessions.iter().position(|s| s.name == *name)
        })
    }
}

impl Default for SessionModel {
    fn default() -> Self {
        Self::new()
    }
}
