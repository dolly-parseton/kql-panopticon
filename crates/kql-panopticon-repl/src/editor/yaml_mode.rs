//! YAML editor mode
//!
//! Provides YAML-specific configuration for editing input definitions:
//! - Simple YAML syntax highlighting
//! - Type completion for input types

use super::completion::{CompletionItem, CompletionItemKind, CompletionSource};
use super::highlight::YamlHighlighter;
use super::widget::{EditorConfig, EditorResult, TuiEditor};
use crate::session::{InputDef, InputType};
use std::io;

/// YAML editor mode configuration
pub struct YamlEditorMode {
    /// The highlighter
    highlighter: YamlHighlighter,
    /// The completion source
    completion_source: YamlCompletionSource,
}

impl YamlEditorMode {
    /// Create a new YAML editor mode
    pub fn new() -> Self {
        Self {
            highlighter: YamlHighlighter::new(),
            completion_source: YamlCompletionSource::new(),
        }
    }

    /// Run the editor with the given configuration
    pub fn run(self, config: EditorConfig) -> io::Result<EditorResult> {
        let editor = TuiEditor::new(config, self.highlighter, self.completion_source);
        editor.run()
    }
}

impl Default for YamlEditorMode {
    fn default() -> Self {
        Self::new()
    }
}

/// YAML completion source (for input type values)
pub struct YamlCompletionSource;

impl YamlCompletionSource {
    /// Create a new YAML completion source
    pub fn new() -> Self {
        Self
    }
}

impl Default for YamlCompletionSource {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionSource for YamlCompletionSource {
    fn get_completions(&self, content: &str, cursor_offset: usize) -> Vec<CompletionItem> {
        // Get the current line and position within it
        let before_cursor = &content[..cursor_offset];
        let current_line = before_cursor.lines().last().unwrap_or("");

        // Check if we're completing a type value
        if current_line.trim_start().starts_with("type:") {
            let colon_pos = current_line.find(':').unwrap_or(0);
            let value_part = &current_line[colon_pos + 1..];
            let prefix = value_part.trim();

            return INPUT_TYPES
                .iter()
                .filter(|(name, _)| prefix.is_empty() || name.starts_with(prefix))
                .map(|(name, desc)| CompletionItem {
                    label: (*name).to_string(),
                    kind: CompletionItemKind::Keyword,
                    detail: Some((*desc).to_string()),
                    insert_text: Some((*name).to_string()),
                    edit_start: cursor_offset - prefix.len(),
                })
                .collect();
        }

        // Check if we're completing a boolean value (required, etc.)
        if current_line.trim_start().starts_with("required:") {
            let colon_pos = current_line.find(':').unwrap_or(0);
            let value_part = &current_line[colon_pos + 1..];
            let prefix = value_part.trim();

            return ["true", "false"]
                .iter()
                .filter(|v| prefix.is_empty() || v.starts_with(prefix))
                .map(|v| CompletionItem {
                    label: (*v).to_string(),
                    kind: CompletionItemKind::Keyword,
                    detail: None,
                    insert_text: Some((*v).to_string()),
                    edit_start: cursor_offset - prefix.len(),
                })
                .collect();
        }

        Vec::new()
    }
}

/// Available input types with descriptions
const INPUT_TYPES: &[(&str, &str)] = &[
    ("string", "Text value"),
    ("int", "Integer number"),
    ("bool", "Boolean (true/false)"),
    ("datetime", "Date and time"),
    ("timespan", "Duration (e.g., P7D, PT1H)"),
];

/// Parse an input definition from YAML content
pub fn parse_input_yaml(content: &str) -> Result<InputDef, String> {
    // Simple YAML parsing for our specific structure
    let mut name = None;
    let mut input_type = InputType::String;
    let mut description = None;
    let mut required = true;
    let mut default = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim();
            let value = line[colon_pos + 1..].trim();

            // Strip comments from value
            let value = value.split('#').next().unwrap_or(value).trim();

            // Strip quotes from value
            let value = value
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(value);

            match key {
                "name" => name = Some(value.to_string()),
                "type" => {
                    input_type = match value.to_lowercase().as_str() {
                        "string" => InputType::String,
                        "int" | "integer" => InputType::Int,
                        "bool" | "boolean" => InputType::Bool,
                        "datetime" => InputType::Datetime,
                        "timespan" => InputType::Timespan,
                        _ => {
                            return Err(format!(
                                "Invalid type '{}'. Valid types: string, int, bool, datetime, timespan",
                                value
                            ))
                        }
                    };
                }
                "description" => {
                    if !value.is_empty() {
                        description = Some(value.to_string());
                    }
                }
                "required" => {
                    required = value.eq_ignore_ascii_case("true") || value == "yes";
                }
                "default" => {
                    if !value.eq_ignore_ascii_case("null") && value != "~" {
                        default = Some(value.to_string());
                    }
                }
                _ => {
                    // Ignore unknown keys
                }
            }
        }
    }

    let name = name.ok_or("Missing required field: name")?;

    Ok(InputDef {
        name,
        input_type,
        description,
        required,
        default,
    })
}

