//! Execution logs modal state and logic

use kql_panopticon::tracing::{LogLevel, TuiEvent};

/// Maximum number of log events to retain (FIFO eviction)
pub const MAX_LOG_EVENTS: usize = 1000;

/// State for the logs modal
#[derive(Debug)]
pub struct LogsState {
    /// Log events (cloned from App for modal lifetime)
    logs: Vec<TuiEvent>,

    /// Current cursor position in filtered view
    cursor: usize,

    /// Scroll offset (first visible line)
    scroll_offset: usize,

    /// Viewport height (set during render)
    viewport_height: usize,

    /// Minimum log level filter (None = show all)
    min_level: Option<LogLevel>,

    /// Cached filtered indices for navigation
    filtered_indices: Vec<usize>,
}

impl LogsState {
    /// Create a new logs state, cursor at bottom (newest events)
    pub fn new(logs: Vec<TuiEvent>) -> Self {
        let mut state = Self {
            logs,
            cursor: 0,
            scroll_offset: 0,
            viewport_height: 20, // default, updated on render
            min_level: None,
            filtered_indices: Vec::new(),
        };
        state.rebuild_filtered_indices();
        // Start at bottom (newest)
        if !state.filtered_indices.is_empty() {
            state.cursor = state.filtered_indices.len() - 1;
        }
        state.ensure_cursor_visible();
        state
    }

    /// Rebuild the filtered indices cache based on current filter
    fn rebuild_filtered_indices(&mut self) {
        self.filtered_indices = self
            .logs
            .iter()
            .enumerate()
            .filter(|(_, event)| self.passes_filter(event))
            .map(|(idx, _)| idx)
            .collect();
    }

    /// Check if an event passes the current filter
    fn passes_filter(&self, event: &TuiEvent) -> bool {
        let Some(min_level) = self.min_level else {
            return true;
        };

        let event_level = match event {
            TuiEvent::Log { level, .. } => *level,
            TuiEvent::StepFailed { .. } => LogLevel::Error,
            TuiEvent::StepSkipped { .. } => LogLevel::Warn,
            // All lifecycle events are Info level
            _ => LogLevel::Info,
        };

        event_level >= min_level
    }

    /// Ensure cursor is visible within viewport
    fn ensure_cursor_visible(&mut self) {
        if self.viewport_height == 0 {
            return;
        }

        // Cursor above viewport - scroll up
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        }

        // Cursor below viewport - scroll down
        if self.cursor >= self.scroll_offset + self.viewport_height {
            self.scroll_offset = self.cursor.saturating_sub(self.viewport_height - 1);
        }
    }

    /// Move cursor up
    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.ensure_cursor_visible();
        }
    }

    /// Move cursor down
    pub fn move_down(&mut self) {
        if !self.filtered_indices.is_empty() && self.cursor < self.filtered_indices.len() - 1 {
            self.cursor += 1;
            self.ensure_cursor_visible();
        }
    }

    /// Page up (move by page_size)
    pub fn page_up(&mut self, page_size: usize) {
        self.cursor = self.cursor.saturating_sub(page_size);
        self.ensure_cursor_visible();
    }

    /// Page down (move by page_size)
    pub fn page_down(&mut self, page_size: usize) {
        if !self.filtered_indices.is_empty() {
            self.cursor = (self.cursor + page_size).min(self.filtered_indices.len() - 1);
            self.ensure_cursor_visible();
        }
    }

    /// Scroll to top (oldest events)
    pub fn scroll_to_top(&mut self) {
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    /// Scroll to bottom (newest events)
    pub fn scroll_to_bottom(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.cursor = self.filtered_indices.len() - 1;
            self.ensure_cursor_visible();
        }
    }

    /// Cycle through level filters: None → Error → Warn → Info → Debug → Trace → None
    pub fn cycle_level_filter(&mut self) {
        self.min_level = match self.min_level {
            None => Some(LogLevel::Error),
            Some(LogLevel::Error) => Some(LogLevel::Warn),
            Some(LogLevel::Warn) => Some(LogLevel::Info),
            Some(LogLevel::Info) => Some(LogLevel::Debug),
            Some(LogLevel::Debug) => Some(LogLevel::Trace),
            Some(LogLevel::Trace) => None,
        };
        self.rebuild_filtered_indices();
        // Adjust cursor if out of bounds
        if self.filtered_indices.is_empty() {
            self.cursor = 0;
            self.scroll_offset = 0;
        } else if self.cursor >= self.filtered_indices.len() {
            self.cursor = self.filtered_indices.len() - 1;
        }
        self.ensure_cursor_visible();
    }

    /// Clear all logs from the modal view (keeps filter settings)
    pub fn clear_logs(&mut self) {
        self.logs.clear();
        self.filtered_indices.clear();
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    /// Get the current minimum level filter
    pub fn min_level(&self) -> Option<LogLevel> {
        self.min_level
    }

    /// Check if logs are empty (after filtering)
    pub fn is_empty(&self) -> bool {
        self.filtered_indices.is_empty()
    }

    /// Get total event count (unfiltered)
    pub fn total_count(&self) -> usize {
        self.logs.len()
    }

    /// Get filtered event count
    pub fn filtered_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Count errors in the log
    pub fn error_count(&self) -> usize {
        self.logs.iter().filter(|e| e.is_error()).count()
    }

    /// Get the event at the current cursor position (for detail view)
    pub fn current_event(&self) -> Option<&TuiEvent> {
        self.filtered_indices
            .get(self.cursor)
            .map(|&idx| &self.logs[idx])
    }

    /// Get current cursor position
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Get current scroll offset
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Update viewport height (called during render)
    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height;
        self.ensure_cursor_visible();
    }

    /// Get iterator over visible events with their indices
    pub fn visible_events(&self) -> impl Iterator<Item = (usize, &TuiEvent)> {
        let start = self.scroll_offset;
        let end = (self.scroll_offset + self.viewport_height).min(self.filtered_indices.len());

        self.filtered_indices[start..end]
            .iter()
            .enumerate()
            .map(move |(display_offset, &log_idx)| (start + display_offset, &self.logs[log_idx]))
    }

    /// Get max scroll offset
    pub fn max_scroll(&self) -> usize {
        self.filtered_indices.len().saturating_sub(self.viewport_height)
    }
}

