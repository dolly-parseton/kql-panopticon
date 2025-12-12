//! Inline input form widget for collecting input values before execution.
//!
//! Uses ratatui's inline viewport to render a form without taking over the terminal.

use super::field::InputField;
use crate::session::{InputDef, InputType};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};
use std::collections::HashMap;
use std::io::{self, stdout, Stdout, Write};

/// Minimum width for the form
const MIN_WIDTH: u16 = 30;
/// Maximum width for the form
const MAX_WIDTH: u16 = 70;
/// Padding inside the border (left/right)
const HORIZONTAL_PADDING: u16 = 2;
/// Padding inside the border (top/bottom)
const VERTICAL_PADDING: u16 = 0;

/// Result of the input form
#[derive(Debug, Clone)]
pub enum InputFormResult {
    /// User submitted the form with input values
    Submitted(HashMap<String, String>),
    /// User cancelled the form
    Cancelled,
}

/// Inline input form widget
pub struct InputForm {
    /// Title shown at the top
    title: String,
    /// Input fields
    fields: Vec<InputField>,
    /// Currently focused field index
    focused: usize,
    /// Whether the Execute button is focused
    button_focused: bool,
    /// Calculated form width (set during run)
    form_width: Option<u16>,
}

impl InputForm {
    /// Create a new input form from input definitions
    pub fn new(title: impl Into<String>, inputs: &[InputDef]) -> Self {
        let fields = inputs
            .iter()
            .map(|input| {
                InputField::new(
                    input.name.clone(),
                    input.input_type.clone(),
                    input.required,
                    input.default.clone(),
                    input.description.clone(),
                )
            })
            .collect();

        Self {
            title: title.into(),
            fields,
            focused: 0,
            button_focused: false,
            form_width: None,
        }
    }

    /// Create a form from a list of (name, type, required, default) tuples
    pub fn from_simple(
        title: impl Into<String>,
        inputs: Vec<(String, InputType, bool, Option<String>)>,
    ) -> Self {
        let fields = inputs
            .into_iter()
            .map(|(name, input_type, required, default)| {
                InputField::new(name, input_type, required, default, None)
            })
            .collect();

        Self {
            title: title.into(),
            fields,
            focused: 0,
            button_focused: false,
            form_width: None,
        }
    }

    /// Calculate the height needed for the form
    fn required_height(&self) -> u16 {
        // border top (1) + fields (2 each: label + input) + spacing + button (1) + border bottom (1) + hints (1)
        let field_lines: u16 = self
            .fields
            .iter()
            .map(|f| if f.error_message().is_some() { 3 } else { 2 })
            .sum();
        1 + field_lines + 1 + 1 + 1 + 1
    }

    /// Calculate content-aware width based on field names and values
    fn required_width(&self) -> u16 {
        // Calculate max content width
        let max_field_width: u16 = self
            .fields
            .iter()
            .map(|f| {
                let type_info = format_type_info(f);
                let label_len = f.name.len() + type_info.len() + 3; // "  name type_info"
                let content_len = f.content().len() + 6; // "  > content"
                label_len.max(content_len) as u16
            })
            .max()
            .unwrap_or(20);

        // Title width
        let title_width = self.title.len() as u16 + 4; // " title "

        // Button and hints width
        let button_width: u16 = 25; // "[ Execute ] (fix errors)"
        let hints_width: u16 = 50; // hint text

        // Find maximum content width
        let content_width = max_field_width
            .max(title_width)
            .max(button_width)
            .max(hints_width);

        // Add border and padding: border(2) + padding(2*HORIZONTAL_PADDING)
        let total_width = content_width + 2 + (HORIZONTAL_PADDING * 2);

        // Clamp to min/max
        total_width.clamp(MIN_WIDTH, MAX_WIDTH)
    }

