//! KQL editor mode
//!
//! Provides KQL-specific configuration for the editor:
//! - Syntax highlighting via FFI
//! - Code completion for operators, functions, columns
//! - Session-aware `{{reference}}` completion

use super::completion::{
    CompletionDisplay, CompletionInsert, CompletionItem, CompletionKind, CompletionSource,
};
use super::highlight::KqlHighlighter;
use super::widget::{EditorConfig, EditorResult, TuiEditor};
use crate::session::PackSession;
use kql_panopticon_core::validation::{KqlValidator, Schema};
use ratatui::style::Color;
use std::io;
use std::sync::Arc;

// ============================================================================
// KQL Completion Item (wraps FFI type directly)
// ============================================================================

/// Completion item wrapping the FFI completion type
///
/// This allows direct use of FFI completions without conversion,
/// preserving all fields including `sort_order`.
pub struct KqlCompletionItem {
    /// The wrapped FFI completion item
    inner: kql_panopticon_core::validation::CompletionItem,
}

impl KqlCompletionItem {
    /// Create from an FFI completion item
    pub fn new(inner: kql_panopticon_core::validation::CompletionItem) -> Self {
        Self { inner }
    }

    /// Get the sort order (preserved from FFI)
    #[allow(dead_code)]
    pub fn sort_order(&self) -> i32 {
        self.inner.sort_order
    }

    /// Map FFI CompletionKind to our CompletionKind
    fn map_kind(&self) -> CompletionKind {
        use kql_panopticon_core::validation::CompletionKind as FfiKind;

        match self.inner.kind {
            FfiKind::Keyword => CompletionKind::Keyword,
            FfiKind::Function => CompletionKind::Function,
            FfiKind::AggregateFunction => CompletionKind::AggregateFunction,
            FfiKind::Table => CompletionKind::Table,
            FfiKind::Column => CompletionKind::Column,
            FfiKind::Variable => CompletionKind::Variable,
            FfiKind::Operator => CompletionKind::Operator,
            FfiKind::Parameter => CompletionKind::Parameter,
            FfiKind::Database => CompletionKind::Database,
            FfiKind::Type => CompletionKind::Type,
            _ => CompletionKind::Other,
        }
    }
}

impl CompletionDisplay for KqlCompletionItem {
    fn label(&self) -> &str {
        &self.inner.label
    }

    fn icon(&self) -> &str {
        self.map_kind().icon()
    }

    fn color(&self) -> Color {
        self.map_kind().color()
    }

    fn detail(&self) -> Option<&str> {
        self.inner.detail.as_deref()
    }
}

impl CompletionInsert for KqlCompletionItem {
    fn insert_text(&self) -> &str {
        self.inner
            .insert_text
            .as_deref()
            .filter(|s| !s.is_empty()) // Filter out empty strings
            .unwrap_or(&self.inner.label)
    }

    fn edit_start(&self) -> usize {
        self.inner.edit_start
    }
}

// ============================================================================
// Reference Completion Item (for {{inputs.x}} and steps)
// ============================================================================

/// Completion item for pack references (inputs and steps)
///
/// These completions are triggered inside `{{...}}` blocks and provide
/// access to pack inputs and step results.
pub struct ReferenceCompletionItem {
    /// Display label (e.g., "inputs.ip" or "events.first")
    label: String,
    /// Kind of reference
    kind: CompletionKind,
    /// Detail text (type or description)
    detail: Option<String>,
    /// Character position where replacement should start (after `{{`)
    edit_start: usize,
}

impl ReferenceCompletionItem {
    /// Create an input reference completion
    pub fn input(name: &str, type_name: &str, edit_start: usize) -> Self {
        Self {
            label: format!("inputs.{}", name),
            kind: CompletionKind::Input,
            detail: Some(type_name.to_string()),
            edit_start,
        }
    }

    /// Create a step reference completion
    pub fn step(name: &str, description: Option<&str>, edit_start: usize) -> Self {
        Self {
            label: name.to_string(),
            kind: CompletionKind::Step,
            detail: description.map(|s| s.to_string()),
            edit_start,
        }
    }

    /// Create a step accessor completion (e.g., "events.first")
    pub fn step_accessor(step_name: &str, accessor: &str, detail: &str, edit_start: usize) -> Self {
        Self {
            label: format!("{}.{}", step_name, accessor),
            kind: CompletionKind::Step,
            detail: Some(detail.to_string()),
            edit_start,
        }
    }
}

impl CompletionDisplay for ReferenceCompletionItem {
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

impl CompletionInsert for ReferenceCompletionItem {
    fn insert_text(&self) -> &str {
        &self.label
    }

