//! Completion popup widget
//!
//! Renders a popup showing completion suggestions above the input line.

use crate::completion::Suggestion;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Maximum number of visible suggestions in the popup
const MAX_VISIBLE: usize = 8;

/// Completion popup state
#[derive(Debug, Clone)]
pub struct CompletionPopup {
    /// Available suggestions
    pub suggestions: Vec<Suggestion>,
    /// Currently selected index
    pub selected: usize,
    /// Scroll offset for long lists
    pub scroll_offset: usize,
}

impl CompletionPopup {
    /// Create a new completion popup with suggestions
    pub fn new(suggestions: Vec<Suggestion>) -> Self {
        Self {
            suggestions,
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Check if there are any suggestions
    pub fn is_empty(&self) -> bool {
        self.suggestions.is_empty()
    }

    /// Get the number of suggestions
    pub fn len(&self) -> usize {
        self.suggestions.len()
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        if self.suggestions.is_empty() {
            return;
        }
        if self.selected > 0 {
            self.selected -= 1;
        } else {
            self.selected = self.suggestions.len() - 1;
        }
        self.adjust_scroll();
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.suggestions.is_empty() {
            return;
        }
        if self.selected + 1 < self.suggestions.len() {
            self.selected += 1;
        } else {
            self.selected = 0;
        }
        self.adjust_scroll();
    }

    /// Adjust scroll offset to keep selected item visible
    fn adjust_scroll(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + MAX_VISIBLE {
            self.scroll_offset = self.selected - MAX_VISIBLE + 1;
        }
    }

    /// Get the currently selected suggestion
    pub fn selected_suggestion(&self) -> Option<&Suggestion> {
        self.suggestions.get(self.selected)
    }

    /// Calculate the popup area based on input line position
    pub fn calculate_area(&self, input_area: Rect, cursor_x: u16) -> Rect {
        let max_width = input_area.width.saturating_sub(2) as usize;
        let popup_width = self.calculate_width().min(max_width) as u16;
        let popup_height = (self.suggestions.len().min(MAX_VISIBLE) + 2) as u16; // +2 for borders

        // Position popup above the input line
        let x = cursor_x.min(input_area.x + input_area.width - popup_width);
        let y = input_area.y.saturating_sub(popup_height);

        Rect::new(x, y, popup_width, popup_height)
    }

    /// Calculate the width needed for the popup
    fn calculate_width(&self) -> usize {
        let max_value_width = self
            .suggestions
            .iter()
            .map(|s| s.value.len())
            .max()
            .unwrap_or(10);

        let max_desc_width = self
            .suggestions
            .iter()
            .filter_map(|s| s.description.as_ref().map(|d| d.len()))
            .max()
            .unwrap_or(0);

        // value + " - " + description + padding
        let content_width = if max_desc_width > 0 {
            max_value_width + 3 + max_desc_width.min(30)
        } else {
            max_value_width
        };

        content_width + 4 // borders + padding
    }

    /// Render the popup
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Clear the area first
        frame.render_widget(Clear, area);

        // Build lines for visible suggestions
        let visible_start = self.scroll_offset;
        let visible_end = (self.scroll_offset + MAX_VISIBLE).min(self.suggestions.len());

        let lines: Vec<Line> = self.suggestions[visible_start..visible_end]
            .iter()
            .enumerate()
            .map(|(i, suggestion)| {
                let actual_index = visible_start + i;
                let is_selected = actual_index == self.selected;

                let style = if is_selected {
                    Style::default()
                        .bg(Color::Yellow)
                        .fg(Color::Black)
                } else {
                    Style::default().fg(Color::White)
                };

                let desc_style = if is_selected {
                    Style::default()
                        .bg(Color::Yellow)
                        .fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let mut spans = vec![Span::styled(&suggestion.value, style)];

                if let Some(desc) = &suggestion.description {
                    // Truncate description if too long
                    let truncated = if desc.len() > 30 {
                        format!("{}...", &desc[..27])
                    } else {
                        desc.clone()
                    };
                    spans.push(Span::styled(" - ", desc_style));
                    spans.push(Span::styled(truncated, desc_style));
                }

                Line::from(spans)
            })
            .collect();

        // Build title with count if scrolling
        let title = if self.suggestions.len() > MAX_VISIBLE {
            format!(
                " Completions ({}/{}) ",
                self.selected + 1,
                self.suggestions.len()
            )
        } else {
            " Completions ".to_string()
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .style(Style::default().bg(Color::Black));

        let paragraph = Paragraph::new(lines).block(block);

        frame.render_widget(paragraph, area);
    }
}
