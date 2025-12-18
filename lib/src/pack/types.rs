//! Pack type definitions
//!
//! Contains all supporting types for pack definitions including steps,
//! inputs, HTTP/file configurations, and reporting/scoring options.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Input value type
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    /// Single string value (default)
    #[default]
    String,
    /// Array of strings (comma-separated input, quoted in substitution)
    Array,
}

/// User-provided input definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
    /// Input name (used in {{inputs.name}})
    pub name: String,

    /// Input type (string or array)
    #[serde(default, rename = "type")]
    pub input_type: InputType,

    /// Human-readable label
    #[serde(default)]
    pub label: Option<String>,

    /// Description
    #[serde(default)]
    pub description: Option<String>,

    /// Default value
    #[serde(default)]
    pub default: Option<String>,

    /// Whether input is required
    #[serde(default = "default_true")]
    pub required: bool,

    /// Example value for validation (used when validating queries with substitution)
    #[serde(default)]
    pub example: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Parse user input value using JSON syntax
///
/// Accepts JSON strings and arrays, rejects all other JSON types:
/// - **JSON string**: `"value"` → normalized to `value`
/// - **JSON array**: `["item1", "item2"]` → normalized to `item1,item2`
/// - **Raw value**: `value` → kept as-is (for simple identifiers/numbers)
///
/// This normalization allows the value to be interpreted correctly based on
/// the pack's `InputType` definition during execution:
/// - `InputType::String` uses the value as-is
/// - `InputType::Array` splits on commas and quotes each element
///
/// # Examples
///
/// ```
/// use kql_panopticon_core::pack::parse_input_value;
///
/// // JSON array
/// assert_eq!(
///     parse_input_value(r#"["8.8.8.8", "1.1.1.1"]"#).unwrap(),
///     "8.8.8.8,1.1.1.1"
/// );
///
/// // Empty array
/// assert_eq!(
///     parse_input_value(r#"[]"#).unwrap(),
///     ""
/// );
///
/// // JSON string
/// assert_eq!(
///     parse_input_value(r#""some value""#).unwrap(),
///     "some value"
/// );
///
/// // Raw value (non-JSON fallback)
/// assert_eq!(
///     parse_input_value("P7D").unwrap(),
///     "P7D"
/// );
///
/// // Note: Valid JSON numbers/booleans are rejected
/// assert!(parse_input_value("7").is_err());
/// assert!(parse_input_value("true").is_err());
/// ```
pub fn parse_input_value(input: &str) -> Result<String, String> {
    use serde_json::Value;

    let trimmed = input.trim();

    // Empty input
    if trimmed.is_empty() {
        return Err("Input value cannot be empty".to_string());
    }

    // Try to parse as JSON first
    match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::String(s)) => {
            // JSON string - extract the value
            Ok(s)
        }
        Ok(Value::Array(arr)) => {
            // JSON array - extract string items and join with commas
            let mut items = Vec::new();
            for (idx, item) in arr.iter().enumerate() {
                match item {
                    Value::String(s) => items.push(s.clone()),
                    _ => {
                        return Err(format!(
                            "Array item at index {} must be a string, found: {}",
                            idx,
                            match item {
                                Value::Null => "null",
                                Value::Bool(_) => "boolean",
                                Value::Number(_) => "number",
                                Value::Array(_) => "nested array",
                                Value::Object(_) => "object",
                                Value::String(_) => unreachable!(),
                            }
                        ));
                    }
                }
            }
            Ok(items.join(","))
        }
        Ok(other) => {
            // Other JSON types not supported
            Err(format!(
                "Input must be a JSON string or array, found: {}",
                match other {
                    Value::Null => "null",
                    Value::Bool(_) => "boolean",
                    Value::Number(_) => "number",
                    Value::Object(_) => "object",
                    _ => unreachable!(),
                }
            ))
        }
        Err(_) => {
            // Not valid JSON - treat as raw value
            // Allow simple unquoted values like: 7, simple-name, 8.8.8.8,1.1.1.1
            Ok(trimmed.to_string())
        }
    }
}

/// A single execution step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Step name (unique identifier)
    pub name: String,

    /// Step type (kql or http)
    #[serde(default, rename = "type")]
    pub step_type: StepType,

    /// KQL query (for KQL steps)
    #[serde(default)]
    pub query: Option<String>,

    /// Timespan for the query (e.g., "P7D")
    #[serde(default)]
    pub timespan: Option<String>,

    /// HTTP request configuration (for HTTP steps)
    #[serde(default)]
    pub request: Option<HttpRequest>,

    /// HTTP response field mapping (for HTTP steps)
    #[serde(default)]
    pub response: Option<HttpResponse>,

    /// File source configuration (for File steps)
    #[serde(default)]
    pub source: Option<FileSource>,

    /// Rate limiting for HTTP steps
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,

    /// Error handling behavior
    #[serde(default)]
    pub on_error: Option<OnError>,

    /// Steps this step depends on (must complete first)
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// Condition for executing this step
    #[serde(default)]
    pub when: Option<String>,

    /// Step-level options
    #[serde(default)]
    pub options: Option<StepOptions>,

    /// Example values for validation (maps variable refs to example values)
    /// Used to substitute realistic values during KQL validation.
    /// Keys are variable references without braces: "step.*.Column" or "step.first.Column"
    #[serde(default)]
    pub examples: HashMap<String, ExampleValue>,
}