    /// Run the input form and return the result
    pub fn run(mut self) -> io::Result<InputFormResult> {
        use crossterm::{cursor, terminal as ct, ExecutableCommand};

        // Calculate viewport dimensions
        let height = self.required_height().min(24);
        let width = self.required_width();

        // Add spacing before the form
        println!();

        // Initialize inline terminal
        terminal::enable_raw_mode()?;
        let out = stdout();
        let backend = CrosstermBackend::new(out);
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(height),
            },
        )?;

        // Store width for rendering
        self.form_width = Some(width);

        // Clear any pending events
        while event::poll(std::time::Duration::from_millis(0))? {
            let _ = event::read()?;
        }

        let result = self.run_loop(&mut terminal);

        // Cleanup - drop terminal first to flush viewport
        drop(terminal);
        terminal::disable_raw_mode()?;

        // Clear the form area by moving up and clearing each line
        let mut out = stdout();
        out.execute(cursor::MoveUp(height))?;
        for _ in 0..height {
            out.execute(ct::Clear(ct::ClearType::CurrentLine))?;
            out.execute(cursor::MoveDown(1))?;
        }
        // Move back to start position
        out.execute(cursor::MoveUp(height))?;
        out.execute(cursor::MoveToColumn(0))?;
        out.flush()?;

        result
    }

    /// Main event loop
    fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> io::Result<InputFormResult> {
        loop {
            // Draw
            terminal.draw(|frame| self.render(frame))?;

            // Handle input
            if let Event::Key(key) = event::read()? {
                match self.handle_key(key) {
                    KeyAction::Continue => {}
                    KeyAction::Submit => {
                        if self.can_submit() {
                            return Ok(InputFormResult::Submitted(self.collect_values()));
                        }
                        // If can't submit, just continue (errors shown inline)
                    }
                    KeyAction::Cancel => {
                        return Ok(InputFormResult::Cancelled);
                    }
                }
            }
        }
    }

    /// Render the form
    fn render(&self, frame: &mut Frame) {
        let full_area = frame.area();

        // Clear the full area
        frame.render_widget(Clear, full_area);

        // Calculate the form area based on form_width
        let form_width = self.form_width.unwrap_or(full_area.width);
        let area = Rect {
            x: full_area.x,
            y: full_area.y,
            width: form_width.min(full_area.width),
            height: full_area.height,
        };

        // Main block
        let block = Block::default()
            .title(format!(" {} ", self.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Create padded inner area
        let padded_inner = Rect {
            x: inner.x + HORIZONTAL_PADDING,
            y: inner.y + VERTICAL_PADDING,
            width: inner.width.saturating_sub(HORIZONTAL_PADDING * 2),
            height: inner.height.saturating_sub(VERTICAL_PADDING * 2),
        };

        // Layout for fields + button + hints
        let total_field_height: u16 = self
            .fields
            .iter()
            .map(|f| if f.error_message().is_some() { 3 } else { 2 })
            .sum();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(total_field_height),
                Constraint::Length(2), // Button + spacing
                Constraint::Length(1), // Hints
            ])
            .split(padded_inner);

        // Render fields
        self.render_fields(frame, chunks[0]);

        // Render button
        self.render_button(frame, chunks[1]);

        // Render hints
        self.render_hints(frame, chunks[2]);
    }

    /// Render all input fields
    fn render_fields(&self, frame: &mut Frame, area: Rect) {
        let mut y = area.y;

        for (i, field) in self.fields.iter().enumerate() {
            let is_focused = i == self.focused && !self.button_focused;
            let has_error = field.error_message().is_some();
            let field_height = if has_error { 3 } else { 2 };

            let field_area = Rect {
                x: area.x,
                y,
                width: area.width,
                height: field_height,
            };

            self.render_field(frame, field, is_focused, field_area);
            y += field_height;
        }
    }

    /// Render a single field
    fn render_field(&self, frame: &mut Frame, field: &InputField, is_focused: bool, area: Rect) {
        // Label line: "  name [type, required/default: x]"
        let type_info = format_type_info(field);
        let label_style = if is_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let label = Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(&field.name, label_style.add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {}", type_info), Style::default().fg(Color::DarkGray)),
        ]);

        frame.render_widget(
            Paragraph::new(label),
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
        );

        // Input line: "  > content_"
        let input_style = if is_focused {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };

        let prompt_style = if is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        // Build input line with cursor
        let content = field.content();
        let cursor_pos = field.cursor();

        let (before_cursor, after_cursor) = if cursor_pos <= content.len() {
            (&content[..cursor_pos], &content[cursor_pos..])
        } else {
            (content, "")
        };

        let mut spans = vec![
            Span::styled("  ", Style::default()),
            Span::styled("> ", prompt_style),
            Span::styled(before_cursor, input_style),
        ];

        if is_focused {
            // Show cursor
            let cursor_char = after_cursor.chars().next().unwrap_or(' ');
            spans.push(Span::styled(
                cursor_char.to_string(),
                Style::default().bg(Color::White).fg(Color::Black),
            ));
            if after_cursor.len() > cursor_char.len_utf8() {
                spans.push(Span::styled(
                    &after_cursor[cursor_char.len_utf8()..],
                    input_style,
                ));
            }
        } else {
            spans.push(Span::styled(after_cursor, input_style));
        }

        let input_line = Line::from(spans);
        frame.render_widget(
            Paragraph::new(input_line),
            Rect {
                x: area.x,
                y: area.y + 1,
                width: area.width,
                height: 1,
            },
        );

        // Error line (if present)
        if let Some(error) = field.error_message() {
            let error_line = Line::from(vec![
                Span::styled("     ", Style::default()),
                Span::styled(error, Style::default().fg(Color::Red)),
            ]);
            frame.render_widget(
                Paragraph::new(error_line),
                Rect {
                    x: area.x,
                    y: area.y + 2,
                    width: area.width,
                    height: 1,
                },
            );
        }
    }

    /// Render the Execute button
    fn render_button(&self, frame: &mut Frame, area: Rect) {
        let can_submit = self.can_submit();
        let style = if self.button_focused {
            if can_submit {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::DarkGray)
            }
        } else if can_submit {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let button_text = if can_submit {
            "[ Execute ]"
        } else {
            "[ Execute ] (fix errors)"
        };

        let button = Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(button_text, style),
        ]);

        frame.render_widget(
            Paragraph::new(button),
            Rect {
                x: area.x,
                y: area.y + 1,
                width: area.width,
                height: 1,
            },
        );
    }

    /// Render keybinding hints
    fn render_hints(&self, frame: &mut Frame, area: Rect) {
        let hints = Line::from(vec![
            Span::styled(
                "Tab",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" next  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Shift+Tab",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" prev  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Ctrl+Enter",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" submit  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
        ]);

        frame.render_widget(Paragraph::new(hints), area);
    }

    /// Handle a key event
    fn handle_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            // Cancel
            KeyCode::Esc => KeyAction::Cancel,

            // Submit
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => KeyAction::Submit,
            KeyCode::Enter if self.button_focused => KeyAction::Submit,

            // Navigation
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.focus_previous();
                KeyAction::Continue
            }
            KeyCode::Tab => {
                self.focus_next();
                KeyAction::Continue
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                self.focus_previous();
                KeyAction::Continue
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                self.focus_next();
                KeyAction::Continue
            }

            // Field editing (only when not on button)
            _ if !self.button_focused => {
                self.handle_field_key(key);
                KeyAction::Continue
            }

            _ => KeyAction::Continue,
        }
    }

    /// Handle a key event for the focused field
    fn handle_field_key(&mut self, key: KeyEvent) {
        let Some(field) = self.fields.get_mut(self.focused) else {
            return;
        };

        match key.code {
            // Text editing
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                match c {
                    'a' => field.move_to_start(),
                    'e' => field.move_to_end(),
                    'k' => field.delete_to_end(),
                    'u' => field.delete_to_start(),
                    'w' => {
                        // Delete word before cursor
                        field.move_word_left();
                        // Would need more complex implementation for delete word
                    }
                    _ => {}
                }
            }
            KeyCode::Char(c) => {
                field.insert_char(c);
            }
            KeyCode::Backspace => {
                field.delete_char_before();
            }
            KeyCode::Delete => {
                field.delete_char_at();
            }

            // Cursor movement
            KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                field.move_word_left();
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                field.move_word_right();
            }
            KeyCode::Left => {
                field.move_left();
            }
            KeyCode::Right => {
                field.move_right();
            }
            KeyCode::Home => {
                field.move_to_start();
            }
            KeyCode::End => {
                field.move_to_end();
            }

            // Enter on field = next field
            KeyCode::Enter => {
                self.focus_next();
            }

            _ => {}
        }
    }

    /// Focus the next field or button
    fn focus_next(&mut self) {
        if self.button_focused {
            // Wrap to first field
            self.button_focused = false;
            self.focused = 0;
        } else if self.focused + 1 < self.fields.len() {
            self.focused += 1;
        } else {
            // Move to button
            self.button_focused = true;
        }
    }

    /// Focus the previous field or button
    fn focus_previous(&mut self) {
        if self.button_focused {
            self.button_focused = false;
            // Stay on last field
        } else if self.focused > 0 {
            self.focused -= 1;
        } else {
            // Wrap to button
            self.button_focused = true;
        }
    }

    /// Check if all fields are valid for submission
    fn can_submit(&self) -> bool {
        self.fields.iter().all(|f| f.is_valid_for_submit())
    }

    /// Collect all field values into a HashMap
    fn collect_values(&self) -> HashMap<String, String> {
        self.fields
            .iter()
            .filter_map(|f| f.submit_value().map(|v| (f.name.clone(), v)))
            .collect()
    }
}

