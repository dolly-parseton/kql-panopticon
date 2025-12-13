//! Widget events with before/after state for undo support
//!
//! These events are emitted when widgets are saved or cancelled,
//! capturing the state before and after for potential undo operations.

use std::collections::HashMap;
use crate::tui::widgets::editor::EditorWidget;
use crate::tui::widgets::input_form::InputFormWidget;
use crate::tui::widgets::input_def::InputDefWidget;
use crate::tui::widgets::results::ResultsWidget;
use crate::tui::widgets::run_progress::RunProgressWidget;
use crate::tui::widgets::workspace_selector::WorkspaceSelectorWidget;

/// State snapshot of a widget for undo/redo
#[derive(Debug, Clone)]
pub enum WidgetState {
    /// Editor content: (name, lines, cursor_position)
    Editor {
        name: String,
        content: String,
        cursor_line: usize,
        cursor_col: usize,
    },

    /// Input form values
    InputForm {
        name: String,
        values: HashMap<String, String>,
    },

    /// Input definition
    InputDef {
        name: String,
        input_type: String,
        default_value: Option<String>,
    },

    /// Results table (scroll position only - data is immutable)
    Results {
        name: String,
        scroll_offset: usize,
        selected_row: usize,
    },

    /// Run progress (state only - progress data is ephemeral)
    RunProgress {
        name: String,
        is_complete: bool,
    },

    /// Workspace selector (selected workspace IDs)
    WorkspaceSelector {
        name: String,
        selected_ids: Vec<String>,
    },
}

impl WidgetState {
    /// Get the widget name
    pub fn name(&self) -> &str {
        match self {
            Self::Editor { name, .. } => name,
            Self::InputForm { name, .. } => name,
            Self::InputDef { name, .. } => name,
            Self::Results { name, .. } => name,
            Self::RunProgress { name, .. } => name,
            Self::WorkspaceSelector { name, .. } => name,
        }
    }

    /// Get a short description of the state
    pub fn description(&self) -> String {
        match self {
            Self::Editor { name, content, .. } => {
                let lines = content.lines().count();
                format!("Editor '{}' ({} lines)", name, lines)
            }
            Self::InputForm { name, values, .. } => {
                format!("Form '{}' ({} values)", name, values.len())
            }
            Self::InputDef { name, input_type, .. } => {
                format!("Input '{}' ({})", name, input_type)
            }
            Self::Results { name, .. } => {
                format!("Results '{}'", name)
            }
            Self::RunProgress { name, is_complete } => {
                let status = if *is_complete { "complete" } else { "in progress" };
                format!("Run '{}' ({})", name, status)
            }
            Self::WorkspaceSelector { name, selected_ids } => {
                format!("Selector '{}' ({} selected)", name, selected_ids.len())
            }
        }
    }
}

/// Widget lifecycle events
///
/// Uses u64 for block_id to match the inner type of BlockId from app.rs
#[derive(Debug, Clone)]
pub enum WidgetEvent {
    /// Widget was focused
    Focused {
        block_id: u64,
        widget_name: String,
    },

    /// Widget was unfocused (blurred)
    Blurred {
        block_id: u64,
        widget_name: String,
    },

    /// Widget content was saved
    Saved {
        block_id: u64,
        /// State before the save (for undo)
        before: Option<WidgetState>,
        /// State after the save
        after: WidgetState,
    },

    /// Widget was cancelled (no changes persisted)
    Cancelled {
        block_id: u64,
        widget_name: String,
    },

    /// Widget was removed from output
    Removed {
        block_id: u64,
        /// State at time of removal (for undo)
        state: WidgetState,
    },

    /// Widget was minimized
    Minimized {
        block_id: u64,
        widget_name: String,
    },

    /// Widget was expanded
    Expanded {
        block_id: u64,
        widget_name: String,
    },
}

impl WidgetEvent {
    /// Get the block ID this event relates to
    pub fn block_id(&self) -> u64 {
        match self {
            Self::Focused { block_id, .. }
            | Self::Blurred { block_id, .. }
            | Self::Saved { block_id, .. }
            | Self::Cancelled { block_id, .. }
            | Self::Removed { block_id, .. }
            | Self::Minimized { block_id, .. }
            | Self::Expanded { block_id, .. } => *block_id,
        }
    }

    /// Get a short description of the event
    pub fn description(&self) -> String {
        match self {
            Self::Focused { widget_name, .. } => {
                format!("Focused: {}", widget_name)
            }
            Self::Blurred { widget_name, .. } => {
                format!("Blurred: {}", widget_name)
            }
            Self::Saved { after, .. } => {
                format!("Saved: {}", after.description())
            }
            Self::Cancelled { widget_name, .. } => {
                format!("Cancelled: {}", widget_name)
            }
            Self::Removed { state, .. } => {
                format!("Removed: {}", state.description())
            }
            Self::Minimized { widget_name, .. } => {
                format!("Minimized: {}", widget_name)
            }
            Self::Expanded { widget_name, .. } => {
                format!("Expanded: {}", widget_name)
            }
        }
    }

    /// Check if this event can be undone
    pub fn is_undoable(&self) -> bool {
        matches!(self, Self::Saved { .. } | Self::Removed { .. })
    }
}

// === From implementations for creating WidgetState from widgets ===

impl From<&EditorWidget> for WidgetState {
    fn from(editor: &EditorWidget) -> Self {
        WidgetState::Editor {
            name: editor.name.clone(),
            content: editor.content(),
            cursor_line: editor.cursor_row,
            cursor_col: editor.cursor_col,
        }
    }
}

impl From<&InputFormWidget> for WidgetState {
    fn from(form: &InputFormWidget) -> Self {
        WidgetState::InputForm {
            name: form.name.clone(),
            values: form.get_values(),
        }
    }
}

impl From<&InputDefWidget> for WidgetState {
    fn from(def: &InputDefWidget) -> Self {
        WidgetState::InputDef {
            name: def.name.clone(),
            input_type: def.input_type().to_string(),
            default_value: def.default_value(),
        }
    }
}

impl From<&ResultsWidget> for WidgetState {
    fn from(results: &ResultsWidget) -> Self {
        WidgetState::Results {
            name: results.name.clone(),
            scroll_offset: results.scroll_offset,
            selected_row: results.selected_row,
        }
    }
}

impl From<&RunProgressWidget> for WidgetState {
    fn from(progress: &RunProgressWidget) -> Self {
        WidgetState::RunProgress {
            name: progress.name.clone(),
            is_complete: !progress.is_running(),
        }
    }
}

impl From<&WorkspaceSelectorWidget> for WidgetState {
    fn from(selector: &WorkspaceSelectorWidget) -> Self {
        WidgetState::WorkspaceSelector {
            name: selector.name.clone(),
            selected_ids: selector.selected_ids(),
        }
    }
}