/// Example value for validation substitution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExampleValue {
    /// Single example value
    Single(String),
    /// Array of example values (for .* references)
    Array(Vec<String>),
}

/// Acquisition step type (KQL, HTTP, File)
///
/// This is used in pack definitions for the `type` field of acquisition steps.
/// For execution-layer types, see `execution::StepType`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AcquisitionStepType {
    #[default]
    Kql,
    Http,
    File,
}

// Re-export as StepType for backward compatibility with Step struct
pub use AcquisitionStepType as StepType;

/// HTTP request configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    /// HTTP method
    pub method: HttpMethod,

    /// URL (supports variable substitution)
    pub url: String,

    /// Query parameters
    #[serde(default)]
    pub params: HashMap<String, String>,

    /// Headers
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Request body
    #[serde(default)]
    pub body: Option<serde_json::Value>,

    /// Authentication method
    #[serde(default)]
    pub auth: Option<AuthMethod>,
}

/// HTTP method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

/// Authentication method for HTTP steps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    /// Use Azure CLI credential
    Azure,
    /// No authentication
    None,
}

/// HTTP response field mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    /// Field mappings (column_name -> JSONPath)
    #[serde(default)]
    pub fields: HashMap<String, String>,
}

/// File source configuration (for File steps)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSource {
    /// File path (supports variable substitution)
    pub path: String,

    /// File format (auto-detected from extension if not specified)
    #[serde(default)]
    pub format: Option<FileFormat>,

    /// CSV-specific options
    #[serde(default)]
    pub csv: Option<CsvOptions>,
}

/// File format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    Csv,
    Json,
    Yaml,
}

/// CSV parsing options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CsvOptions {
    /// Delimiter character (default: comma)
    #[serde(default)]
    pub delimiter: Option<char>,

    /// Whether file has header row (default: true)
    #[serde(default)]
    pub has_header: Option<bool>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Number of requests allowed
    pub requests: u32,

    /// Time period
    pub per: RateLimitPeriod,
}

/// Time period for rate limiting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitPeriod {
    Second,
    Minute,
    Hour,
}

/// Error handling behavior
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnError {
    /// Fail the step
    #[default]
    Fail,
    /// Skip and continue
    Skip,
    /// Record error, continue
    Continue,
}

/// Step-level options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepOptions {
    /// Quote style for variable substitution
    #[serde(default)]
    pub quote_style: Option<QuoteStyle>,

    /// Deduplicate extracted values
    #[serde(default)]
    pub dedupe: Option<bool>,

    /// Chunk size for large arrays
    #[serde(default)]
    pub chunk_size: Option<usize>,
}

/// Quote style for value substitution
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuoteStyle {
    /// Single quotes: 'value'
    #[default]
    Single,
    /// Double quotes: "value"
    Double,
    /// KQL verbatim: @'value'
    Verbatim,
}

impl QuoteStyle {
    /// Format a single value
    pub fn format_value(&self, value: &str) -> String {
        match self {
            QuoteStyle::Single => {
                let escaped = value.replace('\'', "''");
                format!("'{}'", escaped)
            }
            QuoteStyle::Double => {
                let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                format!("\"{}\"", escaped)
            }
            QuoteStyle::Verbatim => {
                let escaped = value.replace('\'', "''");
                format!("@'{}'", escaped)
            }
        }
    }

    /// Format an array of values
    pub fn format_array(&self, values: &[String]) -> String {
        values
            .iter()
            .map(|v| self.format_value(v))
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Output folder configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Folder template
    #[serde(default)]
    pub folder: Option<String>,
}

/// Secrets configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsConfig {
    /// Secret mappings (name -> env var template)
    #[serde(flatten)]
    pub secrets: HashMap<String, String>,
}

