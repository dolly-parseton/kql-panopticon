//! Completion system for the TUI interpreter
//!
//! Provides context-aware tab completion for commands and let statements.

use crate::app::{BlockStatus, InterpreterBlock, LoadedPack};

/// A completion suggestion
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// Text to insert
    pub value: String,
    /// Optional description
    pub description: Option<String>,
    /// Byte offset where replacement starts
    pub replace_start: usize,
    /// Byte offset where replacement ends
    pub replace_end: usize,
}

/// Completion context detected from input
#[derive(Debug)]
enum CompletionContext {
    /// Completing a : command (e.g., ":lo" -> ":load")
    Command { prefix: String },
    /// Completing "let" keyword
    LetKeyword { prefix: String },
    /// Completing variable name after "let "
    VariableName { prefix: String },
    /// Completing type after "let name:"
    LetType { prefix: String },
    /// Completing " = " after "let name:type"
    Equals,
    /// Completing value after "let name:type = "
    Value { var_name: String, prefix: String },
    /// No completion available
    None,
}

/// Get completions for the given input at the cursor position
pub fn get_completions(
    input: &str,
    cursor: usize,
    loaded_pack: Option<&LoadedPack>,
    blocks: &[InterpreterBlock],
) -> Vec<Suggestion> {
    let context = detect_context(input, cursor);
    generate_suggestions(&context, loaded_pack, blocks, cursor, input)
}

/// Detect the completion context from input and cursor position
fn detect_context(input: &str, cursor: usize) -> CompletionContext {
    // Handle cursor beyond input length
    let cursor = cursor.min(input.len());
    let before_cursor = &input[..cursor];
    let trimmed = before_cursor.trim_start();

    // Empty or whitespace only -> no completion
    if trimmed.is_empty() {
        return CompletionContext::None;
    }

    // : command completion
    if trimmed.starts_with(':') {
        return CompletionContext::Command {
            prefix: trimmed.to_string(),
        };
    }

    // "let" keyword completion
    if "let".starts_with(trimmed) && !trimmed.contains(' ') {
        return CompletionContext::LetKeyword {
            prefix: trimmed.to_string(),
        };
    }

    // Inside a let statement
    if trimmed.starts_with("let ") {
        return detect_let_context(trimmed);
    }

    CompletionContext::None
}

/// Detect completion context within a let statement
fn detect_let_context(input: &str) -> CompletionContext {
    let after_let = &input[4..]; // Skip "let "

    // Check for "=" - we're in value territory
    if let Some(eq_pos) = after_let.find('=') {
        let before_eq = &after_let[..eq_pos];
        let after_eq = after_let[eq_pos + 1..].trim_start();

        // Extract variable name
        if let Some(colon_pos) = before_eq.find(':') {
            let var_name = before_eq[..colon_pos].trim();
            return CompletionContext::Value {
                var_name: var_name.to_string(),
                prefix: after_eq.to_string(),
            };
        }
    }

    // Check for ":" - we're completing type or equals
    if let Some(colon_pos) = after_let.find(':') {
        let after_colon = &after_let[colon_pos + 1..];
        let after_colon_trimmed = after_colon.trim();

        // Type is complete if it matches known types
        if after_colon_trimmed == "input" {
            return CompletionContext::Equals;
        }

        // Completing type
        return CompletionContext::LetType {
            prefix: after_colon_trimmed.to_string(),
        };
    }

    // Completing variable name
    CompletionContext::VariableName {
        prefix: after_let.trim().to_string(),
    }
}

/// Generate suggestions based on the detected context
fn generate_suggestions(
    context: &CompletionContext,
    loaded_pack: Option<&LoadedPack>,
    blocks: &[InterpreterBlock],
    cursor: usize,
    input: &str,
) -> Vec<Suggestion> {
    match context {
        CompletionContext::Command { prefix } => complete_commands(prefix, cursor),
        CompletionContext::LetKeyword { prefix } => complete_let_keyword(prefix, cursor),
        CompletionContext::VariableName { prefix } => {
            complete_variable_names(prefix, cursor, loaded_pack)
        }
        CompletionContext::LetType { prefix } => complete_types(prefix, cursor, input),
        CompletionContext::Equals => suggest_equals(cursor),
        CompletionContext::Value { var_name, prefix } => {
            complete_values(var_name, prefix, cursor, loaded_pack, blocks)
        }
        CompletionContext::None => vec![],
    }
}

/// Complete : commands
fn complete_commands(prefix: &str, cursor: usize) -> Vec<Suggestion> {
    let commands = [
        (":ws", "Open workspace selector"),
        (":workspace", "Open workspace selector"),
        (":theme", "Open theme selector"),
        (":load", "Load a pack"),
        (":logs", "View execution logs"),
        (":run", "Execute loaded pack"),
    ];

    commands
        .iter()
        .filter(|(cmd, _)| cmd.starts_with(prefix))
        .map(|(cmd, desc)| Suggestion {
            value: cmd.to_string(),
            description: Some(desc.to_string()),
            replace_start: cursor - prefix.len(),
            replace_end: cursor,
        })
        .collect()
}

/// Complete "let" keyword
fn complete_let_keyword(prefix: &str, cursor: usize) -> Vec<Suggestion> {
    if "let".starts_with(prefix) {
        vec![Suggestion {
            value: "let ".to_string(),
            description: Some("Define input variable".to_string()),
            replace_start: cursor - prefix.len(),
            replace_end: cursor,
        }]
    } else {
        vec![]
    }
}

