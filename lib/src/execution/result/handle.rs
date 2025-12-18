//! Result handle for file-backed step results
//!
//! Provides lazy access to JSONL files for memory-efficient result handling.
//! Uses Polars LazyFrame for efficient columnar operations and query pushdown.

use crate::error::{Error, Result};
use polars::prelude::*;
use serde_json::Value as JsonValue;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Handle to step results stored as JSONL on disk
///
/// Provides lazy access to results without loading into memory.
/// Results are stored as newline-delimited JSON (JSONL) files.
#[derive(Debug, Clone)]
pub struct ResultHandle {
    /// Path to the JSONL file
    path: PathBuf,
    /// Cached row count (populated after write or first access)
    row_count: Option<usize>,
    /// Step name for error context
    step_name: String,
}

impl ResultHandle {
    /// Create handle for an existing file
    pub fn from_path(path: impl Into<PathBuf>, step_name: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            row_count: None,
            step_name: step_name.into(),
        }
    }

    /// Create handle with known row count
    pub fn with_row_count(
        path: impl Into<PathBuf>,
        step_name: impl Into<String>,
        row_count: usize,
    ) -> Self {
        Self {
            path: path.into(),
            row_count: Some(row_count),
            step_name: step_name.into(),
        }
    }

    /// Create an empty handle (no file, zero rows)
    pub fn empty(step_name: impl Into<String>) -> Self {
        Self {
            path: PathBuf::new(),
            row_count: Some(0),
            step_name: step_name.into(),
        }
    }

    /// Get path to the JSONL file
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get step name
    pub fn step_name(&self) -> &str {
        &self.step_name
    }

    /// Check if results file exists
    pub fn exists(&self) -> bool {
        !self.path.as_os_str().is_empty() && self.path.exists()
    }

    /// Check if results are empty
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.row_count()? == 0)
    }

    /// Get row count (cached after first access)
    pub fn row_count(&self) -> Result<usize> {
        if let Some(count) = self.row_count {
            return Ok(count);
        }

        if !self.exists() {
            return Ok(0);
        }

        // Count lines in JSONL file
        let file = std::fs::File::open(&self.path).map_err(|e| {
            Error::execution(format!(
                "Failed to open result file for step '{}': {}",
                self.step_name, e
            ))
        })?;

        let count = BufReader::new(file).lines().count();
        Ok(count)
    }

    /// Get a LazyFrame for this result set
    ///
    /// Uses Polars for efficient lazy evaluation of queries. The LazyFrame
    /// enables columnar operations, query optimization, and memory-efficient
    /// processing of large result sets.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let lf = handle.lazy_frame()?;
    /// let high_severity = lf
    ///     .filter(col("severity").eq(lit("High")))
    ///     .select([col("timestamp"), col("message")])
    ///     .collect()?;
    /// ```
    pub fn lazy_frame(&self) -> Result<LazyFrame> {
        if !self.exists() {
            return Err(Error::execution(format!(
                "Result file not found for step '{}': {:?}",
                self.step_name, self.path
            )));
        }

        LazyJsonLineReader::new(&self.path)
            .finish()
            .map_err(|e| {
                Error::execution(format!(
                    "Failed to read JSONL for step '{}': {}",
                    self.step_name, e
                ))
            })
    }

    /// Get first N rows as JsonValue
    ///
    /// Useful for variable substitution and template rendering.
    pub fn first_n(&self, n: usize) -> Result<Vec<JsonValue>> {
        if !self.exists() || n == 0 {
            return Ok(vec![]);
        }

        let file = std::fs::File::open(&self.path).map_err(|e| {
            Error::execution(format!(
                "Failed to open result file for step '{}': {}",
                self.step_name, e
            ))
        })?;

        let reader = BufReader::new(file);
        let mut rows = Vec::with_capacity(n);

        for line in reader.lines().take(n) {
            let line = line.map_err(|e| {
                Error::execution(format!(
                    "Failed to read line from step '{}': {}",
                    self.step_name, e
                ))
            })?;

            let value: JsonValue = serde_json::from_str(&line).map_err(|e| {
                Error::execution(format!(
                    "Failed to parse JSON in step '{}': {}",
                    self.step_name, e
                ))
            })?;

            rows.push(value);
        }

        Ok(rows)
    }

    /// Get single column values as strings
    ///
    /// Extracts all values from a single column, useful for variable substitution.
    pub fn column_values(&self, column: &str) -> Result<Vec<String>> {
        if !self.exists() {
            return Ok(vec![]);
        }

        let file = std::fs::File::open(&self.path).map_err(|e| {
            Error::execution(format!(
                "Failed to open result file for step '{}': {}",
                self.step_name, e
            ))
        })?;

        let reader = BufReader::new(file);
        let mut values = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| {
                Error::execution(format!(
                    "Failed to read line from step '{}': {}",
                    self.step_name, e
                ))
            })?;

            let row: JsonValue = serde_json::from_str(&line).map_err(|e| {
                Error::execution(format!(
                    "Failed to parse JSON in step '{}': {}",
                    self.step_name, e
                ))
            })?;

            if let Some(value) = row.get(column) {
                let s = json_value_to_string(value);
                if !s.is_empty() {
                    values.push(s);
                }
            }
        }

        Ok(values)
    }

    /// Iterate rows without loading all into memory
    ///
    /// Returns an iterator that reads one row at a time from the file.
    pub fn iter_rows(&self) -> Result<RowIterator> {
        RowIterator::new(&self.path, &self.step_name)
    }

    /// Materialize all rows to Vec<JsonValue>
    ///
    /// WARNING: Loads all data into memory. Use sparingly for large datasets.
    pub fn materialize(&self) -> Result<Vec<JsonValue>> {
        if !self.exists() {
            return Ok(vec![]);
        }

        let file = std::fs::File::open(&self.path).map_err(|e| {
            Error::execution(format!(
                "Failed to open result file for step '{}': {}",
                self.step_name, e
            ))
        })?;

        let reader = BufReader::new(file);
        let mut rows = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| {
                Error::execution(format!(
                    "Failed to read line from step '{}': {}",
                    self.step_name, e
                ))
            })?;

            let value: JsonValue = serde_json::from_str(&line).map_err(|e| {
                Error::execution(format!(
                    "Failed to parse JSON in step '{}': {}",
                    self.step_name, e
                ))
            })?;

            rows.push(value);
        }

        Ok(rows)
    }
}

