use crate::tui::view::kql_highlight;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Widget},
};
use tui_textarea::TextArea;

/// Apply selection highlighting to a vector of spans
fn apply_selection_to_spans(
    spans: Vec<Span<'_>>,
    current_row: usize,
    start_row: usize,
    start_col: usize,
    end_row: usize,
    end_col: usize,
) -> Vec<Span<'_>> {
    // Determine the selection range for this line
    let (sel_start, sel_end) = if current_row == start_row && current_row == end_row {
        // Selection is entirely on this line
        (start_col, end_col)
    } else if current_row == start_row {
        // This is the first line of a multi-line selection
        (start_col, usize::MAX)
    } else if current_row == end_row {
        // This is the last line of a multi-line selection
        (0, end_col)
    } else {
        // This is a middle line - entire line is selected
        (0, usize::MAX)
    };

    // Create a selection style (inverted colors)
    let selection_style = Style::default().bg(Color::Blue).fg(Color::White);

    // Apply selection to spans
    let mut result = Vec::new();
    let mut char_pos = 0;

    for span in spans {
        let span_len = span.content.len();
        let span_end = char_pos + span_len;

        if span_end <= sel_start || char_pos >= sel_end {
            // Span is entirely outside selection
            result.push(span);
        } else if char_pos >= sel_start && span_end <= sel_end {
            // Span is entirely inside selection
            result.push(Span::styled(span.content.clone(), selection_style));
        } else {
            // Span is partially selected - need to split it
            let content_str = span.content.to_string();
            let mut chars_vec: Vec<char> = content_str.chars().collect();

            let mut current_pos = char_pos;
            let mut current_str = String::new();
            let mut in_selection = current_pos >= sel_start && current_pos < sel_end;

            for ch in chars_vec.drain(..) {
                let next_in_selection = current_pos >= sel_start && current_pos < sel_end;

                if next_in_selection != in_selection {
                    // Transition point - flush current string
                    if !current_str.is_empty() {
                        let style = if in_selection {
                            selection_style
                        } else {
                            span.style
                        };
                        result.push(Span::styled(current_str.clone(), style));
                        current_str.clear();
                    }
                    in_selection = next_in_selection;
                }

                current_str.push(ch);
                current_pos += 1;
            }

            // Flush remaining string
            if !current_str.is_empty() {
                let style = if in_selection {
                    selection_style
                } else {
                    span.style
                };
                result.push(Span::styled(current_str, style));
            }
        }

        char_pos = span_end;
    }

    result
}

/// A wrapper around TextArea that adds syntax highlighting
pub struct SyntaxTextArea<'a> {
    textarea: &'a TextArea<'a>,
    block: Option<Block<'a>>,
}

impl<'a> SyntaxTextArea<'a> {
    pub fn new(textarea: &'a TextArea<'a>) -> Self {
        Self {
            textarea,
            block: None,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl<'a> Widget for SyntaxTextArea<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // First, get the inner area if there's a block
        let inner = if let Some(block) = self.block {
            let inner_area = block.inner(area);
            block.render(area, buf);
            inner_area
        } else {
            area
        };

        // Get the textarea's lines
        let lines = self.textarea.lines();

        // Get cursor position for highlighting
        let (cursor_row, cursor_col) = self.textarea.cursor();

        // Get selection range if any
        let selection = self.textarea.selection_range();

        // Get the viewport offset (scroll position)
        let viewport_height = inner.height as usize;
        let max_start = lines.len().saturating_sub(viewport_height);
        let start_row = cursor_row
            .saturating_sub(viewport_height / 2)
            .min(max_start);

        // Calculate line number width
        let line_count = lines.len();
        let line_num_width = line_count.to_string().len().max(2) + 1; // +1 for space

        // Render each visible line with syntax highlighting
        let mut y = inner.y;
        for (idx, line_text) in lines
            .iter()
            .enumerate()
            .skip(start_row)
            .take(viewport_height)
        {
            if y >= inner.y + inner.height {
                break;
            }

            let line_num = format!("{:>width$} ", idx + 1, width = line_num_width - 1);

            // Create line number span
            let mut spans = vec![Span::styled(line_num, Style::default().fg(Color::DarkGray))];

            // Add syntax-highlighted content with selection overlay
            let highlighted_spans =
                if let Some(((start_row, start_col), (end_row, end_col))) = selection {
                    // Check if this line is within the selection
                    let is_selected_line = idx >= start_row && idx <= end_row;

                    if is_selected_line {
                        // Apply selection highlighting
                        apply_selection_to_spans(
                            kql_highlight::highlight_line(line_text),
                            idx,
                            start_row,
                            start_col,
                            end_row,
                            end_col,
                        )
                    } else {
                        kql_highlight::highlight_line(line_text)
                    }
                } else {
                    kql_highlight::highlight_line(line_text)
                };

            spans.extend(highlighted_spans);

            // Render the line
            let line = Line::from(spans);
            let line_area = Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: 1,
            };

            line.render(line_area, buf);

            // Render cursor if on this line
            if idx == cursor_row {
                let cursor_x = inner.x + (line_num_width as u16) + (cursor_col as u16);
                if cursor_x < inner.x + inner.width {
                    // Render cursor as inverse video
                    if let Some(cell) = buf.cell_mut((cursor_x, y)) {
                        let current_fg = cell.fg;
                        let current_bg = cell.bg;
                        cell.set_fg(current_bg);
                        cell.set_bg(current_fg);
                        // If both are the same (or default), use a visible color
                        if current_fg == current_bg {
                            cell.set_bg(Color::White);
                            cell.set_fg(Color::Black);
                        }
                    }
                }
            }

            y += 1;
        }
    }
}
