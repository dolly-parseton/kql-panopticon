//! File step execution handler
//!
//! Reads local files (CSV, JSON, YAML) and normalizes to uniform row format.

use crate::error::{Error, Result};
use crate::execution::step::{StepContext, StepHandler, StepOutput};
use crate::pack::{FileFormat, Step, StepType};
use crate::variable::substitute;
use async_trait::async_trait;
use log::debug;
use serde_json::Value as JsonValue;
use std::path::Path;
use std::time::Instant;

/// Handler for file source steps
pub(crate) struct FileHandler;

impl FileHandler {
    /// Create a new file handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StepHandler for FileHandler {
    fn step_type(&self) -> StepType {
        StepType::File
    }

    async fn execute(&self, step: &Step, ctx: &StepContext<'_>) -> Result<StepOutput> {
        let start = Instant::now();

        // Get source config
        let source = step.source.as_ref().ok_or_else(|| {
            Error::investigation(&step.name, "File step missing source configuration")
        })?;

        // Substitute variables in path
        let path = substitute(&source.path, ctx.substitution).map_err(|e| {
            Error::investigation(&step.name, format!("Path substitution failed: {}", e))
        })?;

        debug!("Executing File step '{}': {}", step.name, path);

        // Determine format
        let format = source.format.unwrap_or_else(|| detect_format(&path));

        // Read file content
        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            Error::investigation(&step.name, format!("Failed to read file '{}': {}", path, e))
        })?;

        // Parse based on format
        let rows = match format {
            FileFormat::Csv => parse_csv(&content, source.csv.as_ref(), &step.name)?,
            FileFormat::Json => parse_json(&content, &step.name)?,
            FileFormat::Yaml => parse_yaml(&content, &step.name)?,
        };

        debug!(
            "File step '{}' completed: {} rows in {:?}",
            step.name,
            rows.len(),
            start.elapsed()
        );

        Ok(StepOutput::from_rows(rows, start.elapsed()))
    }

    fn validate(&self, step: &Step) -> Result<()> {
        if step.source.is_none() {
            return Err(Error::pack(format!(
                "File step '{}' must have 'source' configuration",
                step.name
            )));
        }

        if let Some(source) = &step.source {
            if source.path.trim().is_empty() {
                return Err(Error::pack(format!(
                    "File step '{}' has empty path",
                    step.name
                )));
            }
        }

        if step.query.as_ref().map_or(false, |q| !q.trim().is_empty()) {
            return Err(Error::pack(format!(
                "File step '{}' should not have a 'query' field",
                step.name
            )));
        }

        if step.request.is_some() {
            return Err(Error::pack(format!(
                "File step '{}' should not have 'request' configuration",
                step.name
            )));
        }

        Ok(())
    }
}

/// Detect file format from extension
fn detect_format(path: &str) -> FileFormat {
    let path = Path::new(path);
    match path.extension().and_then(|e| e.to_str()) {
        Some("csv") => FileFormat::Csv,
        Some("json") => FileFormat::Json,
        Some("yaml") | Some("yml") => FileFormat::Yaml,
        _ => FileFormat::Json, // Default to JSON
    }
}

/// Parse CSV content to JSON rows
fn parse_csv(
    content: &str,
    options: Option<&crate::pack::CsvOptions>,
    step_name: &str,
) -> Result<Vec<JsonValue>> {
    let delimiter = options
        .and_then(|o| o.delimiter)
        .unwrap_or(',') as u8;
    let has_header = options.and_then(|o| o.has_header).unwrap_or(true);

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(has_header)
        .from_reader(content.as_bytes());

    let headers: Vec<String> = if has_header {
        reader
            .headers()
            .map_err(|e| {
                Error::investigation(step_name, format!("Failed to read CSV headers: {}", e))
            })?
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        // Generate column names: col_0, col_1, etc.
        let first_record = reader.records().next();
        if let Some(Ok(record)) = first_record {
            (0..record.len()).map(|i| format!("col_{}", i)).collect()
        } else {
            return Ok(vec![]);
        }
    };

    // Re-read if we consumed a record for header detection
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(has_header)
        .from_reader(content.as_bytes());

    let mut rows = Vec::new();
    for result in reader.records() {
        let record = result.map_err(|e| {
            Error::investigation(step_name, format!("Failed to parse CSV record: {}", e))
        })?;

        let mut obj = serde_json::Map::new();
        for (i, value) in record.iter().enumerate() {
            if let Some(header) = headers.get(i) {
                // Try to parse as number or boolean, fallback to string
                let json_value = parse_csv_value(value);
                obj.insert(header.clone(), json_value);
            }
        }
        rows.push(JsonValue::Object(obj));
    }

    Ok(rows)
}

/// Parse a CSV value, attempting type inference
fn parse_csv_value(value: &str) -> JsonValue {
    let trimmed = value.trim();

    // Empty string
    if trimmed.is_empty() {
        return JsonValue::Null;
    }

    // Boolean
    if trimmed.eq_ignore_ascii_case("true") {
        return JsonValue::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return JsonValue::Bool(false);
    }

    // Integer
    if let Ok(n) = trimmed.parse::<i64>() {
        return JsonValue::Number(n.into());
    }

    // Float
    if let Ok(n) = trimmed.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return JsonValue::Number(num);
        }
    }

    // String (default)
    JsonValue::String(value.to_string())
}

