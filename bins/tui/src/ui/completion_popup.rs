//! Completion popup widget
//!
//! Renders a popup showing completion suggestions above the input prompt.

use crate::app::CompletionState;
use crate::completion::Suggestion;
use crate::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Maximum number of visible suggestions
const MAX_VISIBLE: usize = 6;

/// Render the completion popup
pub fn render(
    frame: &mut Frame,
    state: &CompletionState,
    prompt_rect: Rect,
    cursor_col: usize,
    theme: &Theme,
) {
    if state.suggestions.is_empty() {
        return;
    }

    // Calculate popup dimensions and position
    let area = calculate_area(&state.suggestions, prompt_rect, cursor_col);

    // Clear the area
    frame.render_widget(Clear, area);

    // Calculate scroll offset
    let scroll_offset = calculate_scroll_offset(state.selected, state.suggestions.len());
    let visible_end = (scroll_offset + MAX_VISIBLE).min(state.suggestions.len());

    // Build lines for visible suggestions
    let lines: Vec<Line> = state.suggestions[scroll_offset..visible_end]
        .iter()
        .enumerate()
        .map(|(i, suggestion)| {
            let actual_index = scroll_offset + i;
            build_suggestion_line(suggestion, actual_index == state.selected, theme)
        })
        .collect();

    // Build title with count if scrolling
    let title = if state.suggestions.len() > MAX_VISIBLE {
        format!(" ({}/{}) ", state.selected + 1, state.suggestions.len())
    } else {
        String::new()
    };

    // Build block with theme-consistent styling
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(theme.border_type())
        .border_style(theme.text_dim_style())
        .style(theme.modal_overlay_style());

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Calculate popup area positioned above prompt
fn calculate_area(suggestions: &[Suggestion], prompt_rect: Rect, cursor_col: usize) -> Rect {
    // Calculate dimensions
    let width = calculate_width(suggestions).min(prompt_rect.width as usize) as u16;
    let height = (suggestions.len().min(MAX_VISIBLE) + 2) as u16; // +2 for borders

    // Position: bottom-left aligned to cursor
    // +1 for the prompt border
    let x = (prompt_rect.x + 1 + cursor_col as u16)
        .min(prompt_rect.x + prompt_rect.width.saturating_sub(width));
    let y = prompt_rect.y.saturating_sub(height);

    Rect::new(x, y, width, height)
}

/// Calculate width needed for suggestions
fn calculate_width(suggestions: &[Suggestion]) -> usize {
    let max_value = suggestions
        .iter()
        .map(|s| s.value.len())
        .max()
        .unwrap_or(10);
    let max_desc = suggestions
        .iter()
        .filter_map(|s| s.description.as_ref().map(|d| d.len().min(25)))
        .max()
        .unwrap_or(0);

    if max_desc > 0 {
        max_value + 3 + max_desc + 4 // " - " + borders/padding
    } else {
        max_value + 4 // borders/padding only
    }
}

/// Calculate scroll offset to keep selected item visible (centered when possible)
fn calculate_scroll_offset(selected: usize, total: usize) -> usize {
    if total <= MAX_VISIBLE {
        return 0;
    }
    if selected < MAX_VISIBLE / 2 {
        return 0;
    }
    if selected >= total - MAX_VISIBLE / 2 {
        return total - MAX_VISIBLE;
    }
    selected - MAX_VISIBLE / 2
}

/// Build a line for a suggestion
fn build_suggestion_line<'a>(suggestion: &'a Suggestion, is_selected: bool, theme: &Theme) -> Line<'a> {
    let (value_style, desc_style) = if is_selected {
        (
            theme.highlight_style(),
            theme.highlight_style().add_modifier(Modifier::DIM),
        )
    } else {
        (theme.text_style(), theme.text_dim_style())
    };

    let mut spans = vec![Span::styled(suggestion.value.as_str(), value_style)];

    if let Some(desc) = &suggestion.description {
        let truncated: String = if desc.len() > 25 {
            format!("{}...", &desc[..22])
        } else {
            desc.clone()
        };
        spans.push(Span::styled(" - ", desc_style));
        spans.push(Span::styled(truncated, desc_style));
    }

    Line::from(spans)
}
