//! Run progress widget for monitoring execution
//!
//! Displays a tree view of steps with status indicators, timing, and row counts.
//! Single workspace focus for now (multi-workspace tabs planned for later).

use super::{Widget, WidgetResult};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use std::time::Instant;

/// Status of a step in the execution
#[derive(Debug, Clone, PartialEq)]
pub enum StepStatus {
    /// Not yet started
    Pending,
    /// Currently running
    Running,
    /// Completed successfully
    Completed { rows: usize, duration_ms: u64 },
    /// Failed with error
    Failed { error: String },
    /// Skipped (condition not met)
    Skipped { reason: String },
}

/// A step in the progress tree
#[derive(Debug, Clone)]
pub struct ProgressStep {
    /// Step name
    pub name: String,
    /// Current status
    pub status: StepStatus,
    /// Dependencies (for tree visualization)
    pub depends_on: Vec<String>,
}

/// Overall execution status
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionState {
    /// Execution is in progress
    Running,
    /// Execution completed successfully
    Completed { duration_ms: u64 },
    /// Execution failed
    Failed { error: String },
}

/// Run progress widget
#[derive(Debug, Clone)]
pub struct RunProgressWidget {
    /// Pack/session name
    pub name: String,
    /// Workspace name
    pub workspace: String,
    /// Steps in execution order
    pub steps: Vec<ProgressStep>,
    /// Overall execution state
    pub state: ExecutionState,
    /// Start time for elapsed calculation
    start_time: Option<Instant>,
    /// Widget result
    pub result: WidgetResult,
    /// Scroll offset for long step lists
    scroll_offset: usize,
    /// Whether cancellation has been requested
    cancellation_requested: bool,
}

