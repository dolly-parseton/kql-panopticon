//! Completion traits and popup widget for the editor
//!
//! Provides a trait-based completion system supporting:
//! - KQL operators and functions via FFI
//! - Session-aware `{{reference}}` completion
//! - Schema-aware column completion
//! - YAML type/required completions
//!
//! ## Architecture
//!
//! The completion system uses trait-based dynamic dispatch to support
//! multiple completion item types without conversion overhead:
//!
//! ```text
//! CompletionSource::get_completions() -> Vec<Box<dyn CompletionItem>>
//!                                              │
//!                    ┌─────────────────────────┼─────────────────────────┐
//!                    │                         │                         │
//!                    ▼                         ▼                         ▼
//!           KqlCompletionItem      ReferenceCompletionItem     YamlCompletionItem
//!           (wraps FFI type)       (inputs/steps)              (type/required)
//! ```

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

// ============================================================================
// Completion Traits
// ============================================================================

/// Trait for completion item display (rendering)
///
/// Provides the visual representation of a completion item in the popup.
pub trait CompletionDisplay {
    /// The label shown in the completion popup
    fn label(&self) -> &str;

    /// Icon character representing the completion kind
    fn icon(&self) -> &str;

    /// Color for the icon
    fn color(&self) -> Color;

    /// Optional detail text shown after the label
    fn detail(&self) -> Option<&str>;
}

/// Trait for completion item insertion
///
/// Provides the text insertion behavior when a completion is accepted.
pub trait CompletionInsert {
    /// The text to insert (may differ from label)
    fn insert_text(&self) -> &str;

    /// Character position where replacement should start
    fn edit_start(&self) -> usize;
}

/// Combined trait for full completion items
///
/// Completion items must implement both display and insertion traits.
pub trait CompletionItem: CompletionDisplay + CompletionInsert + Send + Sync {
    /// Convert to a trait object for storage
    fn boxed(self) -> Box<dyn CompletionItem>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

/// Blanket implementation for any type implementing both traits
impl<T: CompletionDisplay + CompletionInsert + Send + Sync> CompletionItem for T {}

// ============================================================================
// Completion Kinds (for icon/color lookup)
// ============================================================================

/// Kind of completion item
///
/// Used by concrete completion item types to determine icon and color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// A KQL operator (where, project, etc.)
    Operator,
    /// A function
    Function,
    /// An aggregate function
    AggregateFunction,
    /// A table name
    Table,
    /// A column name
    Column,
    /// A variable
    Variable,
    /// An input reference
    Input,
    /// A step reference
    Step,
    /// A keyword
    Keyword,
    /// A parameter
    Parameter,
    /// A database
    Database,
    /// A type (string, int, bool, etc.)
    Type,
    /// Other/unknown
    Other,
}

impl CompletionKind {
    /// Get the icon for this kind
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Operator => "⊕",
            Self::Function | Self::AggregateFunction => "ƒ",
            Self::Table => "⊞",
            Self::Column => "↳",
            Self::Variable => "χ",
            Self::Input => "→",
            Self::Step => "◈",
            Self::Keyword => "▪",
            Self::Parameter => "◆",
            Self::Database => "⊟",
            Self::Type => "τ",
            Self::Other => "•",
        }
    }

    /// Get the color for this kind
    pub fn color(&self) -> Color {
        match self {
            Self::Operator => Color::Cyan,
            Self::Function | Self::AggregateFunction => Color::Blue,
            Self::Table => Color::Yellow,
            Self::Column => Color::Green,
            Self::Variable => Color::LightCyan,
            Self::Input => Color::Magenta,
            Self::Step => Color::Cyan,
            Self::Keyword => Color::Magenta,
            Self::Parameter => Color::LightYellow,
            Self::Database => Color::Yellow,
            Self::Type => Color::LightBlue,
            Self::Other => Color::White,
        }
    }
}

// ============================================================================
// Completion Source Trait
// ============================================================================

/// Trait for providing completion items
///
/// Implementations should return completion items appropriate for the
/// given cursor position in the content.
pub trait CompletionSource: Send + Sync {
    /// Get completion items at the given cursor position
    ///
    /// # Arguments
    /// * `content` - The full text content
    /// * `cursor_offset` - Byte offset of the cursor position
    ///
    /// # Returns
    /// A vector of boxed completion items
    fn get_completions(&self, content: &str, cursor_offset: usize) -> Vec<Box<dyn CompletionItem>>;
}

/// No-op completion source (for testing)
pub struct NoOpCompletionSource;

impl CompletionSource for NoOpCompletionSource {
    fn get_completions(&self, _content: &str, _cursor_offset: usize) -> Vec<Box<dyn CompletionItem>> {
        Vec::new()
    }
}

