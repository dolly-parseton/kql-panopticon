//! Step execution abstraction
//!
//! Provides a unified interface for executing different step types (KQL, HTTP, File, etc.)
//! Each step type produces normalized output as JSON rows for downstream consumption.

use crate::error::Result;
use crate::pack::{Step, StepType};
use crate::variable::SubstitutionContext;
use crate::workspace::Workspace;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::Path;
use std::time::Duration;

/// Context provided to step handlers during execution
pub struct StepContext<'a> {
    /// Variable substitution context (inputs, previous step results, secrets)
    pub substitution: &'a SubstitutionContext,

    /// Target workspace (required for KQL steps, None for others)
    pub workspace: Option<&'a Workspace>,

    /// Output directory for this execution
    pub output_dir: &'a Path,

    /// Step execution timeout
    pub timeout: Duration,
}

/// Result from executing a step
#[derive(Debug, Clone)]
pub struct StepOutput {
    /// Rows as JSON objects - uniform format for all step types
    ///
    /// - KQL: Query result rows mapped to `{column: value}` objects
    /// - HTTP: Response fields extracted per config
    /// - File: Parsed rows from CSV/JSON (future)
    pub rows: Vec<JsonValue>,

    /// Row count (convenience, same as `rows.len()`)
    pub row_count: usize,

    /// Execution duration
    pub duration: Duration,

    /// Optional raw response for debugging/tracing
    pub raw: Option<JsonValue>,
}

impl StepOutput {
    /// Create empty output (e.g., for skipped steps)
    pub fn empty(duration: Duration) -> Self {
        Self {
            rows: vec![],
            row_count: 0,
            duration,
            raw: None,
        }
    }

    /// Create output from rows
    pub fn from_rows(rows: Vec<JsonValue>, duration: Duration) -> Self {
        let row_count = rows.len();
        Self {
            rows,
            row_count,
            duration,
            raw: None,
        }
    }

    /// Create output with raw response attached
    pub fn with_raw(mut self, raw: JsonValue) -> Self {
        self.raw = Some(raw);
        self
    }

    /// Check if output has any rows
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// Internal trait for step execution handlers
///
/// Each step type (KQL, HTTP, File) implements this trait.
/// The PackExecutor coordinates execution and passes results
/// to the substitution context for downstream steps.
#[async_trait]
pub(crate) trait StepHandler: Send + Sync {
    /// Which step type this handler processes
    fn step_type(&self) -> StepType;

    /// Execute the step and return normalized rows
    ///
    /// The handler should:
    /// 1. Perform variable substitution on relevant fields
    /// 2. Execute the step (query, HTTP call, file read, etc.)
    /// 3. Convert results to uniform `Vec<JsonValue>` format
    async fn execute(&self, step: &Step, ctx: &StepContext<'_>) -> Result<StepOutput>;

    /// Validate step configuration
    ///
    /// Called during pack validation to catch configuration errors early.
    fn validate(&self, step: &Step) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_output_empty() {
        let output = StepOutput::empty(Duration::from_millis(100));
        assert!(output.is_empty());
        assert_eq!(output.row_count, 0);
    }

    #[test]
    fn test_step_output_from_rows() {
        let rows = vec![
            serde_json::json!({"id": 1, "name": "Alice"}),
            serde_json::json!({"id": 2, "name": "Bob"}),
        ];
        let output = StepOutput::from_rows(rows, Duration::from_millis(50));
        assert!(!output.is_empty());
        assert_eq!(output.row_count, 2);
    }

    #[test]
    fn test_step_output_with_raw() {
        let rows = vec![serde_json::json!({"value": 42})];
        let raw = serde_json::json!({"full_response": "data"});
        let output = StepOutput::from_rows(rows, Duration::from_millis(10)).with_raw(raw.clone());
        assert_eq!(output.raw, Some(raw));
    }
}
