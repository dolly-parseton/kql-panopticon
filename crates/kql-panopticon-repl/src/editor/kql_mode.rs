//! KQL editor mode
//!
//! Provides KQL-specific configuration for the editor:
//! - Syntax highlighting via FFI
//! - Code completion for operators, functions, columns
//! - Session-aware `{{reference}}` completion

use super::completion::{CompletionItem, CompletionItemKind, CompletionSource};
use super::highlight::{Highlighter, HighlightSpan, KqlHighlighter};
use super::widget::{EditorConfig, EditorResult, TuiEditor};
use crate::session::PackSession;
use kql_panopticon_core::validation::{KqlValidator, Schema};
use std::io;
use std::sync::Arc;

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
    fn get_reference_completions(&self, prefix: &str) -> Vec<CompletionItem> {
        let Some(session) = &self.session else {
            return Vec::new();
        };

        let mut items = Vec::new();

        // Check if completing inputs
        if prefix.is_empty() || prefix.starts_with("inputs.") {
            let input_prefix = prefix.strip_prefix("inputs.").unwrap_or("");

            for (name, input) in &session.inputs {
                if input_prefix.is_empty() || name.starts_with(input_prefix) {
                    items.push(CompletionItem {
                        label: format!("inputs.{}", name),
                        kind: CompletionItemKind::Input,
                        detail: Some(input.input_type.to_string()),
                        insert_text: Some(format!("inputs.{}", name)),
                        edit_start: 0,
                    });
                }
            }
        }

        // Check if completing steps
        if prefix.is_empty() || !prefix.starts_with("inputs.") {
            for (name, step) in &session.steps {
                if prefix.is_empty() || name.starts_with(prefix) {
                    // Step reference (entire result)
                    items.push(CompletionItem {
                        label: name.clone(),
                        kind: CompletionItemKind::Step,
                        detail: step.description.clone(),
                        insert_text: Some(name.clone()),
                        edit_start: 0,
                    });

                    // Step.first accessor (common pattern)
                    items.push(CompletionItem {
                        label: format!("{}.first", name),
                        kind: CompletionItemKind::Step,
                        detail: Some("First row accessor".to_string()),
                        insert_text: Some(format!("{}.first", name)),
                        edit_start: 0,
                    });
                }
            }
        }

        items
    }

    /// Convert FFI completion items to our format
    fn convert_ffi_completions(
        &self,
        ffi_items: Vec<kql_panopticon_core::validation::CompletionItem>,
    ) -> Vec<CompletionItem> {
        use kql_panopticon_core::validation::CompletionKind as FfiCompletionKind;

        ffi_items
            .into_iter()
            .map(|item| {
                let kind = match item.kind {
                    FfiCompletionKind::Keyword => CompletionItemKind::Keyword,
                    FfiCompletionKind::Function
                    | FfiCompletionKind::AggregateFunction => {
                        CompletionItemKind::Function
                    }
                    FfiCompletionKind::Table => CompletionItemKind::Table,
                    FfiCompletionKind::Column => CompletionItemKind::Column,
                    FfiCompletionKind::Operator => CompletionItemKind::Operator,
                    _ => CompletionItemKind::Other,
                };

                CompletionItem {
                    label: item.label.clone(),
                    kind,
                    detail: item.detail,
                    insert_text: item.insert_text,
                    // Use the edit_start from FFI (from Kusto library)
                    edit_start: item.edit_start,
                }
            })
            .collect()
    }
}

impl CompletionSource for KqlCompletionSource {
    fn get_completions(&self, content: &str, cursor_offset: usize) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // Check if we're inside a `{{` reference
        if cursor_offset >= 2 {
            let before_cursor = &content[..cursor_offset];

            // Find the last `{{`
            if let Some(ref_start) = before_cursor.rfind("{{") {
                // Check if there's no closing `}}` between the start and cursor
                let between = &before_cursor[ref_start + 2..];
                if !between.contains("}}") {
                    // We're inside a reference - get reference completions
                    let prefix = between;
                    return self.get_reference_completions(prefix);
                }
            }
        }

        // Get KQL completions from FFI
        if let Some(validator) = &self.validator {
            if validator.supports_completion() {
                let schema_ref = self.schema.as_ref();
                match validator.get_completions(content, cursor_offset, schema_ref) {
                    Ok(result) => {
                        // Convert FFI items (preserving their edit_start)
                        let converted = self.convert_ffi_completions(result.items);

                        // Filter by prefix: extract what the user has typed
                        // from each item's edit_start to the cursor
                        let filtered: Vec<CompletionItem> = converted
                            .into_iter()
                            .filter(|item| {
                                // Get the prefix the user typed (from edit_start to cursor)
                                if item.edit_start <= cursor_offset && item.edit_start <= content.len() {
                                    let prefix = &content[item.edit_start..cursor_offset.min(content.len())];
                                    // Case-insensitive prefix match
                                    let prefix_lower = prefix.to_lowercase();
                                    let label_lower = item.label.to_lowercase();
                                    label_lower.starts_with(&prefix_lower)
                                } else {
                                    // If edit_start is beyond cursor, include the item
                                    true
                                }
                            })
                            .collect();

                        items.extend(filtered);
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

        // Get all reference completions
        let items = source.get_reference_completions("");
        assert!(!items.is_empty());

        // Should have input completion
        let input_item = items.iter().find(|i| i.label == "inputs.ip");
        assert!(input_item.is_some());

        // Should have step completion
        let step_item = items.iter().find(|i| i.label == "events");
        assert!(step_item.is_some());
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
        let items = source.get_reference_completions("inputs.i");
        assert!(items.iter().any(|i| i.label == "inputs.ip"));
        assert!(!items.iter().any(|i| i.label == "inputs.account"));
    }
}
