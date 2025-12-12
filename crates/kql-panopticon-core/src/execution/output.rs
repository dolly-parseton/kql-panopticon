//! Output utilities for execution results
//!
//! Provides functions for writing execution results to various formats.

use crate::error::Result;
use serde_json::Value as JsonValue;
use std::path::Path;
use tokio::fs;

/// Write rows to CSV file
pub async fn write_csv_results(rows: &[JsonValue], path: &Path) -> Result<()> {
    if rows.is_empty() {
        fs::write(path, "").await?;
        return Ok(());
    }

    let mut content = String::new();

    // Get columns from first row
    let columns: Vec<&str> = rows
        .first()
        .and_then(|r| r.as_object())
        .map(|obj| obj.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();

    // Header
    content.push_str(&columns.join(","));
    content.push('\n');

    // Rows
    for row in rows {
        if let Some(obj) = row.as_object() {
            let values: Vec<String> = columns
                .iter()
                .map(|col| {
                    obj.get(*col)
                        .map(|v| format_csv_value(v))
                        .unwrap_or_default()
                })
                .collect();
            content.push_str(&values.join(","));
            content.push('\n');
        }
    }

    fs::write(path, content).await?;
    Ok(())
}

/// Format a JSON value for CSV
pub fn format_csv_value(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => String::new(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::String(s) => {
            if s.contains(',') || s.contains('"') || s.contains('\n') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.clone()
            }
        }
        JsonValue::Array(_) | JsonValue::Object(_) => {
            let json_str = value.to_string();
            format!("\"{}\"", json_str.replace('"', "\"\""))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_csv_value() {
        assert_eq!(format_csv_value(&JsonValue::Null), "");
        assert_eq!(format_csv_value(&serde_json::json!(true)), "true");
        assert_eq!(format_csv_value(&serde_json::json!(42)), "42");
        assert_eq!(format_csv_value(&serde_json::json!("hello")), "hello");
        assert_eq!(
            format_csv_value(&serde_json::json!("hello,world")),
            "\"hello,world\""
        );
    }

    #[test]
    fn test_format_csv_value_with_quotes() {
        assert_eq!(
            format_csv_value(&serde_json::json!("say \"hello\"")),
            "\"say \"\"hello\"\"\""
        );
    }

    #[test]
    fn test_format_csv_value_with_newline() {
        assert_eq!(
            format_csv_value(&serde_json::json!("line1\nline2")),
            "\"line1\nline2\""
        );
    }
}