/// Serialize an input definition to YAML
pub fn input_to_yaml(input: &InputDef) -> String {
    let mut yaml = String::new();

    yaml.push_str(&format!("name: {}\n", input.name));
    yaml.push_str(&format!(
        "type: {}        # string | int | bool | datetime | timespan\n",
        input.input_type
    ));

    match &input.description {
        Some(desc) => yaml.push_str(&format!("description: \"{}\"\n", desc)),
        None => yaml.push_str("description: \"\"\n"),
    }

    yaml.push_str(&format!("required: {}\n", input.required));

    match &input.default {
        Some(val) => yaml.push_str(&format!("default: \"{}\"\n", val)),
        None => yaml.push_str("default: null\n"),
    }

    yaml
}

/// Convenience function to run a YAML editor for input definition
pub fn run_yaml_input_editor(name: &str, existing: Option<&InputDef>) -> io::Result<EditorResult> {
    let config = match existing {
        Some(input) => EditorConfig::yaml_input(name).with_content(input_to_yaml(input)),
        None => EditorConfig::yaml_input(name),
    };

    YamlEditorMode::new().run(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_input_yaml_basic() {
        let yaml = r#"
name: threat_ip
type: string
description: "IP address to investigate"
required: true
default: null
"#;

        let result = parse_input_yaml(yaml);
        assert!(result.is_ok());

        let input = result.unwrap();
        assert_eq!(input.name, "threat_ip");
        assert_eq!(input.input_type, InputType::String);
        assert_eq!(input.description, Some("IP address to investigate".to_string()));
        assert!(input.required);
        assert!(input.default.is_none());
    }

    #[test]
    fn test_parse_input_yaml_with_default() {
        let yaml = r#"
name: lookback
type: timespan
description: "Time range"
required: false
default: P7D
"#;

        let result = parse_input_yaml(yaml);
        assert!(result.is_ok());

        let input = result.unwrap();
        assert_eq!(input.name, "lookback");
        assert_eq!(input.input_type, InputType::Timespan);
        assert!(!input.required);
        assert_eq!(input.default, Some("P7D".to_string()));
    }

    #[test]
    fn test_parse_input_yaml_with_comments() {
        let yaml = r#"
name: count
type: int  # integer number
description: ""
required: true
default: null
"#;

        let result = parse_input_yaml(yaml);
        assert!(result.is_ok());

        let input = result.unwrap();
        assert_eq!(input.input_type, InputType::Int);
    }

    #[test]
    fn test_parse_input_yaml_invalid_type() {
        let yaml = r#"
name: test
type: invalid_type
"#;

        let result = parse_input_yaml(yaml);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid type"));
    }

    #[test]
    fn test_parse_input_yaml_missing_name() {
        let yaml = r#"
type: string
required: true
"#;

        let result = parse_input_yaml(yaml);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required field: name"));
    }

    #[test]
    fn test_input_to_yaml_roundtrip() {
        let input = InputDef {
            name: "test_input".to_string(),
            input_type: InputType::String,
            description: Some("A test input".to_string()),
            required: true,
            default: None,
        };

        let yaml = input_to_yaml(&input);
        let parsed = parse_input_yaml(&yaml).unwrap();

        assert_eq!(parsed.name, input.name);
        assert_eq!(parsed.input_type, input.input_type);
        assert_eq!(parsed.description, input.description);
        assert_eq!(parsed.required, input.required);
        assert_eq!(parsed.default, input.default);
    }

    #[test]
    fn test_yaml_completion_source_types() {
        let source = YamlCompletionSource::new();
        let content = "name: test\ntype: ";
        let cursor = content.len();

        let items = source.get_completions(content, cursor);

        assert!(!items.is_empty());
        assert!(items.iter().any(|i| i.label == "string"));
        assert!(items.iter().any(|i| i.label == "int"));
        assert!(items.iter().any(|i| i.label == "bool"));
    }

    #[test]
    fn test_yaml_completion_source_booleans() {
        let source = YamlCompletionSource::new();
        let content = "name: test\nrequired: ";
        let cursor = content.len();

        let items = source.get_completions(content, cursor);

        assert!(!items.is_empty());
        assert!(items.iter().any(|i| i.label == "true"));
        assert!(items.iter().any(|i| i.label == "false"));
    }
}
