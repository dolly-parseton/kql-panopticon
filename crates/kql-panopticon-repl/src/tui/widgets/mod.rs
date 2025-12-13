//! Interactive widget blocks for the TUI shell
//!
//! Widgets are interactive components that can be embedded in the output area.
//! Unlike static blocks (text, error, etc.), widgets capture keyboard input
//! and maintain mutable state.
//!
//! ## Widget Types
//!
//! - **Editor**: Multi-line text editor for KQL queries and YAML
//! - **InputForm**: Form for collecting input values before execution
//! - **Selector**: Interactive list picker for selection
//! - **Results**: Scrollable table view for query results
//!
//! ## Lifecycle
//!
//! 1. Widget created via command (e.g., `edit step`)
//! 2. Widget block added to output, focus set to Widget(block_id)
//! 3. User interacts with widget (typing, navigation)
//! 4. Widget completes (save/cancel) and signals result
//! 5. Focus returns to Input, widget may become static or update

pub mod editor;
pub mod input_def;
pub mod input_form;
pub mod results;
pub mod run_progress;
pub mod workspace_selector;

use std::collections::HashMap;

/// Result of a widget interaction
#[derive(Debug, Clone)]
pub enum WidgetResult {
    /// Widget is still active, no result yet
    Pending,
    /// User saved/submitted the widget
    Saved(WidgetOutput),
    /// User cancelled the widget
    Cancelled,
}

/// Output from a completed widget
#[derive(Debug, Clone)]
pub enum WidgetOutput {
    /// Editor content (name, content, is_new)
    EditorContent {
        name: String,
        content: String,
        /// True if creating new item, false if editing existing
        is_new: bool,
    },
    /// Form values for execution
    FormValues(HashMap<String, String>),
    /// Selected item (generic string selection)
    Selection(String),
    /// Input definition from InputDefWidget
    InputDef(crate::session::InputDef),
    /// Workspace selection (list of workspace IDs)
    WorkspaceSelection(Vec<String>),
}

/// Common trait for all widgets
pub trait Widget {
    /// Handle a key event, return true if handled
    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> bool;

    /// Check if the widget has completed
    fn result(&self) -> WidgetResult;

    /// Get the widget's desired height in lines
    fn height(&self) -> u16;
}
