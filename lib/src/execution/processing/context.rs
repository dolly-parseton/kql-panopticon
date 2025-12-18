//! Processing phase execution context
//!
//! Provides read access to acquisition results and write capabilities for processing steps.

use crate::error::Result;
use crate::execution::result::{ResultContext, ResultHandle, ResultWriter};
use serde_json::Value as JsonValue;
use std::path::Path;

/// Context for processing phase execution
///
/// Provides:
/// - Read access to acquisition results
/// - Write capabilities for processing step outputs
/// - Result accumulation for downstream phases
pub struct ProcessingContext<'a> {
    /// Acquisition step results (read-only)
    acquisition_results: &'a ResultContext,

    /// Output directory for processing result files
    output_dir: &'a Path,

    /// Accumulated processing step results
    results: ResultContext,
}

impl<'a> ProcessingContext<'a> {
    /// Create a new processing context
    pub fn new(acquisition_results: &'a ResultContext, output_dir: &'a Path) -> Self {
        Self {
            acquisition_results,
            output_dir,
            results: ResultContext::new(),
        }
    }

    // =========================================================================
    // Output and Writing
    // =========================================================================

    /// Get the output directory
    pub fn output_dir(&self) -> &Path {
        self.output_dir
    }

    /// Create a result writer for a processing step
    pub fn writer(&self, step_name: &str) -> Result<ResultWriter> {
        ResultWriter::for_step(self.output_dir, step_name)
    }

    /// Register a completed step's result handle
    pub fn register_result(&mut self, step_name: impl Into<String>, handle: ResultHandle) {
        self.results.insert(step_name, handle);
    }

    /// Get the accumulated processing results
    pub fn results(&self) -> &ResultContext {
        &self.results
    }

    /// Take ownership of the accumulated processing results
    ///
    /// Called at the end of the processing phase.
    pub fn take_results(self) -> ResultContext {
        self.results
    }

    // =========================================================================
    // Acquisition Results Access (read-only)
    // =========================================================================

    /// Get the acquisition results context
    pub fn acquisition_results(&self) -> &ResultContext {
        self.acquisition_results
    }

    /// Get all rows for an acquisition step (materializes data)
    ///
    /// WARNING: This loads all rows into memory.
    pub fn get_rows(&self, step_name: &str) -> Result<Vec<JsonValue>> {
        self.acquisition_results.materialize(step_name)
    }

    /// Get row count for an acquisition step
    pub fn row_count(&self, step_name: &str) -> Result<usize> {
        self.acquisition_results.row_count(step_name)
    }

    /// Check if acquisition step has results
    pub fn has_results(&self, step_name: &str) -> bool {
        self.acquisition_results.has_results(step_name)
    }

    /// Check if acquisition step is empty
    pub fn is_step_empty(&self, step_name: &str) -> Result<bool> {
        self.acquisition_results.is_step_empty(step_name)
    }

    /// Get first row for an acquisition step
    pub fn first(&self, step_name: &str) -> Result<Option<JsonValue>> {
        self.acquisition_results.first(step_name)
    }

    /// Get column values for an acquisition step
    pub fn column_values(&self, step_name: &str, column: &str) -> Result<Vec<String>> {
        self.acquisition_results.column_values(step_name, column)
    }
}