/// Format type information for display
fn format_type_info(field: &InputField) -> String {
    let type_str = match &field.input_type {
        InputType::String => "string",
        InputType::Int => "int",
        InputType::Bool => "bool",
        InputType::Datetime => "datetime",
        InputType::Timespan => "timespan",
    };

    let req_str = if field.required {
        "required"
    } else if let Some(default) = &field.default {
        &format!("default: {}", default)
    } else {
        "optional"
    };

    format!("[{}, {}]", type_str, req_str)
}

/// Action to take after handling a key
enum KeyAction {
    Continue,
    Submit,
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_form_creation() {
        let form = InputForm::from_simple(
            "Test",
            vec![
                ("name".to_string(), InputType::String, true, None),
                (
                    "count".to_string(),
                    InputType::Int,
                    false,
                    Some("10".to_string()),
                ),
            ],
        );

        assert_eq!(form.fields.len(), 2);
        assert_eq!(form.fields[0].name, "name");
        assert_eq!(form.fields[1].name, "count");
    }

    #[test]
    fn test_can_submit() {
        let mut form = InputForm::from_simple(
            "Test",
            vec![
                ("required".to_string(), InputType::String, true, None),
                ("optional".to_string(), InputType::String, false, None),
            ],
        );

        // Can't submit with empty required field
        assert!(!form.can_submit());

        // Fill required field
        form.fields[0].insert_str("value");
        assert!(form.can_submit());
    }

    #[test]
    fn test_collect_values() {
        let mut form = InputForm::from_simple(
            "Test",
            vec![
                ("a".to_string(), InputType::String, true, None),
                (
                    "b".to_string(),
                    InputType::String,
                    false,
                    Some("default".to_string()),
                ),
            ],
        );

        form.fields[0].insert_str("value_a");

        let values = form.collect_values();
        assert_eq!(values.get("a"), Some(&"value_a".to_string()));
        assert_eq!(values.get("b"), Some(&"default".to_string()));
    }
}
