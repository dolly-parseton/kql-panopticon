//! Input form widget for collecting input values
//!
//! Provides an embedded form for collecting input values before execution.

use super::{Widget, WidgetOutput, WidgetResult};
use crate::input_form::field::InputField;
use crate::session::InputDef;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use std::collections::HashMap;

/// Input form widget for collecting execution inputs
#[derive(Debug, Clone)]
pub struct InputFormWidget {
    /// Title/name of the form
    pub name: String,
    /// Input fields
    fields: Vec<InputField>,
    /// Currently focused field index
    focused: usize,
    /// Whether the submit button is focused
    button_focused: bool,
    /// Widget result (pending until submitted/cancelled)
    pub result: WidgetResult,
}

impl InputFormWidget {
    /// Create a new input form widget
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            fields: Vec::new(),
            focused: 0,
            button_focused: false,
            result: WidgetResult::Pending,
        }
    }

    /// Create from input definitions
    pub fn with_inputs(mut self, inputs: &[InputDef]) -> Self {
        self.fields = inputs
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
        self
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

    /// Get current values (public accessor for event state capture)
    pub fn get_values(&self) -> HashMap<String, String> {
        self.collect_values()
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

    /// Handle a key event for the focused field
    fn handle_field_key(&mut self, key: KeyEvent) {
        let Some(field) = self.fields.get_mut(self.focused) else {
            return;
        };

        match key.code {
            // Text editing with Ctrl modifiers
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => match c {
                'a' => field.move_to_start(),
                'e' => field.move_to_end(),
                'k' => field.delete_to_end(),
                'u' => field.delete_to_start(),
                _ => {}
            },
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

    /// Render the form widget
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let title = format!(" {} ", self.name);

        let block = Block::default()
            .title(title)
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines: Vec<Line> = Vec::new();

        // Render each field
        for (i, field) in self.fields.iter().enumerate() {
            let is_focused = i == self.focused && !self.button_focused;

            // Field label
            let type_info = format_type_info(field);
            let label_style = if is_focused {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Gray)
            };

            lines.push(Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(&field.name, label_style.add_modifier(Modifier::BOLD)),
                Span::styled(format!(" {}", type_info), Style::default().fg(Color::DarkGray)),
            ]));

            // Field input with cursor
            let content = field.content();
            let cursor_pos = field.cursor();

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

            let mut spans = vec![
                Span::styled(" ", Style::default()),
                Span::styled("> ", prompt_style),
            ];

            if is_focused {
                // Show cursor - cursor_pos is a byte position
                let (before, rest) = if cursor_pos <= content.len() {
                    (&content[..cursor_pos], &content[cursor_pos..])
                } else {
                    (content, "")
                };

                let cursor_char = rest.chars().next().unwrap_or(' ');
                let after = if !rest.is_empty() {
                    &rest[cursor_char.len_utf8()..]
                } else {
                    ""
                };

                spans.push(Span::styled(before.to_string(), input_style));
                spans.push(Span::styled(
                    cursor_char.to_string(),
                    Style::default().bg(Color::White).fg(Color::Black),
                ));
                spans.push(Span::styled(after.to_string(), input_style));
            } else {
                spans.push(Span::styled(content.to_string(), input_style));
            }

            lines.push(Line::from(spans));

            // Error line if present
            if let Some(error) = field.error_message() {
                lines.push(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(error, Style::default().fg(Color::Red)),
                ]));
            }
        }

        // Spacing before button
        lines.push(Line::default());

        // Submit button
        let can_submit = self.can_submit();
        let button_style = if self.button_focused {
            if can_submit {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Black).bg(Color::DarkGray)
            }
        } else if can_submit {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let button_text = if can_submit {
            "[ Submit ]"
        } else {
            "[ Submit ] (fix errors)"
        };

        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(button_text, button_style),
        ]));

        // Hints
        lines.push(Line::from(vec![
            Span::styled(
                " Tab",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" next  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Shift+Tab",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" prev  ", Style::default().fg(Color::DarkGray)),
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

impl Widget for InputFormWidget {
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            // Cancel
            KeyCode::Esc => {
                self.result = WidgetResult::Cancelled;
                true
            }

            // Submit
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.can_submit() {
                    self.result = WidgetResult::Saved(WidgetOutput::FormValues(self.collect_values()));
                }
                true
            }
            KeyCode::Enter if self.button_focused => {
                if self.can_submit() {
                    self.result = WidgetResult::Saved(WidgetOutput::FormValues(self.collect_values()));
                }
                true
            }

            // Navigation
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.focus_previous();
                true
            }
            KeyCode::Tab => {
                self.focus_next();
                true
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                self.focus_previous();
                true
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                self.focus_next();
                true
            }

            // Field editing (only when not on button)
            _ if !self.button_focused => {
                self.handle_field_key(key);
                true
            }

            _ => false,
        }
    }

    fn result(&self) -> WidgetResult {
        self.result.clone()
    }

    fn height(&self) -> u16 {
        // Each field: 2 lines (label + input) + optional error line
        // Plus: border (2) + button (1) + hints (1) + spacing (1)
        let field_lines: u16 = self
            .fields
            .iter()
            .map(|f| if f.error_message().is_some() { 3 } else { 2 })
            .sum();
        field_lines + 5
    }
}

/// Format type information for display
fn format_type_info(field: &InputField) -> String {
    use crate::session::InputType;

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