/// Parse JSON content to rows
fn parse_json(content: &str, step_name: &str) -> Result<Vec<JsonValue>> {
    let value: JsonValue = serde_json::from_str(content).map_err(|e| {
        Error::investigation(step_name, format!("Failed to parse JSON: {}", e))
    })?;

    match value {
        // Array of objects -> each object is a row
        JsonValue::Array(arr) => Ok(arr),
        // Single object -> single row
        obj @ JsonValue::Object(_) => Ok(vec![obj]),
        // Other values -> wrap in object
        other => Ok(vec![serde_json::json!({ "value": other })]),
    }
}

/// Parse YAML content to rows
fn parse_yaml(content: &str, step_name: &str) -> Result<Vec<JsonValue>> {
    let value: serde_yaml::Value = serde_yaml::from_str(content).map_err(|e| {
        Error::investigation(step_name, format!("Failed to parse YAML: {}", e))
    })?;

    // Convert YAML value to JSON value
    let json_value = yaml_to_json(value);

    match json_value {
        // Array -> each element is a row
        JsonValue::Array(arr) => Ok(arr),
        // Single object -> single row
        obj @ JsonValue::Object(_) => Ok(vec![obj]),
        // Other values -> wrap in object
        other => Ok(vec![serde_json::json!({ "value": other })]),
    }
}

/// Convert YAML value to JSON value
fn yaml_to_json(yaml: serde_yaml::Value) -> JsonValue {
    match yaml {
        serde_yaml::Value::Null => JsonValue::Null,
        serde_yaml::Value::Bool(b) => JsonValue::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                JsonValue::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null)
            } else {
                JsonValue::Null
            }
        }
        serde_yaml::Value::String(s) => JsonValue::String(s),
        serde_yaml::Value::Sequence(seq) => {
            JsonValue::Array(seq.into_iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let obj: serde_json::Map<String, JsonValue> = map
                .into_iter()
                .filter_map(|(k, v)| {
                    let key = match k {
                        serde_yaml::Value::String(s) => s,
                        serde_yaml::Value::Number(n) => n.to_string(),
                        serde_yaml::Value::Bool(b) => b.to_string(),
                        _ => return None,
                    };
                    Some((key, yaml_to_json(v)))
                })
                .collect();
            JsonValue::Object(obj)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(tagged.value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_format() {
        assert_eq!(detect_format("data.csv"), FileFormat::Csv);
        assert_eq!(detect_format("data.json"), FileFormat::Json);
        assert_eq!(detect_format("data.yaml"), FileFormat::Yaml);
        assert_eq!(detect_format("data.yml"), FileFormat::Yaml);
        assert_eq!(detect_format("data.txt"), FileFormat::Json); // Default
        assert_eq!(detect_format("/path/to/file.csv"), FileFormat::Csv);
    }

    #[test]
    fn test_parse_csv_simple() {
        let csv = "name,age,active\nAlice,30,true\nBob,25,false";
        let rows = parse_csv(csv, None, "test").unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["name"], "Alice");
        assert_eq!(rows[0]["age"], 30);
        assert_eq!(rows[0]["active"], true);
        assert_eq!(rows[1]["name"], "Bob");
        assert_eq!(rows[1]["age"], 25);
        assert_eq!(rows[1]["active"], false);
    }

    #[test]
    fn test_parse_csv_custom_delimiter() {
        let csv = "name;age\nAlice;30";
        let options = crate::pack::CsvOptions {
            delimiter: Some(';'),
            has_header: Some(true),
        };
        let rows = parse_csv(csv, Some(&options), "test").unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], "Alice");
        assert_eq!(rows[0]["age"], 30);
    }

    #[test]
    fn test_parse_csv_value_types() {
        assert_eq!(parse_csv_value("hello"), JsonValue::String("hello".into()));
        assert_eq!(parse_csv_value("42"), JsonValue::Number(42.into()));
        assert_eq!(parse_csv_value("3.14"), JsonValue::Number(serde_json::Number::from_f64(3.14).unwrap()));
        assert_eq!(parse_csv_value("true"), JsonValue::Bool(true));
        assert_eq!(parse_csv_value("FALSE"), JsonValue::Bool(false));
        assert_eq!(parse_csv_value(""), JsonValue::Null);
    }

    #[test]
    fn test_parse_json_array() {
        let json = r#"[{"id": 1}, {"id": 2}]"#;
        let rows = parse_json(json, "test").unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], 1);
        assert_eq!(rows[1]["id"], 2);
    }

    #[test]
    fn test_parse_json_object() {
        let json = r#"{"name": "test", "value": 42}"#;
        let rows = parse_json(json, "test").unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], "test");
        assert_eq!(rows[0]["value"], 42);
    }

    #[test]
    fn test_parse_yaml_array() {
        let yaml = "- id: 1\n- id: 2";
        let rows = parse_yaml(yaml, "test").unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], 1);
        assert_eq!(rows[1]["id"], 2);
    }

    #[test]
    fn test_parse_yaml_object() {
        let yaml = "name: test\nvalue: 42";
        let rows = parse_yaml(yaml, "test").unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], "test");
        assert_eq!(rows[0]["value"], 42);
    }

    #[test]
    fn test_yaml_to_json_types() {
        let yaml: serde_yaml::Value = serde_yaml::from_str("key: value").unwrap();
        let json = yaml_to_json(yaml);
        assert_eq!(json["key"], "value");

        let yaml: serde_yaml::Value = serde_yaml::from_str("num: 42").unwrap();
        let json = yaml_to_json(yaml);
        assert_eq!(json["num"], 42);

        let yaml: serde_yaml::Value = serde_yaml::from_str("flag: true").unwrap();
        let json = yaml_to_json(yaml);
        assert_eq!(json["flag"], true);
    }
}
