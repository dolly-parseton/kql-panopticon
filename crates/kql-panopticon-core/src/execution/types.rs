//! Execution types for pack execution
//!
//! Contains configuration, result, and status types used by the executor.

use crate::pack::Pack;
use super::trace::ExecutionTrace;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for pack execution
#[derive(Debug, Clone)]
pub struct PackExecutorConfig {
    /// The pack to execute
    pub pack: Pack,

    /// Path the pack was loaded from
    pub pack_path: Option<PathBuf>,

    /// User-provided input values
    pub inputs: HashMap<String, String>,

    /// Output directory for results
    pub output_dir: Option<PathBuf>,

    /// Job name for file naming
    pub job_name: Option<String>,

    /// Export results as CSV
    pub export_csv: bool,

    /// Export results as JSON
    pub export_json: bool,
}

impl PackExecutorConfig {
    /// Create config from a pack
    pub fn new(pack: Pack) -> Self {
        Self {
            pack,
            pack_path: None,
            inputs: HashMap::new(),
            output_dir: None,
            job_name: None,
            export_csv: true,
            export_json: false,
        }
    }

    /// Set pack path
    pub fn with_pack_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.pack_path = Some(path.into());
        self
    }

    /// Set input values
    pub fn with_inputs(mut self, inputs: HashMap<String, String>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Add a single input
    pub fn with_input(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.inputs.insert(key.into(), value.into());
        self
    }

    /// Set output directory
    pub fn with_output_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.output_dir = Some(path.into());
        self
    }

    /// Set job name
    pub fn with_job_name(mut self, name: impl Into<String>) -> Self {
        self.job_name = Some(name.into());
        self
    }

    /// Set export formats
    pub fn with_formats(mut self, csv: bool, json: bool) -> Self {
        self.export_csv = csv;
        self.export_json = json;
        self
    }
}

/// Result of pack execution
#[derive(Debug, Clone)]
pub struct PackExecutorResult {
    /// Job ID
    pub job_id: uuid::Uuid,

    /// Pack name
    pub pack_name: String,

    /// Overall status
    pub status: ExecutionStatus,

    /// Results per workspace
    pub workspace_results: HashMap<String, WorkspaceResult>,

    /// Total duration in milliseconds
    pub duration_ms: u64,

    /// Output directory
    pub output_dir: Option<PathBuf>,

    /// Execution trace
    pub trace: Option<ExecutionTrace>,
}

impl PackExecutorResult {
    /// Check if all workspaces succeeded
    pub fn all_succeeded(&self) -> bool {
        self.workspace_results
            .values()
            .all(|r| matches!(r.status, ExecutionStatus::Success))
    }

    /// Get success count
    pub fn success_count(&self) -> usize {
        self.workspace_results
            .values()
            .filter(|r| matches!(r.status, ExecutionStatus::Success))
            .count()
    }

    /// Get failure count
    pub fn failure_count(&self) -> usize {
        self.workspace_results
            .values()
            .filter(|r| matches!(r.status, ExecutionStatus::Failed))
            .count()
    }

    /// Get total rows across all steps
    pub fn total_rows(&self) -> usize {
        self.workspace_results
            .values()
            .flat_map(|r| r.step_results.values())
            .filter_map(|s| s.row_count)
            .sum()
    }
}

/// Result for a single workspace
#[derive(Debug, Clone)]
pub struct WorkspaceResult {
    /// Workspace name
    pub workspace_name: String,

    /// Workspace ID
    pub workspace_id: String,

    /// Status
    pub status: ExecutionStatus,

    /// Per-step results
    pub step_results: HashMap<String, StepResult>,

    /// Step results as JSON rows (for substitution context)
    pub step_data: HashMap<String, Vec<JsonValue>>,

    /// Total duration
    pub duration_ms: u64,

    /// Failed step (if any)
    pub failed_step: Option<String>,

    /// Failure reason
    pub failure_reason: Option<String>,
}

/// Result for a single step
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Step name
    pub name: String,

    /// Status
    pub status: StepStatus,

    /// Row count
    pub row_count: Option<usize>,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Output file path
    pub output_path: Option<PathBuf>,

    /// Error message
    pub error: Option<String>,
}

/// Execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Partial,
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Running => write!(f, "Running"),
            Self::Success => write!(f, "Success"),
            Self::Failed => write!(f, "Failed"),
            Self::Partial => write!(f, "Partial"),
        }
    }
}

/// Step status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let pack = Pack {
            name: "test".to_string(),
            description: None,
            version: None,
            inputs: vec![],
            steps: vec![],
            output: None,
            secrets: None,
            report: None,
            scoring: None,
        };

        let config = PackExecutorConfig::new(pack)
            .with_input("key", "value")
            .with_output_dir("./out")
            .with_formats(true, true);

        assert!(config.inputs.contains_key("key"));
        assert!(config.export_csv);
        assert!(config.export_json);
    }

    #[test]
    fn test_status_display() {
        assert_eq!(ExecutionStatus::Success.to_string(), "Success");
        assert_eq!(ExecutionStatus::Failed.to_string(), "Failed");
    }
}
