//! Editor widget for multi-line text editing
//!
//! Provides a text editor that can be embedded as an OutputBlock.

use super::{Widget, WidgetOutput, WidgetResult};
use crate::editor::{HighlightSpan, Highlighter, KqlHighlighter, YamlHighlighter};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use std::sync::OnceLock;

/// Global KQL highlighter (created once, reused)
static KQL_HIGHLIGHTER: OnceLock<KqlHighlighter> = OnceLock::new();

/// Get or create the KQL highlighter
fn get_kql_highlighter() -> &'static KqlHighlighter {
    KQL_HIGHLIGHTER.get_or_init(KqlHighlighter::new)
}

/// Editor mode (affects syntax highlighting, completions)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    /// KQL query editing
    Kql,
    /// YAML input definition editing
    Yaml,
    /// Plain text
    Plain,
}

/// Editor widget state
#[derive(Debug, Clone)]
pub struct EditorWidget {
    /// Name/title of the editor
    pub name: String,
    /// Editor mode
    pub mode: EditorMode,
    /// Lines of text
    pub lines: Vec<String>,
    /// Cursor row (0-indexed)
    pub cursor_row: usize,
    /// Cursor column (0-indexed)
    pub cursor_col: usize,
    /// Scroll offset (first visible line)
    pub scroll_offset: usize,
    /// Visible height (set during render)
    pub visible_height: u16,
    /// Widget result (pending until save/cancel)
    pub result: WidgetResult,
    /// Whether content has been modified
    pub modified: bool,
    /// Whether this is creating a new item (vs editing existing)
    pub is_new: bool,
}

impl EditorWidget {
    /// Create a new editor widget
    pub fn new(name: impl Into<String>, mode: EditorMode) -> Self {
        Self {
            name: name.into(),
            mode,
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            visible_height: 10,
            result: WidgetResult::Pending,
            modified: false,
            is_new: false,
        }
    }

    /// Create an editor with initial content
    pub fn with_content(mut self, content: &str) -> Self {
        self.lines = content.lines().map(String::from).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self
    }

    /// Get the current content as a single string
    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    /// Get syntax highlight spans for the entire content
    fn get_highlight_spans(&self) -> Vec<HighlightSpan> {
        match self.mode {
            EditorMode::Kql => {
                let content = self.content();
                get_kql_highlighter().highlight(&content)
            }
            EditorMode::Yaml => {
                let content = self.content();
                YamlHighlighter::new().highlight(&content)
            }
            EditorMode::Plain => Vec::new(),
        }
    }

    /// Build styled spans for a single line with syntax highlighting
    fn build_highlighted_line_spans(
        &self,
        line: &str,
        line_start_offset: usize,
        highlight_spans: &[HighlightSpan],
        is_current_line: bool,
    ) -> Vec<Span<'static>> {
        let line_end_offset = line_start_offset + line.len();

        // Get all highlight spans that overlap with this line
        let relevant_spans: Vec<_> = highlight_spans
            .iter()
            .filter(|s| {
                let span_end = s.start + s.length;
                s.start < line_end_offset && span_end > line_start_offset
            })
            .collect();

        if relevant_spans.is_empty() {
            // No highlighting for this line - return plain styled text
            let style = if is_current_line {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            return vec![Span::styled(line.to_string(), style)];
        }

        // Build spans by iterating through the line
        let mut result = Vec::new();
        let mut pos = 0;
        let base_style = if is_current_line {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };

        while pos < line.len() {
            // Find the next highlight span that starts at or after our position
            let line_pos = line_start_offset + pos;

            // Find a span that covers this position
            let covering_span = relevant_spans.iter().find(|s| {
                let span_end = s.start + s.length;
                s.start <= line_pos && span_end > line_pos
            });

            if let Some(span) = covering_span {
                // Calculate how much of this span is in our line from current pos
                let span_end = span.start + span.length;
                let text_start = pos;
                let text_end = (span_end - line_start_offset).min(line.len());

                if text_start < text_end {
                    result.push(Span::styled(
                        line[text_start..text_end].to_string(),
                        span.style,
                    ));
                    pos = text_end;
                } else {
                    pos += 1;
                }
            } else {
                // No span covers this position, find the next span start
                let next_span_start = relevant_spans
                    .iter()
                    .filter(|s| s.start > line_pos)
                    .map(|s| s.start - line_start_offset)
                    .min()
                    .unwrap_or(line.len());

                let text_end = next_span_start.min(line.len());
                if pos < text_end {
                    result.push(Span::styled(line[pos..text_end].to_string(), base_style));
                    pos = text_end;
                } else {
                    pos += 1;
                }
            }
        }

        if result.is_empty() {
            result.push(Span::styled(line.to_string(), base_style));
        }

        result
    }