impl RunProgressWidget {
    /// Create a new progress widget
    pub fn new(name: impl Into<String>, workspace: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            workspace: workspace.into(),
            steps: Vec::new(),
            state: ExecutionState::Running,
            start_time: Some(Instant::now()),
            result: WidgetResult::Pending,
            scroll_offset: 0,
            cancellation_requested: false,
        }
    }

    /// Add a step to the progress tree
    pub fn with_step(mut self, name: impl Into<String>, depends_on: Vec<String>) -> Self {
        self.steps.push(ProgressStep {
            name: name.into(),
            status: StepStatus::Pending,
            depends_on,
        });
        self
    }

    /// Add multiple steps from names
    pub fn with_steps(mut self, step_names: &[String]) -> Self {
        for name in step_names {
            self.steps.push(ProgressStep {
                name: name.clone(),
                status: StepStatus::Pending,
                depends_on: Vec::new(),
            });
        }
        self
    }

    /// Mark a step as running (by name)
    pub fn step_started_by_name(&mut self, step_name: &str) {
        if let Some(step) = self.steps.iter_mut().find(|s| s.name == step_name) {
            step.status = StepStatus::Running;
        }
    }

    /// Mark a step as running (by index)
    pub fn step_started(&mut self, step_index: usize) {
        if let Some(step) = self.steps.get_mut(step_index) {
            step.status = StepStatus::Running;
        }
    }

    /// Mark a step as completed (by name)
    pub fn step_completed_by_name(&mut self, step_name: &str, rows: usize, duration_ms: u64) {
        if let Some(step) = self.steps.iter_mut().find(|s| s.name == step_name) {
            step.status = StepStatus::Completed { rows, duration_ms };
        }
    }

    /// Mark a step as completed (by index)
    pub fn step_completed(&mut self, step_index: usize, rows: usize, duration_ms: u64) {
        if let Some(step) = self.steps.get_mut(step_index) {
            step.status = StepStatus::Completed { rows, duration_ms };
        }
    }

    /// Mark a step as failed (by name)
    pub fn step_failed_by_name(&mut self, step_name: &str, error: String) {
        if let Some(step) = self.steps.iter_mut().find(|s| s.name == step_name) {
            step.status = StepStatus::Failed { error };
        }
    }

    /// Mark a step as failed (by index)
    pub fn step_failed(&mut self, step_index: usize, error: &str) {
        if let Some(step) = self.steps.get_mut(step_index) {
            step.status = StepStatus::Failed { error: error.to_string() };
        }
    }

    /// Mark a step as skipped (by name)
    pub fn step_skipped_by_name(&mut self, step_name: &str, reason: String) {
        if let Some(step) = self.steps.iter_mut().find(|s| s.name == step_name) {
            step.status = StepStatus::Skipped { reason };
        }
    }

    /// Mark a step as skipped (by index)
    pub fn step_skipped(&mut self, step_index: usize, reason: &str) {
        if let Some(step) = self.steps.get_mut(step_index) {
            step.status = StepStatus::Skipped { reason: reason.to_string() };
        }
    }

    /// Mark execution as completed
    pub fn execution_completed(&mut self, duration_ms: u64) {
        self.state = ExecutionState::Completed { duration_ms };
    }

    /// Mark execution as failed
    pub fn execution_failed(&mut self, error: String) {
        self.state = ExecutionState::Failed { error };
    }

    /// Mark execution as complete (convenience method)
    pub fn execution_complete(&mut self, success: bool) {
        if success {
            let duration_ms = self.elapsed_ms();
            self.state = ExecutionState::Completed { duration_ms };
        } else {
            // Check if any step failed to get the error
            let error = self.steps.iter()
                .find_map(|s| match &s.status {
                    StepStatus::Failed { error } => Some(error.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "Execution failed".to_string());
            self.state = ExecutionState::Failed { error };
        }
    }

    /// Check if execution is still running
    pub fn is_running(&self) -> bool {
        matches!(self.state, ExecutionState::Running)
    }

    /// Check if cancellation has been requested
    pub fn is_cancellation_requested(&self) -> bool {
        self.cancellation_requested
    }

    /// Request cancellation of this execution
    pub fn request_cancellation(&mut self) {
        self.cancellation_requested = true;
    }

    /// Mark the execution as cancelled
    pub fn mark_cancelled(&mut self) {
        self.cancellation_requested = true;
        self.state = ExecutionState::Failed {
            error: "Cancelled by user".to_string(),
        };
    }

    /// Get elapsed time
    fn elapsed_ms(&self) -> u64 {
        self.start_time
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    /// Get status icon for a step
    fn step_icon(status: &StepStatus) -> &'static str {
        match status {
            StepStatus::Pending => "○",
            StepStatus::Running => "◐",
            StepStatus::Completed { .. } => "✓",
            StepStatus::Failed { .. } => "✗",
            StepStatus::Skipped { .. } => "◌",
        }
    }

    /// Get status color for a step
    fn step_color(status: &StepStatus) -> Color {
        match status {
            StepStatus::Pending => Color::DarkGray,
            StepStatus::Running => Color::Yellow,
            StepStatus::Completed { .. } => Color::Green,
            StepStatus::Failed { .. } => Color::Red,
            StepStatus::Skipped { .. } => Color::DarkGray,
        }
    }

    /// Format step metadata (rows, timing)
    fn step_metadata(status: &StepStatus) -> String {
        match status {
            StepStatus::Pending => String::new(),
            StepStatus::Running => "running...".to_string(),
            StepStatus::Completed { rows, duration_ms } => {
                format!("{} rows, {}ms", rows, duration_ms)
            }
            StepStatus::Failed { error } => {
                // Truncate long errors
                if error.len() > 40 {
                    format!("{}...", &error[..37])
                } else {
                    error.clone()
                }
            }
            StepStatus::Skipped { reason } => {
                format!("skipped: {}", reason)
            }
        }
    }

    /// Render the widget
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();

        // Header with workspace and elapsed time
        let elapsed = self.elapsed_ms();
        let elapsed_str = format!("{:.1}s", elapsed as f64 / 1000.0);

        let state_indicator = match &self.state {
            ExecutionState::Running if self.cancellation_requested => ("Cancelling...", Color::Magenta),
            ExecutionState::Running => ("Running", Color::Yellow),
            ExecutionState::Completed { .. } => ("Complete", Color::Green),
            ExecutionState::Failed { .. } if self.cancellation_requested => ("Cancelled", Color::Magenta),
            ExecutionState::Failed { .. } => ("Failed", Color::Red),
        };

        lines.push(Line::from(vec![
            Span::styled("Workspace: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&self.workspace, Style::default().fg(Color::Cyan)),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(state_indicator.0, Style::default().fg(state_indicator.1)),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(elapsed_str, Style::default().fg(Color::Gray)),
        ]));

        // Separator
        lines.push(Line::from(vec![
            Span::styled("─".repeat(area.width.saturating_sub(4) as usize), Style::default().fg(Color::DarkGray)),
        ]));

        // Steps tree
        let visible_height = area.height.saturating_sub(6) as usize; // borders + header + separator + status
        let total_steps = self.steps.len();

        for (i, step) in self.steps.iter().enumerate().skip(self.scroll_offset).take(visible_height) {
            let icon = Self::step_icon(&step.status);
            let color = Self::step_color(&step.status);
            let metadata = Self::step_metadata(&step.status);

            let mut spans = vec![
                Span::styled(format!("  {} ", icon), Style::default().fg(color)),
                Span::styled(&step.name, Style::default().fg(Color::White)),
            ];

            if !metadata.is_empty() {
                spans.push(Span::styled(" ", Style::default()));
                spans.push(Span::styled(
                    metadata,
                    Style::default().fg(Color::DarkGray),
                ));
            }

            lines.push(Line::from(spans));
        }

        // Scroll indicator if needed
        if total_steps > visible_height {
            let shown = format!("[{}-{}/{}]",
                self.scroll_offset + 1,
                (self.scroll_offset + visible_height).min(total_steps),
                total_steps
            );
            lines.push(Line::from(vec![
                Span::styled(shown, Style::default().fg(Color::DarkGray)),
            ]));
        }

        // Status line with available actions
        let hints = if self.is_running() {
            if self.cancellation_requested {
                " [↑↓ Scroll] (cancelling...) "
            } else {
                " [↑↓ Scroll] [c Cancel] "
            }
        } else {
            " [↑↓ Scroll] [Esc Close] "
        };
        lines.push(Line::from(vec![
            Span::styled(hints, Style::default().fg(Color::DarkGray)),
        ]));

        let title = format!(" {} ", self.name);
        let block = Block::default()
            .title(title)
            .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}

impl Widget for RunProgressWidget {
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            // Close on Escape (only if execution is done)
            KeyCode::Esc => {
                if !self.is_running() {
                    self.result = WidgetResult::Cancelled;
                    true
                } else {
                    false // Can't close while running
                }
            }

            // Cancel execution
            KeyCode::Char('c') => {
                if self.is_running() && !self.cancellation_requested {
                    self.request_cancellation();
                    true
                } else {
                    false
                }
            }

            // Scroll
            KeyCode::Up | KeyCode::Char('k') => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.scroll_offset + 1 < self.steps.len() {
                    self.scroll_offset += 1;
                }
                true
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(5);
                true
            }
            KeyCode::PageDown => {
                self.scroll_offset = (self.scroll_offset + 5).min(self.steps.len().saturating_sub(1));
                true
            }

            _ => false,
        }
    }

    fn result(&self) -> WidgetResult {
        self.result.clone()
    }

    fn height(&self) -> u16 {
        // Header + separator + steps (min 3, max 10) + scroll indicator + status + borders
        let step_count = self.steps.len().min(10).max(3) as u16;
        step_count + 6
    }
}
