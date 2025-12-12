//! Core TUI editor widget
//!
//! Provides a full-screen text editor based on tui-textarea with:
//! - Title bar showing the editor purpose
//! - Line numbers
//! - Status bar with cursor position and keybinding hints
//! - Configurable syntax highlighting
//! - Completion popup support

use super::completion::{CompletionItem, CompletionPopup, CompletionSource};
use super::highlight::Highlighter;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use std::io::{self, Stdout, Write};
use tui_textarea::{CursorMove, TextArea};

/// Debug logger for completion troubleshooting
/// Writes to /tmp/kql-repl-debug.log
fn debug_log(msg: &str) {
    use std::fs::OpenOptions;
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/kql-repl-debug.log")
    {
        let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(file, "[{}] {}", timestamp, msg);
    }
}

/// Result of running the editor
#[derive(Debug, Clone)]
pub enum EditorResult {
    /// User saved with Ctrl+D
    Saved(String),
    /// User cancelled with Esc
    Cancelled,
}

/// Editor configuration
#[derive(Debug, Clone)]
pub struct EditorConfig {
    /// Title shown in the title bar (e.g., "query: analysis")
    pub title: String,
    /// Mode label shown in status bar (e.g., "KQL", "YAML")
    pub mode: String,
    /// Initial content
    pub initial_content: String,
}

impl EditorConfig {
    /// Create a new editor config for KQL editing
    pub fn kql(name: &str) -> Self {
        Self {
            title: format!("query: {}", name),
            mode: "KQL".to_string(),
            initial_content: String::new(),
        }
    }

    /// Create a new editor config for YAML input definition
    pub fn yaml_input(name: &str) -> Self {
        Self {
            title: format!("input: {}", name),
            mode: "YAML".to_string(),
            initial_content: format!(
                r#"name: {}
type: string        # string | int | bool | datetime | timespan
description: ""
required: true
default: null
"#,
                name
            ),
        }
    }

    /// Set initial content
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.initial_content = content.into();
        self
    }
}

/// The main TUI editor widget
pub struct TuiEditor<'a, H: Highlighter, C: CompletionSource> {
    /// The underlying textarea
    textarea: TextArea<'a>,
    /// Configuration
    config: EditorConfig,
    /// Syntax highlighter
    highlighter: H,
    /// Completion source
    completion_source: C,
    /// Completion popup state
    completion_popup: Option<CompletionPopup>,
    /// Whether completion is active
    completion_active: bool,
    /// Status message (for errors/warnings)
    status_message: Option<(String, Style)>,
}

impl<'a, H: Highlighter, C: CompletionSource> TuiEditor<'a, H, C> {
    /// Create a new editor
    pub fn new(config: EditorConfig, highlighter: H, completion_source: C) -> Self {
        let mut textarea = TextArea::default();

        // Set initial content
        if !config.initial_content.is_empty() {
            let lines: Vec<String> = config.initial_content.lines().map(String::from).collect();
            textarea = TextArea::new(lines);
        }

        // Configure textarea appearance
        textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
        textarea.set_line_number_style(Style::default().fg(Color::DarkGray));

        Self {
            textarea,
            config,
            highlighter,
            completion_source,
            completion_popup: None,
            completion_active: false,
            status_message: None,
        }
    }

    /// Run the editor, taking over the terminal
    pub fn run(mut self) -> io::Result<EditorResult> {
        // Setup terminal
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        stdout.execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Clear any pending events before starting
        while event::poll(std::time::Duration::from_millis(0))? {
            let _ = event::read()?;
        }

        let result = self.run_loop(&mut terminal);

        // Restore terminal
        terminal::disable_raw_mode()?;
        terminal.backend_mut().execute(LeaveAlternateScreen)?;

        result
    }

    /// Main editor loop
    fn run_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<EditorResult> {
        loop {
            // Draw UI (highlighting is computed during render)
            terminal.draw(|frame| {
                self.render(frame, frame.area());
            })?;

            // Handle input
            if let Event::Key(key) = event::read()? {
                match self.handle_key(key) {
                    KeyAction::Continue => {}
                    KeyAction::Save => {
                        let content = self.textarea.lines().join("\n");
                        return Ok(EditorResult::Saved(content));
                    }
                    KeyAction::Cancel => {
                        return Ok(EditorResult::Cancelled);
                    }
                }
            }
        }
    }

