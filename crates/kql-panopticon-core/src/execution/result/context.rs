//! Result context for managing step result handles
//!
//! Provides a container for all step results in an execution.

use crate::error::Result;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

use super::handle::ResultHandle;

/// Context holding all step result handles for an execution
///
/// Each step's results are stored as a JSONL file, accessed via handles.
/// This provides lazy, memory-efficient access to results.
#[derive(Debug, Default, Clone)]
pub struct ResultContext {
    handles: HashMap<String, ResultHandle>,
}

impl ResultContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a result handle for a step
    pub fn insert(&mut self, step_name: impl Into<String>, handle: ResultHandle) {
        self.handles.insert(step_name.into(), handle);
    }

    /// Get handle for a step
    pub fn get(&self, step_name: &str) -> Option<&ResultHandle> {
        self.handles.get(step_name)
    }

    /// Check if a step exists in the context
    pub fn contains(&self, step_name: &str) -> bool {
        self.handles.contains_key(step_name)
    }

    /// Check if step has results (exists and not empty)
    pub fn has_results(&self, step_name: &str) -> bool {
        self.handles
            .get(step_name)
            .map(|h| h.exists() && !h.is_empty().unwrap_or(true))
            .unwrap_or(false)
    }

    /// Check if step results are empty
    pub fn is_empty(&self, step_name: &str) -> Result<bool> {
        match self.handles.get(step_name) {
            Some(h) => h.is_empty(),
            None => Ok(true),
        }
    }

    /// Get row count for a step
    pub fn row_count(&self, step_name: &str) -> Result<usize> {
        match self.handles.get(step_name) {
            Some(h) => h.row_count(),
            None => Ok(0),
        }
    }

    /// Get column values for substitution
    ///
    /// Returns all values from a single column across all rows.
    pub fn column_values(&self, step_name: &str, column: &str) -> Result<Vec<String>> {
        match self.handles.get(step_name) {
            Some(h) => h.column_values(column),
            None => Ok(vec![]),
        }
    }

    /// Get first N rows for a step
    pub fn first_n(&self, step_name: &str, n: usize) -> Result<Vec<JsonValue>> {
        match self.handles.get(step_name) {
            Some(h) => h.first_n(n),
            None => Ok(vec![]),
        }
    }

    /// Get the first row for a step
    pub fn first(&self, step_name: &str) -> Result<Option<JsonValue>> {
        Ok(self.first_n(step_name, 1)?.into_iter().next())
    }

    /// Get a specific row by index
    pub fn get_row(&self, step_name: &str, index: usize) -> Result<Option<JsonValue>> {
        match self.handles.get(step_name) {
            Some(h) => {
                // Read index+1 rows and return the last one
                let rows = h.first_n(index + 1)?;
                Ok(rows.into_iter().nth(index))
            }
            None => Ok(None),
        }
    }

    /// Materialize all rows for a step
    ///
    /// WARNING: Loads all data into memory. Use sparingly.
    pub fn materialize(&self, step_name: &str) -> Result<Vec<JsonValue>> {
        match self.handles.get(step_name) {
            Some(h) => h.materialize(),
            None => Ok(vec![]),
        }
    }

    /// Iterate over all step names
    pub fn step_names(&self) -> impl Iterator<Item = &str> {
        self.handles.keys().map(|s| s.as_str())
    }

    /// Get number of steps in context
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// Check if context has no steps
    pub fn is_context_empty(&self) -> bool {
        self.handles.is_empty()
    }

    /// Remove a step from the context
    pub fn remove(&mut self, step_name: &str) -> Option<ResultHandle> {
        self.handles.remove(step_name)
    }

    /// Clear all step handles
    pub fn clear(&mut self) {
        self.handles.clear();
    }

    /// Get all handles as a HashMap reference
    pub fn handles(&self) -> &HashMap<String, ResultHandle> {
        &self.handles
    }

    /// Merge another context into this one
    ///
    /// Handles from `other` will overwrite existing handles with the same name.
    pub fn merge(&mut self, other: ResultContext) {
        self.handles.extend(other.handles);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::result::ResultWriter;
    use std::fs;
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("kql-panopticon-test")
            .join("context")
            .join(name)
    }

    fn cleanup(path: &PathBuf) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_empty_context() {
        let ctx = ResultContext::new();
        assert!(ctx.is_context_empty());
        assert_eq!(ctx.len(), 0);
        assert!(!ctx.contains("nonexistent"));
        assert!(ctx.is_empty("nonexistent").unwrap());
    }

    #[test]
    fn test_insert_and_get() {
        let path = temp_path("context_test.jsonl");
        cleanup(&path);

        let mut writer = ResultWriter::new(&path, "test_step").unwrap();
        writer
            .write_rows(&[
                serde_json::json!({"id": 1, "name": "Alice"}),
                serde_json::json!({"id": 2, "name": "Bob"}),
            ])
            .unwrap();
        let handle = writer.finish().unwrap();

        let mut ctx = ResultContext::new();
        ctx.insert("test_step", handle);

        assert!(ctx.contains("test_step"));
        assert!(!ctx.is_empty("test_step").unwrap());
        assert_eq!(ctx.row_count("test_step").unwrap(), 2);

        let names = ctx.column_values("test_step", "name").unwrap();
        assert_eq!(names, vec!["Alice", "Bob"]);

        let first = ctx.first("test_step").unwrap();
        assert!(first.is_some());
        assert_eq!(first.unwrap()["name"], "Alice");

        cleanup(&path);
    }

    #[test]
    fn test_get_row() {
        let path = temp_path("get_row_test.jsonl");
        cleanup(&path);

        let mut writer = ResultWriter::new(&path, "test_step").unwrap();
        writer
            .write_rows(&[
                serde_json::json!({"index": 0}),
                serde_json::json!({"index": 1}),
                serde_json::json!({"index": 2}),
            ])
            .unwrap();
        let handle = writer.finish().unwrap();

        let mut ctx = ResultContext::new();
        ctx.insert("test_step", handle);

        assert_eq!(ctx.get_row("test_step", 0).unwrap().unwrap()["index"], 0);
        assert_eq!(ctx.get_row("test_step", 1).unwrap().unwrap()["index"], 1);
        assert_eq!(ctx.get_row("test_step", 2).unwrap().unwrap()["index"], 2);
        assert!(ctx.get_row("test_step", 3).unwrap().is_none());

        cleanup(&path);
    }

    #[test]
    fn test_merge() {
        let path1 = temp_path("merge1.jsonl");
        let path2 = temp_path("merge2.jsonl");
        cleanup(&path1);
        cleanup(&path2);

        let mut writer1 = ResultWriter::new(&path1, "step1").unwrap();
        writer1.write_row(&serde_json::json!({"x": 1})).unwrap();
        let handle1 = writer1.finish().unwrap();

        let mut writer2 = ResultWriter::new(&path2, "step2").unwrap();
        writer2.write_row(&serde_json::json!({"x": 2})).unwrap();
        let handle2 = writer2.finish().unwrap();

        let mut ctx1 = ResultContext::new();
        ctx1.insert("step1", handle1);

        let mut ctx2 = ResultContext::new();
        ctx2.insert("step2", handle2);

        ctx1.merge(ctx2);

        assert_eq!(ctx1.len(), 2);
        assert!(ctx1.contains("step1"));
        assert!(ctx1.contains("step2"));

        cleanup(&path1);
        cleanup(&path2);
    }
}
