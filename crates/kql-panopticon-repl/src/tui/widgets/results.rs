//! Results widget for displaying query results
//!
//! Provides a scrollable table view for query results embedded as an OutputBlock.

use super::{Widget, WidgetOutput, WidgetResult};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

/// Results widget for displaying query output
#[derive(Debug, Clone)]
pub struct ResultsWidget {
    /// Title/name of the results (e.g., step name)
    pub name: String,
    /// Column headers
    pub columns: Vec<String>,
    /// Rows of data (each row is a vec of cell values)
    pub rows: Vec<Vec<String>>,
    /// Currently selected row (0-indexed)
    pub selected_row: usize,
    /// Scroll offset (first visible row)
    pub scroll_offset: usize,
    /// Horizontal scroll offset
    pub horizontal_offset: usize,
    /// Visible height (set during render)
    pub visible_height: u16,
    /// Visible width (set during render)
    visible_width: usize,
    /// Widget result (pending until closed)
    pub result: WidgetResult,
    /// Max column widths for formatting
    column_widths: Vec<usize>,
}

impl ResultsWidget {
    /// Create a new results widget
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            columns: Vec::new(),
            rows: Vec::new(),
            selected_row: 0,
            scroll_offset: 0,
            horizontal_offset: 0,
            visible_height: 10,
            visible_width: 80,
            result: WidgetResult::Pending,
            column_widths: Vec::new(),
        }
    }

    /// Set columns and rows from data
    pub fn with_data(mut self, columns: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        // Calculate column widths (using char count, not byte length)
        let mut widths: Vec<usize> = columns.iter().map(|c| c.chars().count()).collect();

        for row in &rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(cell.chars().count()).min(40); // Cap at 40 chars
                }
            }
        }

        self.columns = columns;
        self.rows = rows;
        self.column_widths = widths;
        self
    }

    /// Ensure selected row is visible
    fn ensure_visible(&mut self) {
        let visible = self.visible_height.saturating_sub(4) as usize; // Account for borders + header
        if self.selected_row < self.scroll_offset {
            self.scroll_offset = self.selected_row;
        } else if self.selected_row >= self.scroll_offset + visible {
            self.scroll_offset = self.selected_row - visible + 1;
        }
    }

    /// Move selection up
    fn move_up(&mut self) {
        if self.selected_row > 0 {
            self.selected_row -= 1;
            self.ensure_visible();
        }
    }

    /// Move selection down
    fn move_down(&mut self) {
        if self.selected_row + 1 < self.rows.len() {
            self.selected_row += 1;
            self.ensure_visible();
        }
    }

    /// Page up
    fn page_up(&mut self) {
        let page_size = self.visible_height.saturating_sub(4) as usize;
        self.selected_row = self.selected_row.saturating_sub(page_size);
        self.ensure_visible();
    }

    /// Page down
    fn page_down(&mut self) {
        let page_size = self.visible_height.saturating_sub(4) as usize;
        self.selected_row = (self.selected_row + page_size).min(self.rows.len().saturating_sub(1));
        self.ensure_visible();
    }

    /// Scroll left
    fn scroll_left(&mut self) {
        self.horizontal_offset = self.horizontal_offset.saturating_sub(5);
    }

    /// Scroll right (bounded by content width)
    fn scroll_right(&mut self) {
        let max_offset = self.max_horizontal_offset();
        if self.horizontal_offset < max_offset {
            self.horizontal_offset = (self.horizontal_offset + 5).min(max_offset);
        }
    }

    /// Calculate the total content width (for scroll bounds)
    fn content_width(&self) -> usize {
        // Sum of column widths + separators (" │ " = 3 chars each)
        let col_width: usize = self.column_widths.iter().sum();
        let separator_width = if self.column_widths.len() > 1 {
            (self.column_widths.len() - 1) * 3
        } else {
            0
        };
        col_width + separator_width
    }

    /// Calculate maximum horizontal offset
    fn max_horizontal_offset(&self) -> usize {
        // Stop scrolling when the end of content reaches the right edge
        // i.e., max_offset = content_width - visible_width (or 0 if content fits)
        let content = self.content_width();
        if content <= self.visible_width {
            0 // Content fits entirely, no scrolling needed
        } else {
            content - self.visible_width
        }
    }

    /// Format a row as a string
    fn format_row(&self, row: &[String]) -> String {
        row.iter()
            .zip(&self.column_widths)
            .map(|(cell, &width)| {
                let char_count = cell.chars().count();
                let truncated = if char_count > width {
                    // Truncate by characters, not bytes
                    let truncated_chars: String = cell.chars().take(width.saturating_sub(1)).collect();
                    format!("{}…", truncated_chars)
                } else {
                    cell.clone()
                };
                // Pad to width (by characters)
                let padding = width.saturating_sub(truncated.chars().count());
                format!("{}{}", truncated, " ".repeat(padding))
            })
            .collect::<Vec<_>>()
            .join(" │ ")
    }

    /// Render the results widget
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.visible_height = area.height;

        let inner_height = area.height.saturating_sub(3) as usize; // borders + status line
        let available_width = area.width.saturating_sub(4) as usize; // borders + padding
        self.visible_width = available_width;

        // Clamp horizontal offset to valid range (in case window resized or first render)
        let max_offset = self.max_horizontal_offset();
        if self.horizontal_offset > max_offset {
            self.horizontal_offset = max_offset;
        }

        let title = format!(" {} ({} rows) ", self.name, self.rows.len());

        let mut display_lines: Vec<Line> = Vec::new();

        // Header line
        if !self.columns.is_empty() {
            let header = self.format_row(&self.columns);
            let header_visible = skip_chars(&header, self.horizontal_offset);
            display_lines.push(Line::from(vec![
                Span::styled(
                    truncate_to_width(header_visible, available_width),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
            ]));

            // Separator
            display_lines.push(Line::from(vec![
                Span::styled("─".repeat(available_width), Style::default().fg(Color::DarkGray)),
            ]));
        }

        // Data rows
        let data_height = inner_height.saturating_sub(2); // Minus header and separator
        for (i, row) in self.rows.iter().enumerate().skip(self.scroll_offset).take(data_height) {
            let is_selected = i == self.selected_row;
            let formatted = self.format_row(row);
            let visible = skip_chars(&formatted, self.horizontal_offset);

            let style = if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };

            display_lines.push(Line::from(vec![
                Span::styled(truncate_to_width(visible, available_width), style),
            ]));
        }

        // Status line
        display_lines.push(Line::from(vec![
            Span::styled(
                format!(
                    " [↑↓ Navigate] [←→ Scroll] [PgUp/PgDn] [Esc Close] │ Row {}/{} ",
                    self.selected_row + 1,
                    self.rows.len()
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        let block = Block::default()
            .title(title)
            .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let paragraph = Paragraph::new(display_lines).block(block);
        frame.render_widget(paragraph, area);

        // Render scrollbar if needed
        if self.rows.len() > data_height {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let mut scrollbar_state = ScrollbarState::new(self.rows.len())
                .position(self.scroll_offset);

            frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }
}

impl Widget for ResultsWidget {
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match (key.modifiers, key.code) {
            // Close: Escape or Q
            (_, KeyCode::Esc) | (_, KeyCode::Char('q')) => {
                self.result = WidgetResult::Cancelled;
                true
            }

            // Navigation
            (_, KeyCode::Up) | (_, KeyCode::Char('k')) => {
                self.move_up();
                true
            }
            (_, KeyCode::Down) | (_, KeyCode::Char('j')) => {
                self.move_down();
                true
            }
            (_, KeyCode::Left) | (_, KeyCode::Char('h')) => {
                self.scroll_left();
                true
            }
            (_, KeyCode::Right) | (_, KeyCode::Char('l')) => {
                self.scroll_right();
                true
            }
            (_, KeyCode::PageUp) => {
                self.page_up();
                true
            }
            (_, KeyCode::PageDown) => {
                self.page_down();
                true
            }
            (_, KeyCode::Home) => {
                self.selected_row = 0;
                self.ensure_visible();
                true
            }
            (_, KeyCode::End) => {
                self.selected_row = self.rows.len().saturating_sub(1);
                self.ensure_visible();
                true
            }

            // Select/copy current row (future feature)
            (_, KeyCode::Enter) => {
                // Could emit a selection event
                true
            }

            _ => false,
        }
    }

    fn result(&self) -> WidgetResult {
        self.result.clone()
    }

    fn height(&self) -> u16 {
        // Header + separator + at least 5 data rows + status + borders
        let content_rows = self.rows.len().min(10).max(5) as u16;
        content_rows + 5
    }
}

/// Skip N characters from a string, returning the remainder
/// This handles multi-byte UTF-8 characters correctly
fn skip_chars(s: &str, n: usize) -> &str {
    let mut chars = s.chars();
    for _ in 0..n {
        if chars.next().is_none() {
            return "";
        }
    }
    chars.as_str()
}

/// Truncate a string to fit within a given display width
/// This handles multi-byte UTF-8 characters correctly
fn truncate_to_width(s: &str, width: usize) -> String {
    s.chars().take(width).collect()
}