    /// Get the current line
    fn current_line(&self) -> &str {
        &self.lines[self.cursor_row]
    }

    /// Get mutable reference to current line
    fn current_line_mut(&mut self) -> &mut String {
        &mut self.lines[self.cursor_row]
    }

    /// Ensure cursor column is valid for current line
    fn clamp_cursor_col(&mut self) {
        let line_len = self.current_line().len();
        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }

    /// Ensure cursor is visible (adjust scroll)
    fn ensure_visible(&mut self) {
        let visible = self.visible_height.saturating_sub(4) as usize; // Account for borders
        if self.cursor_row < self.scroll_offset {
            self.scroll_offset = self.cursor_row;
        } else if self.cursor_row >= self.scroll_offset + visible {
            self.scroll_offset = self.cursor_row - visible + 1;
        }
    }

    /// Insert a character at cursor
    fn insert_char(&mut self, c: char) {
        let col = self.cursor_col;
        self.lines[self.cursor_row].insert(col, c);
        self.cursor_col += 1;
        self.modified = true;
    }

    /// Delete character before cursor (backspace)
    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            let col = self.cursor_col;
            self.lines[self.cursor_row].remove(col);
            self.modified = true;
        } else if self.cursor_row > 0 {
            // Join with previous line
            let current = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
            self.lines[self.cursor_row].push_str(&current);
            self.modified = true;
        }
    }

    /// Delete character at cursor
    fn delete(&mut self) {
        let line_len = self.lines[self.cursor_row].len();
        if self.cursor_col < line_len {
            let col = self.cursor_col;
            self.lines[self.cursor_row].remove(col);
            self.modified = true;
        } else if self.cursor_row + 1 < self.lines.len() {
            // Join with next line
            let next = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next);
            self.modified = true;
        }
    }

    /// Insert a new line at cursor
    fn insert_newline(&mut self) {
        let col = self.cursor_col;
        let rest = self.lines[self.cursor_row].split_off(col);
        self.cursor_row += 1;
        self.lines.insert(self.cursor_row, rest);
        self.cursor_col = 0;
        self.modified = true;
    }

    /// Move cursor up
    fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.clamp_cursor_col();
            self.ensure_visible();
        }
    }

    /// Move cursor down
    fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.clamp_cursor_col();
            self.ensure_visible();
        }
    }

    /// Move cursor left
    fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.current_line().len();
            self.ensure_visible();
        }
    }

    /// Move cursor right
    fn move_right(&mut self) {
        let line_len = self.current_line().len();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
            self.ensure_visible();
        }
    }

    /// Move to start of line
    fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    /// Move to end of line
    fn move_end(&mut self) {
        self.cursor_col = self.current_line().len();
    }

    /// Render the editor widget
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.visible_height = area.height;

        // Calculate inner area (inside border)
        let inner_height = area.height.saturating_sub(3) as usize; // borders + hint line

        // Build title with modified indicator
        let title = if self.modified {
            format!(" {}* ", self.name)
        } else {
            format!(" {} ", self.name)
        };

        let mode_str = match self.mode {
            EditorMode::Kql => "KQL",
            EditorMode::Yaml => "YAML",
            EditorMode::Plain => "Text",
        };

        // Get syntax highlighting spans for the entire content
        let highlight_spans = self.get_highlight_spans();

        // Calculate line offsets (byte position where each line starts)
        let line_offsets: Vec<usize> = {
            let mut offsets = vec![0];
            let mut offset = 0;
            for line in &self.lines {
                offset += line.len() + 1; // +1 for newline
                offsets.push(offset);
            }
            offsets
        };

        // Build lines with line numbers
        let mut display_lines: Vec<Line> = Vec::new();
        let line_num_width = (self.lines.len().max(1) as f64).log10().floor() as usize + 1;
        let num_style = Style::default().fg(Color::DarkGray);

        for (i, line) in self.lines.iter().enumerate().skip(self.scroll_offset).take(inner_height) {
            let line_num = format!("{:>width$} │ ", i + 1, width = line_num_width);
            let is_current = i == self.cursor_row;
            let line_start_offset = line_offsets.get(i).copied().unwrap_or(0);

            // Build highlighted spans for this line
            let mut spans = vec![Span::styled(line_num, num_style)];

            if is_current && !line.is_empty() {
                // For the current line with cursor, we need to insert cursor within highlighted spans
                let highlighted = self.build_highlighted_line_spans(line, line_start_offset, &highlight_spans, true);

                // Find where to insert cursor
                let cursor_byte_pos = self.cursor_col.min(line.len());

                // Get cursor character
                let cursor_char = if cursor_byte_pos < line.len() {
                    line[cursor_byte_pos..].chars().next().map(|c| c.to_string()).unwrap_or(" ".to_string())
                } else {
                    " ".to_string()
                };
                let cursor_char_len = cursor_char.len();

                // Build spans with cursor inserted
                let mut char_pos = 0;
                let mut cursor_inserted = false;

                for span in highlighted {
                    let span_len = span.content.len();
                    let span_end = char_pos + span_len;

                    if !cursor_inserted && cursor_byte_pos >= char_pos && cursor_byte_pos < span_end {
                        // Cursor is within this span - split it
                        let split_pos = cursor_byte_pos - char_pos;
                        let span_text = span.content.to_string();

                        // Text before cursor
                        if split_pos > 0 {
                            spans.push(Span::styled(span_text[..split_pos].to_string(), span.style));
                        }

                        // Cursor character
                        spans.push(Span::styled(cursor_char.clone(), Style::default().bg(Color::White).fg(Color::Black)));

                        // Text after cursor
                        let after_cursor = split_pos + cursor_char_len;
                        if after_cursor < span_len {
                            spans.push(Span::styled(span_text[after_cursor..].to_string(), span.style));
                        }

                        cursor_inserted = true;
                    } else {
                        spans.push(span);
                    }

                    char_pos = span_end;
                }

                // If cursor is at the end of line (past all spans)
                if !cursor_inserted {
                    spans.push(Span::styled(" ", Style::default().bg(Color::White).fg(Color::Black)));
                }
            } else if is_current && line.is_empty() {
                // Empty current line - just show cursor
                spans.push(Span::styled(" ", Style::default().bg(Color::White).fg(Color::Black)));
            } else {
                // Non-current line - just apply highlighting
                let highlighted = self.build_highlighted_line_spans(line, line_start_offset, &highlight_spans, false);
                spans.extend(highlighted);
            }

            display_lines.push(Line::from(spans));
        }

        // Add hint line
        display_lines.push(Line::from(vec![
            Span::styled(
                format!(" [Ctrl+S Save] [Esc Cancel] │ {} │ Ln {}, Col {} ",
                    mode_str,
                    self.cursor_row + 1,
                    self.cursor_col + 1
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        let block = Block::default()
            .title(title)
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let paragraph = Paragraph::new(display_lines).block(block);
        frame.render_widget(paragraph, area);
    }
}

impl Widget for EditorWidget {
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match (key.modifiers, key.code) {
            // Save: Ctrl+S or Ctrl+D
            (KeyModifiers::CONTROL, KeyCode::Char('s'))
            | (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                self.result = WidgetResult::Saved(WidgetOutput::EditorContent {
                    name: self.name.clone(),
                    content: self.content(),
                    is_new: self.is_new,
                });
                true
            }

            // Cancel: Escape
            (_, KeyCode::Esc) => {
                self.result = WidgetResult::Cancelled;
                true
            }

            // Navigation
            (_, KeyCode::Up) => {
                self.move_up();
                true
            }
            (_, KeyCode::Down) => {
                self.move_down();
                true
            }
            (_, KeyCode::Left) => {
                self.move_left();
                true
            }
            (_, KeyCode::Right) => {
                self.move_right();
                true
            }
            (_, KeyCode::Home) | (KeyModifiers::CONTROL, KeyCode::Char('a')) => {
                self.move_home();
                true
            }
            (_, KeyCode::End) | (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
                self.move_end();
                true
            }

            // Editing
            (_, KeyCode::Enter) => {
                self.insert_newline();
                self.ensure_visible();
                true
            }
            (_, KeyCode::Backspace) => {
                self.backspace();
                self.ensure_visible();
                true
            }
            (_, KeyCode::Delete) => {
                self.delete();
                true
            }
            (_, KeyCode::Tab) => {
                // Insert 4 spaces for tab
                for _ in 0..4 {
                    self.insert_char(' ');
                }
                true
            }

            // Character input
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.insert_char(c);
                true
            }

            _ => false,
        }
    }

    fn result(&self) -> WidgetResult {
        self.result.clone()
    }

    fn height(&self) -> u16 {
        // Minimum height: borders (2) + at least 5 lines + hint (1)
        let content_lines = self.lines.len().min(15).max(5) as u16;
        content_lines + 3
    }
}
