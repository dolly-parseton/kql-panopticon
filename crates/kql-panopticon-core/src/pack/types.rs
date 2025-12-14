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

    /// Foreach iteration: "step_name as alias"
    #[serde(default)]
    pub foreach: Option<String>,

    /// Batch size for foreach iterations
    #[serde(default)]
    pub batch_size: Option<usize>,

    /// How to aggregate foreach results
    #[serde(default)]
    pub aggregate: Option<AggregateStrategy>,

    /// Behavior when foreach source is empty
    #[serde(default)]
    pub on_empty: Option<OnEmpty>,

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

/// Aggregation strategy for foreach results
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AggregateStrategy {
    /// Concatenate all result rows
    #[default]
    Append,
    /// Deep merge result objects
    Merge,
    /// Keep only last iteration
    Replace,
    /// Wrap each iteration, keyed by source
    Collect,
}

/// Behavior when foreach source is empty
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnEmpty {
    /// Skip the step
    #[default]
    Skip,
    /// Fail the execution
    Error,
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

/// Parsed foreach clause
#[derive(Debug, Clone)]
pub struct ForeachClause {
    /// Source step name
    pub source_step: String,
    /// Alias for current row/batch
    pub alias: String,
}

impl ForeachClause {
    /// Parse "step_name as alias"
    pub fn parse(foreach: &str) -> Option<Self> {
        let parts: Vec<&str> = foreach.split_whitespace().collect();
        if parts.len() == 3 && parts[1].eq_ignore_ascii_case("as") {
            Some(ForeachClause {
                source_step: parts[0].to_string(),
                alias: parts[2].to_string(),
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foreach_syntax() {
        let clause = ForeachClause::parse("step1 as item").unwrap();
        assert_eq!(clause.source_step, "step1");
        assert_eq!(clause.alias, "item");

        assert!(ForeachClause::parse("step1 item").is_none());
        assert!(ForeachClause::parse("").is_none());
    }

    #[test]
    fn test_quote_styles() {
        assert_eq!(QuoteStyle::Single.format_value("test"), "'test'");
        assert_eq!(QuoteStyle::Single.format_value("O'Brien"), "'O''Brien'");
        assert_eq!(QuoteStyle::Double.format_value("test"), "\"test\"");
        assert_eq!(QuoteStyle::Verbatim.format_value("test"), "@'test'");
    }
}
