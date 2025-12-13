//! Input definition widget for creating/editing input parameters
//!
//! Provides a form for defining input metadata: type, required, default, description.

use super::{Widget, WidgetOutput, WidgetResult};
use crate::session::{InputDef, InputType};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Available input types for selection
const INPUT_TYPES: &[InputType] = &[
    InputType::String,
    InputType::Int,
    InputType::Bool,
    InputType::Datetime,
    InputType::Timespan,
];

/// Fields in the input definition form
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Type,
    Required,
    Default,
    Description,
    Submit,
}

impl Field {
    fn next(self) -> Self {
        match self {
            Field::Type => Field::Required,
            Field::Required => Field::Default,
            Field::Default => Field::Description,
            Field::Description => Field::Submit,
            Field::Submit => Field::Type,
        }
    }

    fn prev(self) -> Self {
        match self {
            Field::Type => Field::Submit,
            Field::Required => Field::Type,
            Field::Default => Field::Required,
            Field::Description => Field::Default,
            Field::Submit => Field::Description,
        }
    }
}

/// Widget for defining input parameters
#[derive(Debug, Clone)]
pub struct InputDefWidget {
    /// Input name (provided on command line)
    pub name: String,
    /// Selected type index
    type_index: usize,
    /// Whether input is required
    required: bool,
    /// Default value (empty = no default)
    default_value: String,
    /// Default value cursor position
    default_cursor: usize,
    /// Description
    description: String,
    /// Description cursor position
    description_cursor: usize,
    /// Currently focused field
    focused: Field,
    /// Widget result
    pub result: WidgetResult,
}

impl InputDefWidget {
    /// Create a new input definition widget
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_index: 0, // Default to String
            required: true,
            default_value: String::new(),
            default_cursor: 0,
            description: String::new(),
            description_cursor: 0,
            focused: Field::Type,
            result: WidgetResult::Pending,
        }
    }

    /// Create from an existing input definition (for editing)
    pub fn from_existing(input: &InputDef) -> Self {
        let type_index = INPUT_TYPES
            .iter()
            .position(|t| *t == input.input_type)
            .unwrap_or(0);

        Self {
            name: input.name.clone(),
            type_index,
            required: input.required,
            default_value: input.default.clone().unwrap_or_default(),
            default_cursor: input.default.as_ref().map(|d| d.len()).unwrap_or(0),
            description: input.description.clone().unwrap_or_default(),
            description_cursor: input.description.as_ref().map(|d| d.len()).unwrap_or(0),
            focused: Field::Type,
            result: WidgetResult::Pending,
        }
    }

    /// Build the InputDef from current form state
    fn build_input_def(&self) -> InputDef {
        InputDef {
            name: self.name.clone(),
            input_type: INPUT_TYPES[self.type_index].clone(),
            required: self.required,
            default: if self.default_value.is_empty() {
                None
            } else {
                Some(self.default_value.clone())
            },
            description: if self.description.is_empty() {
                None
            } else {
                Some(self.description.clone())
            },
        }
    }

    /// Get the current input type
    pub fn input_type(&self) -> InputType {
        INPUT_TYPES[self.type_index].clone()
    }

    /// Get the current default value (None if empty)
    pub fn default_value(&self) -> Option<String> {
        if self.default_value.is_empty() {
            None
        } else {
            Some(self.default_value.clone())
        }
    }

    /// Check if form can be submitted
    fn can_submit(&self) -> bool {
        // Can always submit - all fields have valid defaults
        // Required + no default is valid (will prompt at runtime)
        true
    }

    /// Handle text input for the focused text field
    fn handle_text_input(&mut self, key: KeyEvent) {
        let (buffer, cursor) = match self.focused {
            Field::Default => (&mut self.default_value, &mut self.default_cursor),
            Field::Description => (&mut self.description, &mut self.description_cursor),
            _ => return,
        };

        match key.code {
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => match c {
                'a' => *cursor = 0,
                'e' => *cursor = buffer.len(),
                'k' => buffer.truncate(*cursor),
                'u' => {
                    buffer.drain(..*cursor);
                    *cursor = 0;
                }
                _ => {}
            },
            KeyCode::Char(c) => {
                buffer.insert(*cursor, c);
                *cursor += c.len_utf8();
            }
            KeyCode::Backspace if *cursor > 0 => {
                let prev_char = buffer[..*cursor].chars().last().unwrap();
                *cursor -= prev_char.len_utf8();
                buffer.remove(*cursor);
            }
            KeyCode::Delete if *cursor < buffer.len() => {
                buffer.remove(*cursor);
            }
            KeyCode::Left if *cursor > 0 => {
                let prev_char = buffer[..*cursor].chars().last().unwrap();
                *cursor -= prev_char.len_utf8();
            }
            KeyCode::Right if *cursor < buffer.len() => {
                let next_char = buffer[*cursor..].chars().next().unwrap();
                *cursor += next_char.len_utf8();
            }
            KeyCode::Home => *cursor = 0,
            KeyCode::End => *cursor = buffer.len(),
            _ => {}
        }
    }

    /// Render the form
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let title = format!(" Define Input: {} ", self.name);

        let block = Block::default()
            .title(title)
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines: Vec<Line> = Vec::new();

        // Type field (selection)
        let type_focused = self.focused == Field::Type;
        lines.push(self.render_label("Type", type_focused));
        lines.push(self.render_type_selector(type_focused));

        // Required field (toggle)
        let req_focused = self.focused == Field::Required;
        lines.push(self.render_label("Required", req_focused));
        lines.push(self.render_toggle(self.required, req_focused));

        // Default value field (text)
        let def_focused = self.focused == Field::Default;
        lines.push(self.render_label("Default", def_focused));
        lines.push(self.render_text_input(&self.default_value, self.default_cursor, def_focused));

        // Description field (text)
        let desc_focused = self.focused == Field::Description;
        lines.push(self.render_label("Description", desc_focused));
        lines.push(self.render_text_input(&self.description, self.description_cursor, desc_focused));

        // Spacing
        lines.push(Line::default());

        // Submit button
        let submit_focused = self.focused == Field::Submit;
        let button_style = if submit_focused {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };
        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled("[ Save ]", button_style),
        ]));

        // Hints
        lines.push(Line::from(vec![
            Span::styled(
                " Tab",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" next  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Enter",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" select/save  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
        ]));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn render_label(&self, text: &str, focused: bool) -> Line<'static> {
        let style = if focused {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(text.to_string(), style),
        ])
    }

    fn render_type_selector(&self, focused: bool) -> Line<'static> {
        let mut spans = vec![Span::styled("   ", Style::default())];

        for (i, input_type) in INPUT_TYPES.iter().enumerate() {
            let is_selected = i == self.type_index;
            let type_str = format!("{}", input_type);

            let style = if is_selected && focused {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Gray)
            } else if focused {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            spans.push(Span::styled(format!(" {} ", type_str), style));
            spans.push(Span::styled(" ", Style::default()));
        }

        Line::from(spans)
    }

    fn render_toggle(&self, value: bool, focused: bool) -> Line<'static> {
        let yes_style = if value && focused {
            Style::default().fg(Color::Black).bg(Color::Green)
        } else if value {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else if focused {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let no_style = if !value && focused {
            Style::default().fg(Color::Black).bg(Color::Red)
        } else if !value {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else if focused {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(" Yes ", yes_style),
            Span::styled(" ", Style::default()),
            Span::styled(" No ", no_style),
            if focused {
                Span::styled("  (Space/Enter to toggle)", Style::default().fg(Color::DarkGray))
            } else {
                Span::default()
            },
        ])
    }

    fn render_text_input(&self, content: &str, cursor: usize, focused: bool) -> Line<'static> {
        let prompt_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let input_style = if focused {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };

        let mut spans = vec![
            Span::styled(" ", Style::default()),
            Span::styled("> ", prompt_style),
        ];

        if focused {
            // Show cursor
            let (before, rest) = if cursor <= content.len() {
                (&content[..cursor], &content[cursor..])
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
        } else if content.is_empty() {
            spans.push(Span::styled("(empty)", Style::default().fg(Color::DarkGray)));
        } else {
            spans.push(Span::styled(content.to_string(), input_style));
        }

        Line::from(spans)
    }
}