// Note: ReportConfig, ScoringConfig and related types have been moved to
// pack/reporting.rs and pack/processing.rs respectively.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quote_styles() {
        assert_eq!(QuoteStyle::Single.format_value("test"), "'test'");
        assert_eq!(QuoteStyle::Single.format_value("O'Brien"), "'O''Brien'");
        assert_eq!(QuoteStyle::Double.format_value("test"), "\"test\"");
        assert_eq!(QuoteStyle::Verbatim.format_value("test"), "@'test'");
    }

    #[test]
    fn test_parse_input_value_json_arrays() {
        // Basic array
        assert_eq!(
            parse_input_value(r#"["8.8.8.8", "1.1.1.1"]"#).unwrap(),
            "8.8.8.8,1.1.1.1"
        );

        // Single item array
        assert_eq!(
            parse_input_value(r#"["single"]"#).unwrap(),
            "single"
        );

        // Empty array
        assert_eq!(
            parse_input_value(r#"[]"#).unwrap(),
            ""
        );

        // Array with whitespace (JSON handles this)
        assert_eq!(
            parse_input_value(r#"["item1" , "item2" , "item3"]"#).unwrap(),
            "item1,item2,item3"
        );

        // Array with escaped quotes (JSON handles this)
        assert_eq!(
            parse_input_value(r#"["item \"with\" quotes"]"#).unwrap(),
            r#"item "with" quotes"#
        );

        // Array with commas in values
        assert_eq!(
            parse_input_value(r#"["value,with,commas", "normal"]"#).unwrap(),
            "value,with,commas,normal"
        );
    }

    #[test]
    fn test_parse_input_value_json_array_errors() {
        // Non-string array items rejected
        assert!(parse_input_value(r#"[1, 2, 3]"#).is_err());
        assert!(parse_input_value(r#"[true, false]"#).is_err());
        assert!(parse_input_value(r#"[null]"#).is_err());
        assert!(parse_input_value(r#"[{"key": "value"}]"#).is_err());
        assert!(parse_input_value(r#"[["nested"]]"#).is_err());
    }

    #[test]
    fn test_parse_input_value_json_strings() {
        // Double quotes (JSON standard)
        assert_eq!(
            parse_input_value(r#""some value""#).unwrap(),
            "some value"
        );

        // String with escaped quotes
        assert_eq!(
            parse_input_value(r#""value with \"quotes\"""#).unwrap(),
            r#"value with "quotes""#
        );

        // String with commas (kept literal)
        assert_eq!(
            parse_input_value(r#""a,b,c""#).unwrap(),
            "a,b,c"
        );

        // String with escaped backslash
        assert_eq!(
            parse_input_value(r#""path\\to\\file""#).unwrap(),
            r#"path\to\file"#
        );

        // String with unicode escapes
        assert_eq!(
            parse_input_value(r#""hello\u0020world""#).unwrap(),
            "hello world"
        );

        // Empty string
        assert_eq!(parse_input_value(r#""""#).unwrap(), "");
    }

    #[test]
    fn test_parse_input_value_invalid_json_fallback() {
        // Invalid JSON syntax falls back to raw values

        // Single quotes not valid JSON - treated as raw
        assert_eq!(parse_input_value(r#"'single quotes'"#).unwrap(), "'single quotes'");

        // These look like arrays but aren't valid JSON - treated as raw
        assert_eq!(parse_input_value(r#"[',',']"#).unwrap(), "[',',']");
    }

    #[test]
    fn test_parse_input_value_json_type_rejection() {
        // Numbers rejected
        assert!(parse_input_value("123").is_err());
        assert!(parse_input_value("123.45").is_err());

        // Booleans rejected
        assert!(parse_input_value("true").is_err());
        assert!(parse_input_value("false").is_err());

        // Null rejected
        assert!(parse_input_value("null").is_err());

        // Objects rejected
        assert!(parse_input_value(r#"{"key": "value"}"#).is_err());
    }

    #[test]
    fn test_parse_input_value_raw_fallback() {
        // Raw values that aren't valid JSON are accepted as-is

        // Simple identifiers
        assert_eq!(parse_input_value("P7D").unwrap(), "P7D");
        assert_eq!(parse_input_value("admin").unwrap(), "admin");

        // Comma-separated (for array type inputs)
        assert_eq!(
            parse_input_value("8.8.8.8,1.1.1.1").unwrap(),
            "8.8.8.8,1.1.1.1"
        );

        // URL-like strings
        assert_eq!(
            parse_input_value("https://example.com").unwrap(),
            "https://example.com"
        );

        // Values with special chars (not valid JSON)
        assert_eq!(parse_input_value("user@domain.com").unwrap(), "user@domain.com");
    }

    #[test]
    fn test_parse_input_value_edge_cases() {
        // Empty input
        assert!(parse_input_value("").is_err());

        // Whitespace only
        assert!(parse_input_value("   ").is_err());

        // Raw values that happen to be valid JSON of wrong type get rejected
        assert!(parse_input_value("7").is_err());  // Valid JSON number
        assert!(parse_input_value("true").is_err());  // Valid JSON boolean
    }
}