// ============================================================================
// Completion Popup Widget
// ============================================================================

/// Completion popup widget
///
/// Displays a list of completion items and handles selection.
pub struct CompletionPopup {
    /// Available completion items
    items: Vec<Box<dyn CompletionItem>>,
    /// List state for selection
    state: ListState,
    /// Cursor row where popup should appear
    cursor_row: usize,
    /// Cursor column where popup should appear
    cursor_col: usize,
}

impl CompletionPopup {
    /// Create a new completion popup
    pub fn new(items: Vec<Box<dyn CompletionItem>>, cursor_row: usize, cursor_col: usize) -> Self {
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
    pub fn selected_item(&self) -> Option<&dyn CompletionItem> {
        self.state
            .selected()
            .and_then(|i| self.items.get(i))
            .map(|b| b.as_ref())
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
                let detail_len = item.detail().map(|d| d.len() + 2).unwrap_or(0);
                item.label().len() + 4 + detail_len // icon + padding + label + detail
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
                    format!("{} ", item.icon()),
                    Style::default().fg(item.color()),
                );
                let label = Span::raw(item.label());
                let detail = item.detail().map(|d| {
                    Span::styled(format!("  {}", d), Style::default().fg(Color::DarkGray))
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

// ============================================================================
// Simple Completion Item (for basic/testing use)
// ============================================================================

/// A simple concrete completion item for basic use cases
#[derive(Debug, Clone)]
pub struct SimpleCompletionItem {
    /// Display label
    pub label: String,
    /// Kind of completion
    pub kind: CompletionKind,
    /// Optional detail text
    pub detail: Option<String>,
    /// Text to insert (if different from label)
    pub insert_text: Option<String>,
    /// Character position where replacement should start
    pub edit_start: usize,
}

impl CompletionDisplay for SimpleCompletionItem {
    fn label(&self) -> &str {
        &self.label
    }

    fn icon(&self) -> &str {
        self.kind.icon()
    }

    fn color(&self) -> Color {
        self.kind.color()
    }

    fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

impl CompletionInsert for SimpleCompletionItem {
    fn insert_text(&self) -> &str {
        self.insert_text.as_deref().unwrap_or(&self.label)
    }

    fn edit_start(&self) -> usize {
        self.edit_start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_popup_selection() {
        let items: Vec<Box<dyn CompletionItem>> = vec![
            Box::new(SimpleCompletionItem {
                label: "where".to_string(),
                kind: CompletionKind::Operator,
                detail: Some("Filter rows".to_string()),
                insert_text: None,
                edit_start: 0,
            }),
            Box::new(SimpleCompletionItem {
                label: "project".to_string(),
                kind: CompletionKind::Operator,
                detail: Some("Select columns".to_string()),
                insert_text: None,
                edit_start: 0,
            }),
        ];

        let mut popup = CompletionPopup::new(items, 0, 0);

        // Initially selected first item
        assert_eq!(popup.selected_item().unwrap().label(), "where");

        // Move down
        popup.move_selection(1);
        assert_eq!(popup.selected_item().unwrap().label(), "project");

        // Move down wraps to first
        popup.move_selection(1);
        assert_eq!(popup.selected_item().unwrap().label(), "where");

        // Move up wraps to last
        popup.move_selection(-1);
        assert_eq!(popup.selected_item().unwrap().label(), "project");
    }

    #[test]
    fn test_completion_kind_icons() {
        assert_eq!(CompletionKind::Operator.icon(), "⊕");
        assert_eq!(CompletionKind::Function.icon(), "ƒ");
        assert_eq!(CompletionKind::Table.icon(), "⊞");
        assert_eq!(CompletionKind::Column.icon(), "↳");
        assert_eq!(CompletionKind::Input.icon(), "→");
        assert_eq!(CompletionKind::Step.icon(), "◈");
    }

    #[test]
    fn test_noop_completion_source() {
        let source = NoOpCompletionSource;
        let items = source.get_completions("any content", 0);
        assert!(items.is_empty());
    }

    #[test]
    fn test_simple_completion_item_insert_text() {
        let item = SimpleCompletionItem {
            label: "func()".to_string(),
            kind: CompletionKind::Function,
            detail: None,
            insert_text: Some("func($0)".to_string()),
            edit_start: 0,
        };

        // When insert_text is Some, use it
        assert_eq!(item.insert_text(), "func($0)");

        let item2 = SimpleCompletionItem {
            label: "where".to_string(),
            kind: CompletionKind::Keyword,
            detail: None,
            insert_text: None,
            edit_start: 0,
        };

        // When insert_text is None, fall back to label
        assert_eq!(item2.insert_text(), "where");
    }
}