impl Widget for InputDefWidget {
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            // Cancel
            KeyCode::Esc => {
                self.result = WidgetResult::Cancelled;
                true
            }

            // Navigation
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.focused = self.focused.prev();
                true
            }
            KeyCode::Tab => {
                self.focused = self.focused.next();
                true
            }
            KeyCode::Up => {
                self.focused = self.focused.prev();
                true
            }
            KeyCode::Down => {
                self.focused = self.focused.next();
                true
            }

            // Submit with Ctrl+Enter from anywhere
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.can_submit() {
                    self.result = WidgetResult::Saved(WidgetOutput::InputDef(self.build_input_def()));
                }
                true
            }

            // Field-specific handling
            _ => match self.focused {
                Field::Type => {
                    match key.code {
                        KeyCode::Left => {
                            if self.type_index > 0 {
                                self.type_index -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if self.type_index < INPUT_TYPES.len() - 1 {
                                self.type_index += 1;
                            }
                        }
                        KeyCode::Enter => {
                            self.focused = self.focused.next();
                        }
                        _ => {}
                    }
                    true
                }
                Field::Required => {
                    match key.code {
                        KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') => {
                            self.required = !self.required;
                        }
                        KeyCode::Enter => {
                            self.focused = self.focused.next();
                        }
                        _ => {}
                    }
                    true
                }
                Field::Default | Field::Description => {
                    match key.code {
                        KeyCode::Enter => {
                            self.focused = self.focused.next();
                        }
                        _ => {
                            self.handle_text_input(key);
                        }
                    }
                    true
                }
                Field::Submit => {
                    if key.code == KeyCode::Enter && self.can_submit() {
                        self.result = WidgetResult::Saved(WidgetOutput::InputDef(self.build_input_def()));
                    }
                    true
                }
            },
        }
    }

    fn result(&self) -> WidgetResult {
        self.result.clone()
    }

    fn height(&self) -> u16 {
        // Labels + fields + spacing + button + hints + border
        // Type: 2, Required: 2, Default: 2, Description: 2, spacing: 1, button: 1, hints: 1, border: 2
        13
    }
}
