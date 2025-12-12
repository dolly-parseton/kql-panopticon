//! Input field editing logic for the input form widget.
//!
//! Handles text editing, cursor movement, and validation state for individual input fields.

use super::validation::{validate_input_value, ValidationResult};
use crate::session::InputType;

/// An editable input field
#[derive(Debug, Clone)]
pub struct InputField {
    /// Field name (display label)
    pub name: String,
    /// Input type for validation
    pub input_type: InputType,
    /// Whether this field is required
    pub required: bool,
    /// Default value (shown as placeholder if not edited)
    pub default: Option<String>,
    /// Description/hint text
    pub description: Option<String>,
    /// Current text content
    content: String,
    /// Cursor position within content
    cursor: usize,
    /// Cached validation result
    validation: ValidationResult,
    /// Whether the field has been modified from default
    modified: bool,
}

impl InputField {
    /// Create a new input field
    pub fn new(
        name: String,
        input_type: InputType,
        required: bool,
        default: Option<String>,
        description: Option<String>,
    ) -> Self {
        // Pre-fill with default value if present
        let content = default.clone().unwrap_or_default();
        let validation = validate_input_value(&content, &input_type);
        let cursor = content.len();

        Self {
            name,
            input_type,
            required,
            default,
            description,
            content,
            cursor,
            validation,
            modified: false,
        }
    }

    /// Get the current content
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the cursor position
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Get the current validation result
    pub fn validation(&self) -> &ValidationResult {
        &self.validation
    }

    /// Check if the field has been modified from its default
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Check if the field is valid for submission
    ///
    /// A field is valid if:
    /// - It has a valid value, OR
    /// - It's empty but not required
    pub fn is_valid_for_submit(&self) -> bool {
        match &self.validation {
            ValidationResult::Valid => true,
            ValidationResult::Empty => !self.required,
            ValidationResult::Invalid(_) => false,
        }
    }

    /// Get the final value for submission
    ///
    /// Returns the content if modified, otherwise the default if present.
    /// Returns None if empty and not required.
    pub fn submit_value(&self) -> Option<String> {
        if self.content.is_empty() {
            if self.required {
                None
            } else {
                self.default.clone()
            }
        } else {
            Some(self.content.clone())
        }
    }

    /// Get error message if invalid
    pub fn error_message(&self) -> Option<&str> {
        if let ValidationResult::Invalid(msg) = &self.validation {
            Some(msg)
        } else if self.required && matches!(self.validation, ValidationResult::Empty) && self.modified
        {
            Some("Required field")
        } else {
            None
        }
    }

