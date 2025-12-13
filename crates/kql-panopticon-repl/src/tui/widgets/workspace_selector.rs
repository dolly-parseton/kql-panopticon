//! Workspace selector widget for multi-select workspace selection
//!
//! Provides an embedded list for selecting one or more workspaces.
//! Groups workspaces by subscription and supports select/deselect all.

use super::{Widget, WidgetOutput, WidgetResult};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kql_panopticon_core::Workspace;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use std::collections::HashSet;

/// A workspace entry in the selector
#[derive(Debug, Clone)]
struct WorkspaceEntry {
    /// Workspace ID (unique identifier)
    workspace_id: String,
    /// Workspace name (display)
    name: String,
    /// Subscription name (for grouping)
    subscription_name: String,
    /// Location (for display)
    location: String,
    /// Whether this workspace is selected
    selected: bool,
}

impl From<&Workspace> for WorkspaceEntry {
    fn from(ws: &Workspace) -> Self {
        Self {
            workspace_id: ws.workspace_id.clone(),
            name: ws.name.clone(),
            subscription_name: ws.subscription_name.clone(),
            location: ws.location.clone(),
            selected: false,
        }
    }
}

/// Workspace selector widget for multi-select workspace selection
#[derive(Debug, Clone)]
pub struct WorkspaceSelectorWidget {
    /// Widget name/title
    pub name: String,
    /// All workspace entries (flat list, but will render grouped)
    entries: Vec<WorkspaceEntry>,
    /// Currently focused entry index
    focused: usize,
    /// Scroll offset for long lists
    scroll_offset: usize,
    /// Widget result (pending until submitted/cancelled)
    pub result: WidgetResult,
    /// Whether the submit button is focused
    button_focused: bool,
}