    fn edit_start(&self) -> usize {
        self.edit_start
    }
}

// ============================================================================
// KQL Editor Mode
// ============================================================================

/// KQL editor mode configuration
pub struct KqlEditorMode {
    /// The highlighter
    highlighter: KqlHighlighter,
    /// The completion source
    completion_source: KqlCompletionSource,
}

impl KqlEditorMode {
    /// Create a new KQL editor mode
    pub fn new() -> Self {
        Self {
            highlighter: KqlHighlighter::new(),
            completion_source: KqlCompletionSource::new(None, None),
        }
    }

    /// Create with session context for `{{reference}}` completion
    pub fn with_session(session: Arc<PackSession>) -> Self {
        Self {
            highlighter: KqlHighlighter::new(),
            completion_source: KqlCompletionSource::new(Some(session), None),
        }
    }

    /// Create with schema context for column completion
    pub fn with_schema(session: Arc<PackSession>, schema: Schema) -> Self {
        Self {
            highlighter: KqlHighlighter::new(),
            completion_source: KqlCompletionSource::new(Some(session), Some(schema)),
        }
    }

    /// Run the editor with the given configuration
    pub fn run(self, config: EditorConfig) -> io::Result<EditorResult> {
        let editor = TuiEditor::new(config, self.highlighter, self.completion_source);
        editor.run()
    }
}

impl Default for KqlEditorMode {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// KQL Completion Source
// ============================================================================

/// KQL completion source
pub struct KqlCompletionSource {
    /// Validator for getting completions
    validator: Option<KqlValidator>,
    /// Session for reference completion
    session: Option<Arc<PackSession>>,
    /// Schema for column completion
    schema: Option<Schema>,
}

impl KqlCompletionSource {
    /// Create a new KQL completion source
    pub fn new(session: Option<Arc<PackSession>>, schema: Option<Schema>) -> Self {
        let validator = KqlValidator::new().ok();
        Self {
            validator,
            session,
            schema,
        }
    }