/// Iterator over rows in a JSONL file
///
/// Reads one row at a time, keeping memory usage low.
pub struct RowIterator {
    reader: BufReader<std::fs::File>,
    step_name: String,
    line_buffer: String,
}

impl RowIterator {
    pub(crate) fn new(path: &Path, step_name: &str) -> Result<Self> {
        if !path.exists() {
            return Err(Error::execution(format!(
                "Result file not found for step '{}': {:?}",
                step_name, path
            )));
        }

        let file = std::fs::File::open(path).map_err(|e| {
            Error::execution(format!(
                "Failed to open result file for step '{}': {}",
                step_name, e
            ))
        })?;

        Ok(Self {
            reader: BufReader::new(file),
            step_name: step_name.to_string(),
            line_buffer: String::new(),
        })
    }
}

impl Iterator for RowIterator {
    type Item = Result<JsonValue>;

    fn next(&mut self) -> Option<Self::Item> {
        self.line_buffer.clear();

        match self.reader.read_line(&mut self.line_buffer) {
            Ok(0) => None, // EOF
            Ok(_) => {
                let trimmed = self.line_buffer.trim();
                if trimmed.is_empty() {
                    return self.next(); // Skip empty lines
                }

                Some(serde_json::from_str(trimmed).map_err(|e| {
                    Error::execution(format!(
                        "Failed to parse JSON in step '{}': {}",
                        self.step_name, e
                    ))
                }))
            }
            Err(e) => Some(Err(Error::execution(format!(
                "Failed to read line from step '{}': {}",
                self.step_name, e
            )))),
        }
    }
}

/// Convert JsonValue to string representation
fn json_value_to_string(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => String::new(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::String(s) => s.clone(),
        JsonValue::Array(_) | JsonValue::Object(_) => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::result::ResultWriter;
    use std::fs;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("kql-panopticon-test")
            .join("handle")
            .join(name)
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_empty_handle() {
        let handle = ResultHandle::empty("empty_step");
        assert!(!handle.exists());
        assert!(handle.is_empty().unwrap());
        assert_eq!(handle.row_count().unwrap(), 0);
        assert!(handle.materialize().unwrap().is_empty());
    }

    #[test]
    fn test_first_n() {
        let path = temp_path("first_n.jsonl");
        cleanup(&path);

        let mut writer = ResultWriter::new(&path, "test").unwrap();
        writer
            .write_rows(&[
                serde_json::json!({"id": 1}),
                serde_json::json!({"id": 2}),
                serde_json::json!({"id": 3}),
            ])
            .unwrap();
        let handle = writer.finish().unwrap();

        let first_two = handle.first_n(2).unwrap();
        assert_eq!(first_two.len(), 2);
        assert_eq!(first_two[0]["id"], 1);
        assert_eq!(first_two[1]["id"], 2);

        cleanup(&path);
    }

    #[test]
    fn test_column_values() {
        let path = temp_path("column_values.jsonl");
        cleanup(&path);

        let mut writer = ResultWriter::new(&path, "test").unwrap();
        writer
            .write_rows(&[
                serde_json::json!({"name": "Alice"}),
                serde_json::json!({"name": "Bob"}),
            ])
            .unwrap();
        let handle = writer.finish().unwrap();

        let names = handle.column_values("name").unwrap();
        assert_eq!(names, vec!["Alice", "Bob"]);

        cleanup(&path);
    }

    #[test]
    fn test_row_iterator() {
        let path = temp_path("iterator.jsonl");
        cleanup(&path);

        let mut writer = ResultWriter::new(&path, "test").unwrap();
        for i in 1..=3 {
            writer.write_row(&serde_json::json!({"n": i})).unwrap();
        }
        let handle = writer.finish().unwrap();

        let collected: Vec<_> = handle
            .iter_rows()
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0]["n"], 1);
        assert_eq!(collected[2]["n"], 3);

        cleanup(&path);
    }

    #[test]
    fn test_lazy_frame() {
        let path = temp_path("lazy_frame.jsonl");
        cleanup(&path);

        let mut writer = ResultWriter::new(&path, "test").unwrap();
        writer
            .write_rows(&[
                serde_json::json!({"value": 10, "category": "A"}),
                serde_json::json!({"value": 20, "category": "B"}),
                serde_json::json!({"value": 30, "category": "A"}),
            ])
            .unwrap();
        let handle = writer.finish().unwrap();

        // Test basic LazyFrame access
        let lf = handle.lazy_frame().unwrap();
        let df = lf.collect().unwrap();

        assert_eq!(df.height(), 3);
        assert!(df.column("value").is_ok());
        assert!(df.column("category").is_ok());

        // Test filtering with LazyFrame
        let lf = handle.lazy_frame().unwrap();
        let filtered = lf
            .filter(col("category").eq(lit("A")))
            .collect()
            .unwrap();

        assert_eq!(filtered.height(), 2);

        cleanup(&path);
    }

    #[test]
    fn test_lazy_frame_empty_file_error() {
        let handle = ResultHandle::empty("empty_step");
        let result = handle.lazy_frame();
        assert!(result.is_err());
    }
}
