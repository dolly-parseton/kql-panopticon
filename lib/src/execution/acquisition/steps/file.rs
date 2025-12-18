//! File step execution handler
//!
//! Reads local files (CSV, JSON, YAML) and normalizes to uniform row format.

use crate::error::{Error, Result};
use crate::execution::acquisition::{
    AcquisitionContext, AcquisitionStepHandler, AcquisitionStepOutput,
};
use crate::execution::result::ResultWriter;
use crate::pack::{AcquisitionStepType, FileFormat, Step};
use crate::variable::{ContextType, SubstitutionBuilder};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::Path;
use std::time::Instant;
use tracing::debug;

/// Handler for file source steps
pub struct FileStepHandler;

impl FileStepHandler {
    /// Create a new file handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileStepHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AcquisitionStepHandler for FileStepHandler {
    fn handles(&self) -> AcquisitionStepType {
        AcquisitionStepType::File
    }

    async fn execute(
        &self,
        step: &Step,
        ctx: &mut AcquisitionContext<'_>,
    ) -> Result<AcquisitionStepOutput> {
        let start = Instant::now();

        // Get source config
        let source = step.source.as_ref().ok_or_else(|| {
            Error::investigation(&step.name, "File step missing source configuration")
        })?;

        // Substitute variables in path
        let builder = SubstitutionBuilder::new(&source.path, ctx.evaluation()).map_err(|e| {
            Error::investigation(&step.name, format!("Path parsing failed: {}", e))
        })?;

        // File steps don't support for_each
        builder.validate(ContextType::KqlQuery).map_err(|e| {
            Error::investigation(&step.name, format!("Path validation failed: {}", e))
        })?;

        let path = builder.substitute().map_err(|e| {
            Error::investigation(&step.name, format!("Path substitution failed: {}", e))
        })?;

        debug!("Executing File step '{}': {}", step.name, path);

        // Determine format
        let format = source.format.unwrap_or_else(|| detect_format(&path));

        // Read file content
        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            Error::investigation(&step.name, format!("Failed to read file '{}': {}", path, e))
        })?;

        // Create JSONL writer
        let mut writer = ctx.writer(&step.name)?;

        // Parse based on format and write directly to JSONL
        match format {
            FileFormat::Csv => {
                write_csv_to_writer(&content, source.csv.as_ref(), &step.name, &mut writer)?
            }
            FileFormat::Json => write_json_to_writer(&content, &step.name, &mut writer)?,
            FileFormat::Yaml => write_yaml_to_writer(&content, &step.name, &mut writer)?,
        };

        let handle = writer.finish()?;
        let row_count = handle.row_count()?;

        debug!(
            "File step '{}' completed: {} rows in {:?}",
            step.name,
            row_count,
            start.elapsed()
        );

        Ok(AcquisitionStepOutput::new(handle, start.elapsed()))
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

        if step.query.as_ref().is_some_and(|q| !q.trim().is_empty()) {
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

/// Write CSV content directly to JSONL
fn write_csv_to_writer(
    content: &str,
    options: Option<&crate::pack::CsvOptions>,
    step_name: &str,
    writer: &mut ResultWriter,
) -> Result<()> {
    let delimiter = options.and_then(|o| o.delimiter).unwrap_or(',') as u8;
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
            return Ok(());
        }
    };

    // Re-read if we consumed a record for header detection
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(has_header)
        .from_reader(content.as_bytes());

    for result in reader.records() {
        let record = result.map_err(|e| {
            Error::investigation(step_name, format!("Failed to parse CSV record: {}", e))
        })?;

        let mut obj = serde_json::Map::new();
        for (i, value) in record.iter().enumerate() {
            if let Some(header) = headers.get(i) {
                let json_value = parse_csv_value(value);
                obj.insert(header.clone(), json_value);
            }
        }
        writer.write_row(&JsonValue::Object(obj))?;
    }

    Ok(())
}

/// Parse a CSV value, attempting type inference
fn parse_csv_value(value: &str) -> JsonValue {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return JsonValue::Null;
    }

    if trimmed.eq_ignore_ascii_case("true") {
        return JsonValue::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return JsonValue::Bool(false);
    }

    if let Ok(n) = trimmed.parse::<i64>() {
        return JsonValue::Number(n.into());
    }

    if let Ok(n) = trimmed.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return JsonValue::Number(num);
        }
    }

    JsonValue::String(value.to_string())
}

/// Write JSON content directly to JSONL
fn write_json_to_writer(
    content: &str,
    step_name: &str,
    writer: &mut ResultWriter,
) -> Result<()> {
    let value: JsonValue = serde_json::from_str(content).map_err(|e| {
        Error::investigation(step_name, format!("Failed to parse JSON: {}", e))
    })?;

    match value {
        JsonValue::Array(arr) => {
            for row in arr {
                writer.write_row(&row)?;
            }
        }
        obj @ JsonValue::Object(_) => {
            writer.write_row(&obj)?;
        }
        other => {
            writer.write_row(&serde_json::json!({ "value": other }))?;
        }
    }

    Ok(())
}

/// Write YAML content directly to JSONL
fn write_yaml_to_writer(
    content: &str,
    step_name: &str,
    writer: &mut ResultWriter,
) -> Result<()> {
    let value: serde_yaml::Value = serde_yaml::from_str(content).map_err(|e| {
        Error::investigation(step_name, format!("Failed to parse YAML: {}", e))
    })?;

    let json_value = yaml_to_json(value);

    match json_value {
        JsonValue::Array(arr) => {
            for row in arr {
                writer.write_row(&row)?;
            }
        }
        obj @ JsonValue::Object(_) => {
            writer.write_row(&obj)?;
        }
        other => {
            writer.write_row(&serde_json::json!({ "value": other }))?;
        }
    }

    Ok(())
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
        assert_eq!(detect_format("data.txt"), FileFormat::Json);
    }

    #[test]
    fn test_parse_csv_value_types() {
        assert_eq!(
            parse_csv_value("hello"),
            JsonValue::String("hello".into())
        );
        assert_eq!(parse_csv_value("42"), JsonValue::Number(42.into()));
        assert_eq!(
            parse_csv_value("3.14"),
            JsonValue::Number(serde_json::Number::from_f64(3.14).unwrap())
        );
        assert_eq!(parse_csv_value("true"), JsonValue::Bool(true));
        assert_eq!(parse_csv_value("FALSE"), JsonValue::Bool(false));
        assert_eq!(parse_csv_value(""), JsonValue::Null);
    }

    #[test]
    fn test_yaml_to_json() {
        let yaml: serde_yaml::Value = serde_yaml::from_str("key: value").unwrap();
        let json = yaml_to_json(yaml);
        assert_eq!(json["key"], "value");

        let yaml: serde_yaml::Value = serde_yaml::from_str("num: 42").unwrap();
        let json = yaml_to_json(yaml);
        assert_eq!(json["num"], 42);
    }
}