    /// Get reference completions (inputs and steps)
    ///
    /// # Arguments
    /// * `prefix` - The text typed after `{{`
    /// * `edit_start` - The byte position after `{{` where replacement starts
    fn get_reference_completions(
        &self,
        prefix: &str,
        edit_start: usize,
    ) -> Vec<Box<dyn CompletionItem>> {
        let Some(session) = &self.session else {
            return Vec::new();
        };

        let mut items: Vec<Box<dyn CompletionItem>> = Vec::new();

        // Check if completing inputs
        if prefix.is_empty() || prefix.starts_with("inputs.") {
            let input_prefix = prefix.strip_prefix("inputs.").unwrap_or("");

            for (name, input) in &session.inputs {
                if input_prefix.is_empty() || name.starts_with(input_prefix) {
                    items.push(Box::new(ReferenceCompletionItem::input(
                        name,
                        &input.input_type.to_string(),
                        edit_start,
                    )));
                }
            }
        }

        // Check if completing steps
        if prefix.is_empty() || !prefix.starts_with("inputs.") {
            for (name, step) in &session.steps {
                if prefix.is_empty() || name.starts_with(prefix) {
                    // Step reference (entire result)
                    items.push(Box::new(ReferenceCompletionItem::step(
                        name,
                        step.description.as_deref(),
                        edit_start,
                    )));

                    // Step.first accessor (common pattern)
                    items.push(Box::new(ReferenceCompletionItem::step_accessor(
                        name,
                        "first",
                        "First row accessor",
                        edit_start,
                    )));
                }
            }
        }

        items
    }
}

impl CompletionSource for KqlCompletionSource {
    fn get_completions(&self, content: &str, cursor_offset: usize) -> Vec<Box<dyn CompletionItem>> {
        // Check if we're inside a `{{` reference
        if cursor_offset >= 2 {
            let before_cursor = &content[..cursor_offset];

            // Find the last `{{`
            if let Some(ref_start) = before_cursor.rfind("{{") {
                // Check if there's no closing `}}` between the start and cursor
                let between = &before_cursor[ref_start + 2..];
                if !between.contains("}}") {
                    // We're inside a reference - get reference completions
                    // edit_start is the position right after `{{`
                    let edit_start = ref_start + 2;
                    return self.get_reference_completions(between, edit_start);
                }
            }
        }

        // Get KQL completions from FFI
        let mut items: Vec<Box<dyn CompletionItem>> = Vec::new();

        if let Some(validator) = &self.validator {
            if validator.supports_completion() {
                let schema_ref = self.schema.as_ref();
                match validator.get_completions(content, cursor_offset, schema_ref) {
                    Ok(result) => {
                        // Filter by prefix and wrap in KqlCompletionItem
                        for ffi_item in result.items {
                            let edit_start = ffi_item.edit_start;

                            // Get the prefix the user typed (from edit_start to cursor)
                            let include = if edit_start <= cursor_offset
                                && edit_start <= content.len()
                            {
                                let prefix =
                                    &content[edit_start..cursor_offset.min(content.len())];
                                // Case-insensitive prefix match
                                ffi_item.label.to_lowercase().starts_with(&prefix.to_lowercase())
                            } else {
                                // If edit_start is beyond cursor, include the item
                                true
                            };

                            if include {
                                items.push(Box::new(KqlCompletionItem::new(ffi_item)));
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("Completion failed: {}", e);
                    }
                }
            }
        }

        items
    }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Convenience function to run a KQL editor
pub fn run_kql_editor(name: &str, initial_content: Option<&str>) -> io::Result<EditorResult> {
    let config = match initial_content {
        Some(content) => EditorConfig::kql(name).with_content(content),
        None => EditorConfig::kql(name),
    };

    KqlEditorMode::new().run(config)
}

/// Run a KQL editor with session context
pub fn run_kql_editor_with_session(
    name: &str,
    initial_content: Option<&str>,
    session: Arc<PackSession>,
    schema: Option<Schema>,
) -> io::Result<EditorResult> {
    let config = match initial_content {
        Some(content) => EditorConfig::kql(name).with_content(content),
        None => EditorConfig::kql(name),
    };

    let mode = match schema {
        Some(s) => KqlEditorMode::with_schema(session, s),
        None => KqlEditorMode::with_session(session),
    };

    mode.run(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{InputDef, InputType, StepDef};

    #[test]
    fn test_reference_completions() {
        let mut session = PackSession::new();
        session.add_input(InputDef {
            name: "ip".to_string(),
            input_type: InputType::String,
            description: Some("IP address".to_string()),
            required: true,
            default: None,
        });
        session.add_step(StepDef::new("events", "SecurityEvent | take 10"));

        let source = KqlCompletionSource::new(Some(Arc::new(session)), None);

        // Get all reference completions (edit_start = 2 simulates after `{{`)
        let items = source.get_reference_completions("", 2);
        assert!(!items.is_empty());

        // Should have input completion
        let has_input = items.iter().any(|i| i.label() == "inputs.ip");
        assert!(has_input);

        // Should have step completion
        let has_step = items.iter().any(|i| i.label() == "events");
        assert!(has_step);
    }

    #[test]
    fn test_reference_completion_filtering() {
        let mut session = PackSession::new();
        session.add_input(InputDef {
            name: "ip".to_string(),
            input_type: InputType::String,
            description: None,
            required: true,
            default: None,
        });
        session.add_input(InputDef {
            name: "account".to_string(),
            input_type: InputType::String,
            description: None,
            required: true,
            default: None,
        });

        let source = KqlCompletionSource::new(Some(Arc::new(session)), None);

        // Filter by prefix
        let items = source.get_reference_completions("inputs.i", 2);
        assert!(items.iter().any(|i| i.label() == "inputs.ip"));
        assert!(!items.iter().any(|i| i.label() == "inputs.account"));
    }

    #[test]
    fn test_reference_completion_edit_start() {
        let mut session = PackSession::new();
        session.add_input(InputDef {
            name: "ip".to_string(),
            input_type: InputType::String,
            description: None,
            required: true,
            default: None,
        });

        let source = KqlCompletionSource::new(Some(Arc::new(session)), None);

        // Simulate cursor at position 10 with `{{` at position 5
        let edit_start = 7; // Position after `{{`
        let items = source.get_reference_completions("", edit_start);

        // All items should have the correct edit_start
        for item in &items {
            assert_eq!(item.edit_start(), edit_start);
        }
    }

    #[test]
    fn test_kql_completion_item_preserves_sort_order() {
        use kql_panopticon_core::validation::CompletionKind as FfiKind;

        let ffi_item = kql_panopticon_core::validation::CompletionItem {
            label: "where".to_string(),
            kind: FfiKind::Keyword,
            detail: Some("Filter rows".to_string()),
            insert_text: None,
            sort_order: 42,
            edit_start: 5,
        };

        let kql_item = KqlCompletionItem::new(ffi_item);

        assert_eq!(kql_item.label(), "where");
        assert_eq!(kql_item.sort_order(), 42);
        assert_eq!(kql_item.edit_start(), 5);
        assert_eq!(kql_item.detail(), Some("Filter rows"));
    }
}
