//! Completion popup widget for the editor
//!
//! Provides code completion with:
//! - KQL operators and functions via FFI
//! - Session-aware `{{reference}}` completion
//! - Schema-aware column completion

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

/// A completion item to display
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// Display label
    pub label: String,
    /// Kind of completion (for icon/color)
    pub kind: CompletionItemKind,
    /// Optional detail text
    pub detail: Option<String>,
    /// Text to insert (if different from label)
    pub insert_text: Option<String>,
    /// Character position where replacement should start
    pub edit_start: usize,
}

/// Kind of completion item
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemKind {
    /// A KQL operator (where, project, etc.)
    Operator,
    /// A function
    Function,
    /// A table name
    Table,
    /// A column name
    Column,
    /// An input reference
    Input,
    /// A step reference
    Step,
    /// A keyword
    Keyword,
    /// Other/unknown
    Other,
}

impl CompletionItemKind {
    /// Get the icon for this kind
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Operator => "⊕",
            Self::Function => "ƒ",
            Self::Table => "⊞",
            Self::Column => "↳",
            Self::Input => "→",
            Self::Step => "◈",
            Self::Keyword => "▪",
            Self::Other => "•",
        }
    }

    /// Get the color for this kind
    pub fn color(&self) -> Color {
        match self {
            Self::Operator => Color::Cyan,
            Self::Function => Color::Blue,
            Self::Table => Color::Yellow,
            Self::Column => Color::Green,
            Self::Input => Color::Magenta,
            Self::Step => Color::Cyan,
            Self::Keyword => Color::Magenta,
            Self::Other => Color::White,
        }
    }
}

/// Trait for providing completion items
pub trait CompletionSource: Send + Sync {
    /// Get completion items at the given cursor position
    fn get_completions(&self, content: &str, cursor_offset: usize) -> Vec<CompletionItem>;
}

/// No-op completion source (for testing)
pub struct NoOpCompletionSource;

impl CompletionSource for NoOpCompletionSource {
    fn get_completions(&self, _content: &str, _cursor_offset: usize) -> Vec<CompletionItem> {
        Vec::new()
    }
}

/// Completion popup widget
pub struct CompletionPopup {
    /// Available completion items
    items: Vec<CompletionItem>,
    /// List state for selection
    state: ListState,
    /// Cursor row where popup should appear
    cursor_row: usize,
    /// Cursor column where popup should appear
    cursor_col: usize,
}

impl CompletionPopup {
    /// Create a new completion popup
    pub fn new(items: Vec<CompletionItem>, cursor_row: usize, cursor_col: usize) -> Self {
        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(0));
        }

        Self {
            items,
            state,
            cursor_row,
            cursor_col,
        }
    }

    /// Move the selection by delta (negative = up, positive = down)
    pub fn move_selection(&mut self, delta: i32) {
        if self.items.is_empty() {
            return;
        }

        let current = self.state.selected().unwrap_or(0) as i32;
        let new_selection = (current + delta).rem_euclid(self.items.len() as i32) as usize;
        self.state.select(Some(new_selection));
    }

    /// Get the currently selected item
    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.state.selected().and_then(|i| self.items.get(i))
    }

    /// Check if the popup is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the number of items
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Render the completion popup
    pub fn render(&mut self, frame: &mut ratatui::Frame, editor_area: Rect) {
        if self.items.is_empty() {
            return;
        }

        // Calculate popup dimensions
        let max_width = self
            .items
            .iter()
            .map(|item| {
                let detail_len = item.detail.as_ref().map(|d| d.len() + 2).unwrap_or(0);
                item.label.len() + 4 + detail_len // icon + padding + label + detail
            })
            .max()
            .unwrap_or(20)
            .min(60) as u16;

        let height = self.items.len().min(10) as u16;

        // Position popup below cursor, or above if not enough space
        let popup_y = if self.cursor_row + 2 + height as usize <= editor_area.height as usize {
            // Below cursor
            editor_area.y + self.cursor_row as u16 + 2 // +2 for border and line
        } else {
            // Above cursor
            editor_area
                .y
                .saturating_add(self.cursor_row as u16)
                .saturating_sub(height + 1)
        };

        // Position popup at cursor column (with some padding for line numbers)
        let popup_x = editor_area.x + self.cursor_col as u16 + 5; // +5 for line number margin

        // Clamp to editor area
        let popup_x = popup_x.min(editor_area.x + editor_area.width - max_width - 2);
        let popup_y = popup_y.max(editor_area.y);

        let popup_area = Rect::new(popup_x, popup_y, max_width + 2, height + 2);

        // Build list items
        let list_items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let icon = Span::styled(
                    format!("{} ", item.kind.icon()),
                    Style::default().fg(item.kind.color()),
                );
                let label = Span::raw(&item.label);
                let detail = item.detail.as_ref().map(|d| {
                    Span::styled(
                        format!("  {}", d),
                        Style::default().fg(Color::DarkGray),
                    )
                });

                let mut spans = vec![icon, label];
                if let Some(d) = detail {
                    spans.push(d);
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        // Create the list widget
        let list = List::new(list_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title("Completions"),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▸ ");

        // Clear the area first, then render
        frame.render_widget(Clear, popup_area);
        frame.render_stateful_widget(list, popup_area, &mut self.state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_popup_selection() {
        let items = vec![
            CompletionItem {
                label: "where".to_string(),
                kind: CompletionItemKind::Operator,
                detail: Some("Filter rows".to_string()),
                insert_text: None,
                edit_start: 0,
            },
            CompletionItem {
                label: "project".to_string(),
                kind: CompletionItemKind::Operator,
                detail: Some("Select columns".to_string()),
                insert_text: None,
                edit_start: 0,
            },
        ];

        let mut popup = CompletionPopup::new(items, 0, 0);

        // Initially selected first item
        assert_eq!(popup.selected_item().unwrap().label, "where");

        // Move down
        popup.move_selection(1);
        assert_eq!(popup.selected_item().unwrap().label, "project");

        // Move down wraps to first
        popup.move_selection(1);
        assert_eq!(popup.selected_item().unwrap().label, "where");

        // Move up wraps to last
        popup.move_selection(-1);
        assert_eq!(popup.selected_item().unwrap().label, "project");
    }

    #[test]
    fn test_completion_item_kind_icons() {
        assert_eq!(CompletionItemKind::Operator.icon(), "⊕");
        assert_eq!(CompletionItemKind::Function.icon(), "ƒ");
        assert_eq!(CompletionItemKind::Table.icon(), "⊞");
        assert_eq!(CompletionItemKind::Column.icon(), "↳");
        assert_eq!(CompletionItemKind::Input.icon(), "→");
        assert_eq!(CompletionItemKind::Step.icon(), "◈");
    }

    #[test]
    fn test_noop_completion_source() {
        let source = NoOpCompletionSource;
        let items = source.get_completions("any content", 0);
        assert!(items.is_empty());
    }
}
