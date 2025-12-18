//! Result writer for streaming step results to disk
//!
//! Writes rows one at a time to a JSONL file, avoiding memory buildup.

use crate::error::{Error, Result};
use serde_json::Value as JsonValue;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use super::handle::ResultHandle;

/// Writer for streaming results to JSONL
///
/// Writes rows one at a time to a JSONL file, avoiding memory buildup
/// for large result sets.
pub struct ResultWriter {
    path: PathBuf,
    step_name: String,
    writer: Option<BufWriter<std::fs::File>>,
    row_count: usize,
}

impl ResultWriter {
    /// Create a new writer
    ///
    /// Creates the parent directory if it doesn't exist.
    pub fn new(path: impl Into<PathBuf>, step_name: impl Into<String>) -> Result<Self> {
        let path = path.into();
        let step_name = step_name.into();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::execution(format!(
                    "Failed to create output directory for step '{}': {}",
                    step_name, e
                ))
            })?;
        }

        let file = std::fs::File::create(&path).map_err(|e| {
            Error::execution(format!(
                "Failed to create result file for step '{}': {}",
                step_name, e
            ))
        })?;

        Ok(Self {
            path,
            step_name,
            writer: Some(BufWriter::new(file)),
            row_count: 0,
        })
    }

    /// Create a writer for a step in the given output directory
    pub fn for_step(output_dir: &Path, step_name: &str) -> Result<Self> {
        let path = output_dir.join(format!("{}.jsonl", step_name));
        Self::new(path, step_name)
    }

    /// Get the output path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write a single row
    pub fn write_row(&mut self, row: &JsonValue) -> Result<()> {
        if let Some(ref mut writer) = self.writer {
            serde_json::to_writer(&mut *writer, row).map_err(|e| {
                Error::execution(format!(
                    "Failed to write row for step '{}': {}",
                    self.step_name, e
                ))
            })?;

            writeln!(writer).map_err(|e| {
                Error::execution(format!(
                    "Failed to write newline for step '{}': {}",
                    self.step_name, e
                ))
            })?;

            self.row_count += 1;
        }
        Ok(())
    }

    /// Write multiple rows
    pub fn write_rows(&mut self, rows: &[JsonValue]) -> Result<()> {
        for row in rows {
            self.write_row(row)?;
        }
        Ok(())
    }

    /// Get current row count
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    /// Flush the buffer without finishing
    pub fn flush(&mut self) -> Result<()> {
        if let Some(ref mut writer) = self.writer {
            writer.flush().map_err(|e| {
                Error::execution(format!(
                    "Failed to flush result file for step '{}': {}",
                    self.step_name, e
                ))
            })?;
        }
        Ok(())
    }

    /// Finish writing and return a handle
    ///
    /// Flushes the buffer and returns a handle for reading.
    pub fn finish(mut self) -> Result<ResultHandle> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush().map_err(|e| {
                Error::execution(format!(
                    "Failed to flush result file for step '{}': {}",
                    self.step_name, e
                ))
            })?;
        }

        Ok(ResultHandle::with_row_count(
            self.path,
            self.step_name,
            self.row_count,
        ))
    }

    /// Finish writing and return an empty handle if no rows were written
    pub fn finish_or_empty(self) -> Result<ResultHandle> {
        if self.row_count == 0 {
            Ok(ResultHandle::empty(&self.step_name))
        } else {
            self.finish()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("kql-panopticon-test")
            .join("writer")
            .join(name)
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_write_single_row() {
        let path = temp_path("single.jsonl");
        cleanup(&path);

        let mut writer = ResultWriter::new(&path, "test").unwrap();
        writer
            .write_row(&serde_json::json!({"id": 1, "name": "Alice"}))
            .unwrap();
        let handle = writer.finish().unwrap();

        assert_eq!(handle.row_count().unwrap(), 1);
        let rows = handle.materialize().unwrap();
        assert_eq!(rows[0]["name"], "Alice");

        cleanup(&path);
    }

    #[test]
    fn test_write_multiple_rows() {
        let path = temp_path("multiple.jsonl");
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

        assert_eq!(handle.row_count().unwrap(), 3);

        cleanup(&path);
    }

    #[test]
    fn test_for_step() {
        let dir = std::env::temp_dir().join("kql-panopticon-test").join("for_step");
        let _ = fs::create_dir_all(&dir);

        let mut writer = ResultWriter::for_step(&dir, "my_step").unwrap();
        assert!(writer.path().ends_with("my_step.jsonl"));

        writer.write_row(&serde_json::json!({"x": 1})).unwrap();
        let handle = writer.finish().unwrap();
        assert_eq!(handle.row_count().unwrap(), 1);

        let _ = fs::remove_file(handle.path());
    }

    #[test]
    fn test_finish_or_empty() {
        let path = temp_path("empty_finish.jsonl");
        cleanup(&path);

        let writer = ResultWriter::new(&path, "test").unwrap();
        let handle = writer.finish_or_empty().unwrap();

        // Should return empty handle since no rows written
        assert!(!handle.exists());
        assert_eq!(handle.row_count().unwrap(), 0);

        cleanup(&path);
    }
}