    /// Build highlighted lines from content
    fn build_highlighted_lines(&self) -> Vec<Line<'static>> {
        let content = self.textarea.lines().join("\n");
        let highlight_spans = self.highlighter.highlight(&content);

        // Debug: log span count
        if !highlight_spans.is_empty() {
            log::trace!("build_highlighted_lines: {} highlight spans for {} chars", highlight_spans.len(), content.len());
        }

        let lines: Vec<&str> = self.textarea.lines().iter().map(|s| s.as_str()).collect();

        // Build style spans for each line
        let mut result = Vec::new();
        let mut offset = 0;

        for line in &lines {
            let line_start = offset;
            let line_end = offset + line.len();

            // Collect spans that apply to this line
            let mut line_spans: Vec<(usize, usize, Style)> = Vec::new();

            for span in &highlight_spans {
                let span_start = span.start;
                let span_end = span.start + span.length;

                // Check if span overlaps with this line
                if span_end > line_start && span_start < line_end {
                    let start_in_line = span_start.saturating_sub(line_start);
                    let end_in_line = (span_end - line_start).min(line.len());

                    if start_in_line < end_in_line {
                        line_spans.push((start_in_line, end_in_line, span.style));
                    }
                }
            }

            // Sort spans by start position
            line_spans.sort_by_key(|(start, _, _)| *start);

            // Build Line with styled spans
            let mut spans = Vec::new();
            let mut last_end = 0;

            // Debug: log line spans
            if !line_spans.is_empty() {
                log::trace!("Line {}: {} highlight spans for '{}'", result.len(), line_spans.len(), line);
            }

            for (start, end, style) in line_spans {
                // Add unstyled text before this span
                if start > last_end {
                    spans.push(Span::raw(line[last_end..start].to_string()));
                }
                // Add styled span
                let styled_text = &line[start..end];
                log::trace!("  Applying style to '{}' at {}..{}", styled_text, start, end);
                spans.push(Span::styled(styled_text.to_string(), style));
                last_end = end;
            }

            // Add remaining unstyled text
            if last_end < line.len() {
                spans.push(Span::raw(line[last_end..].to_string()));
            }

            // If no spans, just add the whole line
            if spans.is_empty() {
                spans.push(Span::raw(line.to_string()));
            }

            result.push(Line::from(spans));
            offset = line_end + 1; // +1 for newline
        }