    // === Text Editing Operations ===

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.modified = true;
        self.revalidate();
    }

    /// Insert a string at the cursor position (for paste)
    pub fn insert_str(&mut self, s: &str) {
        self.content.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.modified = true;
        self.revalidate();
    }

    /// Delete the character before the cursor (backspace)
    pub fn delete_char_before(&mut self) {
        if self.cursor > 0 {
            // Find the previous char boundary
            let prev_boundary = self
                .content[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);

            self.content.remove(prev_boundary);
            self.cursor = prev_boundary;
            self.modified = true;
            self.revalidate();
        }
    }

    /// Delete the character at the cursor (delete key)
    pub fn delete_char_at(&mut self) {
        if self.cursor < self.content.len() {
            self.content.remove(self.cursor);
            self.modified = true;
            self.revalidate();
        }
    }

    /// Delete from cursor to end of line
    pub fn delete_to_end(&mut self) {
        if self.cursor < self.content.len() {
            self.content.truncate(self.cursor);
            self.modified = true;
            self.revalidate();
        }
    }

    /// Delete from cursor to start of line
    pub fn delete_to_start(&mut self) {
        if self.cursor > 0 {
            self.content = self.content[self.cursor..].to_string();
            self.cursor = 0;
            self.modified = true;
            self.revalidate();
        }
    }

    /// Clear all content
    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor = 0;
        self.modified = true;
        self.revalidate();
    }

    /// Set content (replaces everything)
    pub fn set_content(&mut self, content: &str) {
        self.content = content.to_string();
        self.cursor = self.content.len();
        self.modified = true;
        self.revalidate();
    }

    // === Cursor Movement ===

    /// Move cursor left
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            // Find previous char boundary
            self.cursor = self.content[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right
    pub fn move_right(&mut self) {
        if self.cursor < self.content.len() {
            // Find next char boundary
            self.cursor = self.content[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.content.len());
        }
    }

    /// Move cursor to start
    pub fn move_to_start(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end
    pub fn move_to_end(&mut self) {
        self.cursor = self.content.len();
    }

    /// Move cursor to previous word boundary
    pub fn move_word_left(&mut self) {
        if self.cursor == 0 {
            return;
        }

        // Skip whitespace
        let before_cursor = &self.content[..self.cursor];
        let trimmed_end = before_cursor.trim_end();
        if trimmed_end.is_empty() {
            self.cursor = 0;
            return;
        }

        // Find word boundary
        self.cursor = trimmed_end
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);
    }

    /// Move cursor to next word boundary
    pub fn move_word_right(&mut self) {
        if self.cursor >= self.content.len() {
            return;
        }

        // Skip current word
        let after_cursor = &self.content[self.cursor..];
        if let Some(space_pos) = after_cursor.find(char::is_whitespace) {
            // Skip whitespace
            let after_space = &after_cursor[space_pos..];
            let skip_ws = after_space
                .find(|c: char| !c.is_whitespace())
                .unwrap_or(after_space.len());
            self.cursor += space_pos + skip_ws;
        } else {
            self.cursor = self.content.len();
        }
    }

    // === Internal ===

    /// Revalidate the current content
    fn revalidate(&mut self) {
        self.validation = validate_input_value(&self.content, &self.input_type);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string_field() -> InputField {
        InputField::new(
            "test".to_string(),
            InputType::String,
            true,
            None,
            None,
        )
    }

    fn field_with_default() -> InputField {
        InputField::new(
            "test".to_string(),
            InputType::String,
            false,
            Some("default".to_string()),
            None,
        )
    }

    #[test]
    fn test_new_field() {
        let field = string_field();
        assert_eq!(field.content(), "");
        assert_eq!(field.cursor(), 0);
        assert!(!field.is_modified());
    }

    #[test]
    fn test_field_with_default() {
        let field = field_with_default();
        assert_eq!(field.content(), "default");
        assert_eq!(field.cursor(), 7); // At end
        assert!(!field.is_modified()); // Default doesn't count as modified
    }

    #[test]
    fn test_insert_char() {
        let mut field = string_field();
        field.insert_char('a');
        field.insert_char('b');
        field.insert_char('c');
        assert_eq!(field.content(), "abc");
        assert_eq!(field.cursor(), 3);
        assert!(field.is_modified());
    }

    #[test]
    fn test_insert_str() {
        let mut field = string_field();
        field.insert_str("hello");
        assert_eq!(field.content(), "hello");
        assert_eq!(field.cursor(), 5);
    }

    #[test]
    fn test_delete_char_before() {
        let mut field = string_field();
        field.insert_str("abc");
        field.delete_char_before();
        assert_eq!(field.content(), "ab");
        assert_eq!(field.cursor(), 2);
    }

    #[test]
    fn test_cursor_movement() {
        let mut field = string_field();
        field.insert_str("hello");
        assert_eq!(field.cursor(), 5);

        field.move_left();
        assert_eq!(field.cursor(), 4);

        field.move_to_start();
        assert_eq!(field.cursor(), 0);

        field.move_right();
        assert_eq!(field.cursor(), 1);

        field.move_to_end();
        assert_eq!(field.cursor(), 5);
    }

    #[test]
    fn test_validation() {
        let mut field = InputField::new(
            "count".to_string(),
            InputType::Int,
            true,
            None,
            None,
        );

        field.insert_str("42");
        assert!(field.is_valid_for_submit());

        field.clear();
        field.insert_str("not a number");
        assert!(!field.is_valid_for_submit());
        assert!(field.error_message().is_some());
    }

    #[test]
    fn test_submit_value() {
        let mut field = field_with_default();

        // Unmodified - returns default
        assert_eq!(field.submit_value(), Some("default".to_string()));

        // Modified - returns content
        field.clear();
        field.insert_str("custom");
        assert_eq!(field.submit_value(), Some("custom".to_string()));
    }

    #[test]
    fn test_required_empty() {
        let mut field = InputField::new(
            "required".to_string(),
            InputType::String,
            true,
            None,
            None,
        );

        // Initially not modified, so no error shown
        assert!(field.error_message().is_none());

        // After modification and clearing, show error
        field.insert_char('x');
        field.clear();
        assert!(field.error_message().is_some());
        assert!(!field.is_valid_for_submit());
    }
}
