/// The input prompt state
#[derive(Debug, Clone, Default)]
pub struct PromptState {
    /// Current input buffer
    pub buffer: String,
    /// Cursor position (character index)
    pub cursor: usize,
    /// Command history
    pub history: Vec<String>,
    /// Current history navigation index (None = current input)
    pub history_index: Option<usize>,
    /// Saved current input when navigating history
    pub saved_input: String,
    /// Accumulated lines during line continuation
    pub continuation_lines: Vec<String>,
}

impl PromptState {
    /// Check if we're in line continuation mode
    pub fn is_continuation(&self) -> bool {
        !self.continuation_lines.is_empty()
    }

    /// Get the full input including continuation lines
    pub fn full_input(&self) -> String {
        if self.continuation_lines.is_empty() {
            self.buffer.clone()
        } else {
            let mut result = self.continuation_lines.join("\n");
            result.push('\n');
            result.push_str(&self.buffer);
            result
        }
    }

    /// Clear continuation state
    pub fn clear_continuation(&mut self) {
        self.continuation_lines.clear();
    }
}