        result
    }

    /// Render the editor UI
    fn render(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        // Layout: title bar, editor area, status bar
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Title bar
                Constraint::Min(3),    // Editor area
                Constraint::Length(1), // Status bar
            ])
            .split(area);

        // Title bar
        let title = Line::from(vec![
            Span::styled(" ", Style::default().bg(Color::Blue)),
            Span::styled(
                format!(" {} ", self.config.title),
                Style::default().fg(Color::White).bg(Color::Blue),
            ),
            Span::styled(
                " ".repeat(area.width.saturating_sub(self.config.title.len() as u16 + 3) as usize),
                Style::default().bg(Color::Blue),
            ),
        ]);
        frame.render_widget(Paragraph::new(title), chunks[0]);

        // Editor area with syntax highlighting
        self.render_editor(frame, chunks[1]);

        // Status bar
        self.render_status_bar(frame, chunks[2]);

        // Completion popup (if active)
        if self.completion_active {
            if let Some(popup) = &mut self.completion_popup {
                popup.render(frame, chunks[1]);
            }
        }
    }

    /// Render the editor content with syntax highlighting
    fn render_editor(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        // Create block for the editor
        let block = Block::default().borders(Borders::ALL).border_style(
            Style::default().fg(Color::DarkGray),
        );

        // Get the inner area (inside the block borders)
        let inner_area = block.inner(area);

        // Render the block border first
        frame.render_widget(block, area);

        // Build highlighted lines
        let highlighted_lines = self.build_highlighted_lines();

        // Get cursor position
        let (cursor_row, cursor_col) = self.textarea.cursor();

        // Calculate visible line range (simple scrolling)
        let visible_height = inner_area.height as usize;
        let total_lines = highlighted_lines.len();

        // Calculate scroll offset to keep cursor visible
        let scroll_offset = if cursor_row >= visible_height {
            cursor_row - visible_height + 1
        } else {
            0
        };

        // Build lines with line numbers and cursor indicator
        let mut display_lines: Vec<Line<'static>> = Vec::new();

        for (i, line) in highlighted_lines.iter().enumerate().skip(scroll_offset).take(visible_height) {
            let line_num = i + 1;

            // Build line number with padding
            let line_num_str = format!("{:>4} ", line_num);
            let line_num_span = Span::styled(
                line_num_str,
                Style::default().fg(Color::DarkGray),
            );

            // Clone the line's spans and add cursor if on this line
            let mut spans = vec![line_num_span];

            if i == cursor_row {
                // Cursor line - preserve highlighting but add cursor indicator
                // We need to split the spans at the cursor position and insert cursor styling
                let line_content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                let cursor_pos = cursor_col.min(line_content.len());

                // Build spans by walking through original spans and splitting at cursor
                let mut char_offset = 0;
                let mut cursor_inserted = false;

                for original_span in &line.spans {
                    let span_text = original_span.content.as_ref();
                    let span_start = char_offset;
                    let span_end = char_offset + span_text.len();

                    if !cursor_inserted && cursor_pos >= span_start && cursor_pos < span_end {
                        // Cursor is within this span - split it
                        let relative_pos = cursor_pos - span_start;

                        // Text before cursor (with original style)
                        if relative_pos > 0 {
                            spans.push(Span::styled(
                                span_text[..relative_pos].to_string(),
                                original_span.style,
                            ));
                        }

                        // Cursor character (inverted)
                        let cursor_char = if relative_pos < span_text.len() {
                            &span_text[relative_pos..relative_pos + 1]
                        } else {
                            " "
                        };
                        spans.push(Span::styled(
                            cursor_char.to_string(),
                            Style::default().bg(Color::White).fg(Color::Black),
                        ));

                        // Text after cursor (with original style)
                        if relative_pos + 1 < span_text.len() {
                            spans.push(Span::styled(
                                span_text[relative_pos + 1..].to_string(),
                                original_span.style,
                            ));
                        }

                        cursor_inserted = true;
                    } else if !cursor_inserted && cursor_pos == span_end {
                        // Cursor is right after this span
                        spans.push(original_span.clone());
                    } else {
                        // Regular span (cursor not here)
                        spans.push(original_span.clone());
                    }

                    char_offset = span_end;
                }

                // If cursor is at the end of line (after all spans)
                if !cursor_inserted {
                    spans.push(Span::styled(
                        " ".to_string(),
                        Style::default().bg(Color::White).fg(Color::Black),
                    ));
                }
            } else {
                // Non-cursor line - use highlighted spans
                spans.extend(line.spans.iter().cloned());
            }

            display_lines.push(Line::from(spans));
        }

        // Add empty lines with line numbers if content is shorter than visible area
        for i in total_lines..scroll_offset + visible_height {
            let line_num_span = Span::styled(
                format!("{:>4} ", i + 1),
                Style::default().fg(Color::DarkGray),
            );
            display_lines.push(Line::from(vec![line_num_span, Span::styled("~", Style::default().fg(Color::Gray).add_modifier(Modifier::DIM))]));
        }

        // Render the content
        let paragraph = Paragraph::new(display_lines);
        frame.render_widget(paragraph, inner_area);
    }

    /// Render the status bar
    fn render_status_bar(&self, frame: &mut ratatui::Frame, area: Rect) {
        let (row, col) = self.textarea.cursor();
        let cursor_info = format!("Ln {}, Col {}", row + 1, col + 1);

        // Build status bar content
        let status = if let Some((msg, style)) = &self.status_message {
            Line::from(vec![
                Span::styled(
                    format!(" {} ", cursor_info),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("│ "),
                Span::styled(
                    format!("{} ", self.config.mode),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw("│ "),
                Span::styled(msg.clone(), *style),
            ])
        } else {
            Line::from(vec![
                Span::styled(
                    format!(" {} ", cursor_info),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("│ "),
                Span::styled(
                    format!("{} ", self.config.mode),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw("│ "),
                Span::styled("[Ctrl+D save]", Style::default().fg(Color::Green)),
                Span::raw(" "),
                Span::styled("[Esc cancel]", Style::default().fg(Color::Yellow)),
            ])
        };

        frame.render_widget(Paragraph::new(status), area);
    }

    /// Handle a key event
    fn handle_key(&mut self, key: KeyEvent) -> KeyAction {
        // Check for global keybindings first
        match (key.modifiers, key.code) {
            // Ctrl+D: Save
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                return KeyAction::Save;
            }
            // Esc: Cancel (or dismiss completion)
            (KeyModifiers::NONE, KeyCode::Esc) => {
                if self.completion_active {
                    self.completion_active = false;
                    self.completion_popup = None;
                } else {
                    return KeyAction::Cancel;
                }
            }
            // Tab: Trigger or accept completion
            (KeyModifiers::NONE, KeyCode::Tab) => {
                if self.completion_active {
                    self.accept_completion();
                } else {
                    self.trigger_completion();
                }
                return KeyAction::Continue;
            }
            // Enter: Accept completion if active, otherwise insert newline
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if self.completion_active {
                    self.accept_completion();
                    return KeyAction::Continue;
                }
                // Fall through to textarea handling
            }
            // Up/Down: Navigate completion if active (handle any modifier combo for arrow keys)
            (_, KeyCode::Up) if self.completion_active => {
                if let Some(popup) = &mut self.completion_popup {
                    popup.move_selection(-1);
                }
                return KeyAction::Continue;
            }
            (_, KeyCode::Down) if self.completion_active => {
                if let Some(popup) = &mut self.completion_popup {
                    popup.move_selection(1);
                }
                return KeyAction::Continue;
            }
            // Ctrl+N/Ctrl+P for completion navigation (vim-style)
            (KeyModifiers::CONTROL, KeyCode::Char('n')) if self.completion_active => {
                if let Some(popup) = &mut self.completion_popup {
                    popup.move_selection(1);
                }
                return KeyAction::Continue;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('p')) if self.completion_active => {
                if let Some(popup) = &mut self.completion_popup {
                    popup.move_selection(-1);
                }
                return KeyAction::Continue;
            }
            _ => {}
        }

        // Handle completion dismissal on most typing
        if self.completion_active && !matches!(key.code, KeyCode::Up | KeyCode::Down | KeyCode::Tab | KeyCode::Enter) {
            self.completion_active = false;
            self.completion_popup = None;
        }

        // Pass to textarea for default handling
        self.textarea.input(key);

        // Check for completion triggers after typing
        self.check_completion_trigger();

        KeyAction::Continue
    }

    /// Check if we should trigger completion
    fn check_completion_trigger(&mut self) {
        let (row, col) = self.textarea.cursor();
        let line = self.textarea.lines()[row].clone();

        // Check for `{{` trigger for references
        if col >= 2 && line.len() >= col {
            let prefix = &line[col.saturating_sub(2)..col];
            if prefix == "{{" {
                self.trigger_completion();
                return;
            }
        }

        // Check for `|` trigger for operators (after pipe)
        if col >= 1 && line.len() >= col {
            let char_before = line.chars().nth(col - 1);
            if char_before == Some('|') {
                self.trigger_completion();
            }
        }
    }

    /// Trigger completion popup
    fn trigger_completion(&mut self) {
        let content = self.textarea.lines().join("\n");
        let (row, col) = self.textarea.cursor();

        // Calculate byte offset
        let mut offset = 0;
        for (i, line) in self.textarea.lines().iter().enumerate() {
            if i < row {
                offset += line.len() + 1; // +1 for newline
            } else {
                offset += col;
                break;
            }
        }

        // Get completions from source
        let items = self.completion_source.get_completions(&content, offset);

        if !items.is_empty() {
            self.completion_popup = Some(CompletionPopup::new(items, row, col));
            self.completion_active = true;
        }
    }

    /// Accept the currently selected completion
    fn accept_completion(&mut self) {
        debug_log("=== accept_completion START ===");

        if let Some(popup) = &self.completion_popup {
            if let Some(item) = popup.selected_item() {
                // Get the text to insert from the trait method
                let insert_text = item.insert_text();
                let label = item.label();
                debug_log(&format!("Selected item label: '{}'", label));
                debug_log(&format!("Insert text: '{}'", insert_text));

                let (row, col) = self.textarea.cursor();
                let line = self.textarea.lines()[row].clone();
                debug_log(&format!("Cursor: row={}, col={}", row, col));
                debug_log(&format!("Current line: '{}'", line));

                // Work with characters, not bytes
                let chars: Vec<char> = line.chars().collect();
                let chars_before_cursor: String = chars[..col.min(chars.len())].iter().collect();
                debug_log(&format!("Chars before cursor: '{}'", chars_before_cursor));

                if col >= 2 && chars_before_cursor.ends_with("{{") {
                    debug_log("Reference completion mode (after `{{`)");
                    self.textarea.insert_str(insert_text);
                    self.textarea.insert_str("}}");
                } else {
                    // Find where the current word starts
                    let mut word_start_col = 0;
                    for (i, c) in chars_before_cursor.chars().rev().enumerate() {
                        if c.is_whitespace() || c == '|' || c == '(' || c == ',' {
                            word_start_col = col - i;
                            break;
                        }
                    }
                    debug_log(&format!("word_start_col: {}", word_start_col));

                    let chars_to_delete = col.saturating_sub(word_start_col);
                    debug_log(&format!("chars_to_delete: {}", chars_to_delete));
                    debug_log(&format!("BEFORE - lines: {:?}", self.textarea.lines()));

                    // delete_char() is BACKSPACE (deletes char before cursor)
                    // So just call it N times from current position to delete the word backwards
                    for _ in 0..chars_to_delete {
                        self.textarea.delete_char();
                    }
                    debug_log(&format!("AFTER deletion - lines: {:?}", self.textarea.lines()));
                    debug_log(&format!("Cursor after deletion: {:?}", self.textarea.cursor()));

                    // Insert the completion
                    self.textarea.insert_str(insert_text);
                    debug_log(&format!("AFTER insert - lines: {:?}", self.textarea.lines()));
                }
            }
        }

        self.completion_active = false;
        self.completion_popup = None;
        debug_log("=== accept_completion END ===");
    }

    /// Set a status message
    #[allow(dead_code)]
    pub fn set_status(&mut self, message: impl Into<String>, style: Style) {
        self.status_message = Some((message.into(), style));
    }

    /// Clear status message
    #[allow(dead_code)]
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}

/// Action to take after handling a key
enum KeyAction {
    /// Continue editing
    Continue,
    /// Save and exit
    Save,
    /// Cancel and exit
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::highlight::NoOpHighlighter;
    use crate::editor::completion::NoOpCompletionSource;

    #[test]
    fn test_editor_config_kql() {
        let config = EditorConfig::kql("analysis");
        assert_eq!(config.title, "query: analysis");
        assert_eq!(config.mode, "KQL");
        assert!(config.initial_content.is_empty());
    }

    #[test]
    fn test_editor_config_yaml() {
        let config = EditorConfig::yaml_input("threat_ip");
        assert_eq!(config.title, "input: threat_ip");
        assert_eq!(config.mode, "YAML");
        assert!(config.initial_content.contains("name: threat_ip"));
        assert!(config.initial_content.contains("type: string"));
    }

    #[test]
    fn test_editor_config_with_content() {
        let config = EditorConfig::kql("test").with_content("SecurityEvent | take 10");
        assert_eq!(config.initial_content, "SecurityEvent | take 10");
    }
}
