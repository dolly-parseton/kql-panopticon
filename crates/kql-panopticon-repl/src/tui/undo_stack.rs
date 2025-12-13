//! Undo/redo stack for widget state changes
//!
//! Provides undo/redo functionality for widget operations that modify state.
//! Works with WidgetEvent's before/after state to restore previous states.

use crate::tui::events::{WidgetEvent, WidgetState};

/// An undoable operation
#[derive(Debug, Clone)]
pub struct UndoableOperation {
    /// Description of what was done
    pub description: String,
    /// Block ID this operation affects
    pub block_id: u64,
    /// State before the operation (for undo)
    pub before: Option<WidgetState>,
    /// State after the operation (for redo)
    pub after: WidgetState,
}

impl UndoableOperation {
    /// Create from a Saved widget event
    pub fn from_saved_event(event: &WidgetEvent) -> Option<Self> {
        match event {
            WidgetEvent::Saved { block_id, before, after } => Some(Self {
                description: format!("Saved: {}", after.description()),
                block_id: *block_id,
                before: before.clone(),
                after: after.clone(),
            }),
            _ => None,
        }
    }
}

/// Undo/redo stack for tracking widget state changes
#[derive(Debug, Default)]
pub struct UndoStack {
    /// Stack of undoable operations
    undo_stack: Vec<UndoableOperation>,
    /// Stack of redoable operations
    redo_stack: Vec<UndoableOperation>,
    /// Maximum number of operations to keep
    max_size: usize,
}

impl UndoStack {
    /// Create a new undo stack with default max size (50)
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size: 50,
        }
    }

    /// Create with a specific max size
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size,
        }
    }

    /// Push a new operation onto the undo stack
    ///
    /// Clears the redo stack since we're on a new branch.
    pub fn push(&mut self, operation: UndoableOperation) {
        self.undo_stack.push(operation);
        self.redo_stack.clear();

        // Trim if over max size
        if self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }
    }

    /// Push from a widget event if it's undoable
    pub fn push_event(&mut self, event: &WidgetEvent) {
        if let Some(op) = UndoableOperation::from_saved_event(event) {
            self.push(op);
        }
    }

    /// Pop the last operation for undo
    ///
    /// Returns the operation that should be undone (restore `before` state).
    /// The operation is moved to the redo stack.
    pub fn pop_undo(&mut self) -> Option<UndoableOperation> {
        if let Some(op) = self.undo_stack.pop() {
            self.redo_stack.push(op.clone());
            Some(op)
        } else {
            None
        }
    }

    /// Pop from redo stack
    ///
    /// Returns the operation that should be redone (restore `after` state).
    /// The operation is moved back to the undo stack.
    pub fn pop_redo(&mut self) -> Option<UndoableOperation> {
        if let Some(op) = self.redo_stack.pop() {
            self.undo_stack.push(op.clone());
            Some(op)
        } else {
            None
        }
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Get the description of the next undo operation
    pub fn next_undo_description(&self) -> Option<&str> {
        self.undo_stack.last().map(|op| op.description.as_str())
    }

    /// Get the description of the next redo operation
    pub fn next_redo_description(&self) -> Option<&str> {
        self.redo_stack.last().map(|op| op.description.as_str())
    }

    /// Get the number of undo operations available
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get the number of redo operations available
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clear all undo/redo history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_editor_state(name: &str, content: &str) -> WidgetState {
        WidgetState::Editor {
            name: name.to_string(),
            content: content.to_string(),
            cursor_line: 0,
            cursor_col: 0,
        }
    }

    #[test]
    fn test_push_and_undo() {
        let mut stack = UndoStack::new();

        let op = UndoableOperation {
            description: "Edit step1".to_string(),
            block_id: 1,
            before: Some(make_editor_state("step1", "old content")),
            after: make_editor_state("step1", "new content"),
        };

        stack.push(op);

        assert!(stack.can_undo());
        assert!(!stack.can_redo());

        let undone = stack.pop_undo().unwrap();
        assert_eq!(undone.block_id, 1);

        assert!(!stack.can_undo());
        assert!(stack.can_redo());
    }

    #[test]
    fn test_undo_then_redo() {
        let mut stack = UndoStack::new();

        let op = UndoableOperation {
            description: "Edit".to_string(),
            block_id: 1,
            before: Some(make_editor_state("s", "a")),
            after: make_editor_state("s", "b"),
        };

        stack.push(op);

        // Undo
        let _ = stack.pop_undo();
        assert!(stack.can_redo());

        // Redo
        let redone = stack.pop_redo().unwrap();
        assert_eq!(redone.description, "Edit");

        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_new_push_clears_redo() {
        let mut stack = UndoStack::new();

        stack.push(UndoableOperation {
            description: "A".to_string(),
            block_id: 1,
            before: None,
            after: make_editor_state("s", "a"),
        });

        stack.pop_undo();
        assert!(stack.can_redo());

        // Push new operation
        stack.push(UndoableOperation {
            description: "B".to_string(),
            block_id: 2,
            before: None,
            after: make_editor_state("s", "b"),
        });

        // Redo should be cleared
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_max_size() {
        let mut stack = UndoStack::with_max_size(3);

        for i in 0..5 {
            stack.push(UndoableOperation {
                description: format!("Op {}", i),
                block_id: i,
                before: None,
                after: make_editor_state("s", &format!("{}", i)),
            });
        }

        assert_eq!(stack.undo_count(), 3);
        assert_eq!(stack.next_undo_description(), Some("Op 4"));
    }
}