/// Format a TuiEvent for display
pub fn format_event(event: &TuiEvent) -> String {
    let timestamp = event.timestamp().format("%H:%M:%S");
    let (level, message) = match event {
        TuiEvent::JobStarted {
            pack_name,
            workspace_count,
            ..
        } => (
            "INFO ",
            format!("Job started: {} ({} workspaces)", pack_name, workspace_count),
        ),
        TuiEvent::JobCompleted {
            success, duration, ..
        } => {
            let status = if *success { "completed" } else { "failed" };
            (
                if *success { "INFO " } else { "ERROR" },
                format!("Job {}: {:?}", status, duration),
            )
        }
        TuiEvent::PhaseStarted {
            phase,
            workspace,
            step_count,
            ..
        } => (
            "INFO ",
            format!(
                "{} phase started on {} ({} steps)",
                phase, workspace, step_count
            ),
        ),
        TuiEvent::PhaseCompleted {
            phase,
            workspace,
            duration,
            ..
        } => (
            "INFO ",
            format!("{} phase completed on {}: {:?}", phase, workspace, duration),
        ),
        TuiEvent::StepStarted {
            step_name,
            workspace,
            step_type,
            ..
        } => (
            "INFO ",
            format!(
                "Step '{}' ({}) started on {}",
                step_name, step_type, workspace
            ),
        ),
        TuiEvent::StepCompleted {
            step_name,
            row_count,
            duration,
            ..
        } => {
            let rows = row_count
                .map(|r| format!("{} rows", r))
                .unwrap_or_default();
            (
                "INFO ",
                format!("Step '{}' completed: {} ({:?})", step_name, rows, duration),
            )
        }
        TuiEvent::StepFailed {
            step_name, error, ..
        } => ("ERROR", format!("Step '{}' failed: {}", step_name, error)),
        TuiEvent::StepSkipped {
            step_name, reason, ..
        } => ("WARN ", format!("Step '{}' skipped: {}", step_name, reason)),
        TuiEvent::ForeachProgress {
            step_name,
            current,
            total,
            ..
        } => (
            "DEBUG",
            format!("Step '{}': {}/{}", step_name, current, total),
        ),
        TuiEvent::Log {
            level, message, ..
        } => {
            let level_str = match level {
                LogLevel::Trace => "TRACE",
                LogLevel::Debug => "DEBUG",
                LogLevel::Info => "INFO ",
                LogLevel::Warn => "WARN ",
                LogLevel::Error => "ERROR",
            };
            (level_str, message.clone())
        }
    };

    format!("[{}] [{}] {}", timestamp, level, message)
}

/// Get the log level for an event (for coloring)
pub fn event_level(event: &TuiEvent) -> LogLevel {
    match event {
        TuiEvent::Log { level, .. } => *level,
        TuiEvent::StepFailed { .. } | TuiEvent::JobCompleted { success: false, .. } => {
            LogLevel::Error
        }
        TuiEvent::StepSkipped { .. } => LogLevel::Warn,
        TuiEvent::ForeachProgress { .. } => LogLevel::Debug,
        _ => LogLevel::Info,
    }
}

/// Wrap text to fit within a given width
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_len = word.chars().count();

        if current_width == 0 {
            // First word on line
            current_line = word.to_string();
            current_width = word_len;
        } else if current_width + 1 + word_len <= width {
            // Word fits on current line
            current_line.push(' ');
            current_line.push_str(word);
            current_width += 1 + word_len;
        } else {
            // Need new line
            lines.push(current_line);
            current_line = word.to_string();
            current_width = word_len;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}