impl WorkspaceSelectorWidget {
    /// Create a new workspace selector widget
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entries: Vec::new(),
            focused: 0,
            scroll_offset: 0,
            result: WidgetResult::Pending,
            button_focused: false,
        }
    }

    /// Populate with workspaces, optionally pre-selecting some
    pub fn with_workspaces(mut self, workspaces: &[Workspace], selected_ids: &HashSet<String>) -> Self {
        self.entries = workspaces
            .iter()
            .map(|ws| {
                let mut entry = WorkspaceEntry::from(ws);
                entry.selected = selected_ids.contains(&ws.workspace_id);
                entry
            })
            .collect();

        // Sort by subscription name, then by workspace name
        self.entries.sort_by(|a, b| {
            match a.subscription_name.cmp(&b.subscription_name) {
                std::cmp::Ordering::Equal => a.name.cmp(&b.name),
                other => other,
            }
        });

        self
    }

    /// Get the selected workspace IDs
    pub fn selected_ids(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.selected)
            .map(|e| e.workspace_id.clone())
            .collect()
    }

    /// Get the selected workspace names
    pub fn selected_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.selected)
            .map(|e| e.name.clone())
            .collect()
    }

    /// Count selected workspaces
    pub fn selected_count(&self) -> usize {
        self.entries.iter().filter(|e| e.selected).count()
    }

    /// Check if any workspace is selected
    pub fn has_selection(&self) -> bool {
        self.entries.iter().any(|e| e.selected)
    }

    /// Toggle selection of the focused entry
    fn toggle_focused(&mut self) {
        if !self.button_focused {
            if let Some(entry) = self.entries.get_mut(self.focused) {
                entry.selected = !entry.selected;
            }
        }
    }

    /// Select all workspaces
    fn select_all(&mut self) {
        for entry in &mut self.entries {
            entry.selected = true;
        }
    }

    /// Deselect all workspaces
    fn deselect_all(&mut self) {
        for entry in &mut self.entries {
            entry.selected = false;
        }
    }

    /// Move focus to the next entry
    fn focus_next(&mut self) {
        if self.button_focused {
            // Wrap to first entry
            self.button_focused = false;
            self.focused = 0;
        } else if self.focused + 1 < self.entries.len() {
            self.focused += 1;
        } else {
            // Move to button
            self.button_focused = true;
        }
        self.adjust_scroll();
    }

    /// Move focus to the previous entry
    fn focus_previous(&mut self) {
        if self.button_focused {
            self.button_focused = false;
            // Stay on last entry
        } else if self.focused > 0 {
            self.focused -= 1;
        } else {
            // Wrap to button
            self.button_focused = true;
        }
        self.adjust_scroll();
    }

    /// Maximum visible entries (excluding header, button, hints)
    const MAX_VISIBLE_ENTRIES: usize = 12;

    /// Adjust scroll offset to keep focused item visible
    fn adjust_scroll(&mut self) {
        if self.button_focused {
            // Make sure the button is visible
            let entry_count = self.entries.len();
            if entry_count > Self::MAX_VISIBLE_ENTRIES {
                self.scroll_offset = entry_count - Self::MAX_VISIBLE_ENTRIES;
            }
        } else if self.focused < self.scroll_offset {
            self.scroll_offset = self.focused;
        } else if self.focused >= self.scroll_offset + Self::MAX_VISIBLE_ENTRIES {
            self.scroll_offset = self.focused - Self::MAX_VISIBLE_ENTRIES + 1;
        }
    }

    /// Render the widget
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        // Clear the area first to prevent rendering artifacts
        frame.render_widget(Clear, area);

        let title = format!(" {} ", self.name);

        let block = Block::default()
            .title(title)
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines: Vec<Line> = Vec::new();

        // Selection summary header
        let selected_count = self.selected_count();
        let total_count = self.entries.len();
        let summary = format!("{}/{} selected", selected_count, total_count);
        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(summary, Style::default().fg(Color::DarkGray)),
            Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
            Span::styled("a", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" all  ", Style::default().fg(Color::DarkGray)),
            Span::styled("n", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" none", Style::default().fg(Color::DarkGray)),
        ]));
        lines.push(Line::default());

        // Group entries by subscription for display
        let mut current_subscription: Option<&str> = None;

        // Calculate visible range
        let visible_start = self.scroll_offset;
        let visible_end = (self.scroll_offset + Self::MAX_VISIBLE_ENTRIES).min(self.entries.len());

        for (idx, entry) in self.entries.iter().enumerate() {
            // Skip entries outside visible range
            if idx < visible_start || idx >= visible_end {
                continue;
            }

            // Subscription header (only if changed)
            if current_subscription != Some(&entry.subscription_name) {
                current_subscription = Some(&entry.subscription_name);
                lines.push(Line::from(vec![
                    Span::styled(" ", Style::default()),
                    Span::styled(
                        &entry.subscription_name,
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ),
                ]));
            }

            let is_focused = idx == self.focused && !self.button_focused;

            // Checkbox
            let checkbox = if entry.selected { "[✓]" } else { "[ ]" };
            let checkbox_style = if entry.selected {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            // Entry line
            let focus_indicator = if is_focused { "▸" } else { " " };
            let focus_style = if is_focused {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            let name_style = if is_focused {
                Style::default().fg(Color::White)
            } else if entry.selected {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Gray)
            };

            let location_style = Style::default().fg(Color::DarkGray);

            lines.push(Line::from(vec![
                Span::styled(format!("  {}", focus_indicator), focus_style),
                Span::styled(format!(" {} ", checkbox), checkbox_style),
                Span::styled(&entry.name, name_style),
                Span::styled(format!(" ({})", entry.location), location_style),
            ]));
        }

        // Scroll indicator if needed
        if self.entries.len() > Self::MAX_VISIBLE_ENTRIES {
            let indicator = format!(
                " ↕ {}-{} of {}",
                visible_start + 1,
                visible_end,
                self.entries.len()
            );
            lines.push(Line::from(vec![
                Span::styled(indicator, Style::default().fg(Color::DarkGray)),
            ]));
        }

        // Spacing before button
        lines.push(Line::default());

        // Submit button
        let button_style = if self.button_focused {
            if self.has_selection() {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Black).bg(Color::DarkGray)
            }
        } else if self.has_selection() {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let button_text = if self.has_selection() {
            format!("[ Confirm ({}) ]", self.selected_count())
        } else {
            "[ Confirm ] (select at least one)".to_string()
        };

        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(button_text, button_style),
        ]));

        // Hints
        lines.push(Line::from(vec![
            Span::styled(
                " Space",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" toggle  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "↑↓",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" nav  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Enter",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" confirm  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
        ]));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

impl Widget for WorkspaceSelectorWidget {
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            // Cancel
            KeyCode::Esc => {
                self.result = WidgetResult::Cancelled;
                true
            }

            // Submit (Ctrl+Enter or Enter on button)
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.has_selection() {
                    let selected_ids = self.selected_ids();
                    self.result = WidgetResult::Saved(WidgetOutput::WorkspaceSelection(selected_ids));
                }
                true
            }
            KeyCode::Enter if self.button_focused => {
                if self.has_selection() {
                    let selected_ids = self.selected_ids();
                    self.result = WidgetResult::Saved(WidgetOutput::WorkspaceSelection(selected_ids));
                }
                true
            }

            // Toggle selection with Space
            KeyCode::Char(' ') if !self.button_focused => {
                self.toggle_focused();
                true
            }

            // Select all with 'a'
            KeyCode::Char('a') if !self.button_focused => {
                self.select_all();
                true
            }

            // Deselect all with 'n'
            KeyCode::Char('n') if !self.button_focused => {
                self.deselect_all();
                true
            }

            // Navigation
            KeyCode::Up | KeyCode::Char('k') => {
                self.focus_previous();
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.focus_next();
                true
            }
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.focus_previous();
                true
            }
            KeyCode::Tab => {
                self.focus_next();
                true
            }

            // Page navigation
            KeyCode::PageUp => {
                for _ in 0..Self::MAX_VISIBLE_ENTRIES {
                    if self.focused > 0 {
                        self.focused -= 1;
                    }
                }
                self.adjust_scroll();
                true
            }
            KeyCode::PageDown => {
                for _ in 0..Self::MAX_VISIBLE_ENTRIES {
                    if self.focused + 1 < self.entries.len() {
                        self.focused += 1;
                    }
                }
                self.adjust_scroll();
                true
            }

            // Home/End
            KeyCode::Home => {
                self.focused = 0;
                self.button_focused = false;
                self.adjust_scroll();
                true
            }
            KeyCode::End => {
                if !self.entries.is_empty() {
                    self.focused = self.entries.len() - 1;
                }
                self.button_focused = false;
                self.adjust_scroll();
                true
            }

            _ => false,
        }
    }

    fn result(&self) -> WidgetResult {
        self.result.clone()
    }

    fn height(&self) -> u16 {
        // Header (2) + visible entries + scroll indicator + spacing + button + hints + borders
        let entry_lines = self.entries.len().min(Self::MAX_VISIBLE_ENTRIES);
        // Account for subscription headers (roughly estimate)
        let subscription_count = {
            let mut subs = HashSet::new();
            for entry in &self.entries {
                subs.insert(&entry.subscription_name);
            }
            subs.len().min(entry_lines)
        };

        let content_lines = 2 // header + blank
            + entry_lines
            + subscription_count // subscription headers
            + 1 // scroll indicator (if any)
            + 1 // spacing
            + 1 // button
            + 1; // hints

        (content_lines + 2) as u16 // +2 for borders
    }
}