/// Complete variable names from loaded pack
fn complete_variable_names(
    prefix: &str,
    cursor: usize,
    loaded_pack: Option<&LoadedPack>,
) -> Vec<Suggestion> {
    let Some(pack) = loaded_pack else {
        return vec![];
    };

    pack.pack
        .acquisition
        .inputs
        .iter()
        .filter(|input| input.name.starts_with(prefix))
        .map(|input| Suggestion {
            value: input.name.clone(),
            description: input.description.clone(),
            replace_start: cursor - prefix.len(),
            replace_end: cursor,
        })
        .collect()
}

/// Complete let types
fn complete_types(prefix: &str, cursor: usize, input: &str) -> Vec<Suggestion> {
    let types = [("input", "User-provided input value")];

    // Calculate replace_start: find where the type starts (after the colon)
    let colon_pos = input[..cursor].rfind(':').unwrap_or(cursor);
    let type_start = colon_pos + 1;
    // Skip any whitespace after colon
    let type_start = input[type_start..cursor]
        .find(|c: char| !c.is_whitespace())
        .map(|i| type_start + i)
        .unwrap_or(cursor);

    types
        .iter()
        .filter(|(t, _)| t.starts_with(prefix))
        .map(|(t, desc)| Suggestion {
            value: t.to_string(),
            description: Some(desc.to_string()),
            replace_start: type_start,
            replace_end: cursor,
        })
        .collect()
}

/// Suggest " = " after complete type
fn suggest_equals(cursor: usize) -> Vec<Suggestion> {
    vec![Suggestion {
        value: " = ".to_string(),
        description: Some("Assignment operator".to_string()),
        replace_start: cursor,
        replace_end: cursor,
    }]
}

/// Complete values from history and pack defaults
fn complete_values(
    var_name: &str,
    prefix: &str,
    cursor: usize,
    loaded_pack: Option<&LoadedPack>,
    blocks: &[InterpreterBlock],
) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();
    let mut seen_values = std::collections::HashSet::new();

    // Extract previous values from completed blocks
    for block in blocks
        .iter()
        .filter(|b| b.status == BlockStatus::Completed)
    {
        if let Some(value) = extract_let_value(&block.command, var_name) {
            if value.starts_with(prefix) && seen_values.insert(value.clone()) {
                suggestions.push(Suggestion {
                    value,
                    description: Some("Previous value".to_string()),
                    replace_start: cursor - prefix.len(),
                    replace_end: cursor,
                });
            }
        }
    }

    // Add default and example from pack if available
    if let Some(pack) = loaded_pack {
        if let Some(input) = pack
            .pack
            .acquisition
            .inputs
            .iter()
            .find(|i| i.name == var_name)
        {
            if let Some(default) = &input.default {
                if default.starts_with(prefix) && seen_values.insert(default.clone()) {
                    suggestions.push(Suggestion {
                        value: default.clone(),
                        description: Some("Default".to_string()),
                        replace_start: cursor - prefix.len(),
                        replace_end: cursor,
                    });
                }
            }
            if let Some(example) = &input.example {
                if example.starts_with(prefix) && seen_values.insert(example.clone()) {
                    suggestions.push(Suggestion {
                        value: example.clone(),
                        description: Some("Example".to_string()),
                        replace_start: cursor - prefix.len(),
                        replace_end: cursor,
                    });
                }
            }
        }
    }

    suggestions
}

/// Extract value from a let statement like "let foo:input = bar"
fn extract_let_value(command: &str, target_var: &str) -> Option<String> {
    let command = command.trim();
    if !command.starts_with("let ") {
        return None;
    }

    let after_let = &command[4..];
    let eq_pos = after_let.find('=')?;
    let colon_pos = after_let.find(':')?;

    let var_name = after_let[..colon_pos].trim();
    if var_name != target_var {
        return None;
    }

    let value = after_let[eq_pos + 1..].trim();
    Some(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_command_context() {
        let ctx = detect_context(":lo", 3);
        assert!(matches!(ctx, CompletionContext::Command { prefix } if prefix == ":lo"));
    }

    #[test]
    fn test_detect_let_keyword() {
        let ctx = detect_context("le", 2);
        assert!(matches!(ctx, CompletionContext::LetKeyword { prefix } if prefix == "le"));
    }

    #[test]
    fn test_detect_variable_name() {
        let ctx = detect_context("let foo", 7);
        assert!(matches!(ctx, CompletionContext::VariableName { prefix } if prefix == "foo"));
    }

    #[test]
    fn test_detect_type() {
        let ctx = detect_context("let foo:inp", 11);
        assert!(matches!(ctx, CompletionContext::LetType { prefix } if prefix == "inp"));
    }

    #[test]
    fn test_detect_equals() {
        let ctx = detect_context("let foo:input", 13);
        assert!(matches!(ctx, CompletionContext::Equals));
    }

    #[test]
    fn test_detect_value() {
        let ctx = detect_context("let foo:input = bar", 19);
        assert!(
            matches!(ctx, CompletionContext::Value { var_name, prefix } if var_name == "foo" && prefix == "bar")
        );
    }

    #[test]
    fn test_complete_commands() {
        let suggestions = complete_commands(":lo", 3);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].value, ":load");
    }

    #[test]
    fn test_extract_let_value() {
        let value = extract_let_value("let foo:input = bar", "foo");
        assert_eq!(value, Some("bar".to_string()));

        let value = extract_let_value("let foo:input = bar", "baz");
        assert_eq!(value, None);
    }
}
