//! Execution history tracking
//!
//! Records what was executed during the REPL session for recall,
//! comparison, and re-execution.

use chrono::{DateTime, Utc};
use kql_panopticon_core::ExecutionStatus;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Tracks all executions in this REPL session
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExecutionHistory {
    /// All execution records
    executions: Vec<ExecutionRecord>,
}

impl ExecutionHistory {
    /// Create a new empty history
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an execution record
    pub fn add(&mut self, record: ExecutionRecord) {
        self.executions.push(record);
    }

    /// Get all executions
    pub fn all(&self) -> &[ExecutionRecord] {
        &self.executions
    }

    /// Get executions in reverse chronological order
    pub fn recent(&self) -> impl Iterator<Item = &ExecutionRecord> {
        self.executions.iter().rev()
    }

    /// Get the most recent execution
    pub fn last(&self) -> Option<&ExecutionRecord> {
        self.executions.last()
    }

    /// Get execution by ID
    pub fn get(&self, id: &Uuid) -> Option<&ExecutionRecord> {
        self.executions.iter().find(|e| &e.id == id)
    }

    /// Get execution by short ID prefix
    pub fn get_by_prefix(&self, prefix: &str) -> Option<&ExecutionRecord> {
        let prefix_lower = prefix.to_lowercase();
        self.executions
            .iter()
            .find(|e| e.id.to_string().to_lowercase().starts_with(&prefix_lower))
    }

    /// Count total executions
    pub fn len(&self) -> usize {
        self.executions.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.executions.is_empty()
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.executions.clear();
    }
}

/// Record of a single execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Unique execution ID
    pub id: Uuid,

    /// What was executed
    pub source: ExecutionSource,

    /// When execution started
    pub timestamp: DateTime<Utc>,

    /// Target workspaces
    pub workspaces: Vec<String>,

    /// Result summary
    pub result: ExecutionSummary,

    /// Where outputs were written
    pub output_path: Option<PathBuf>,
}

impl ExecutionRecord {
    /// Create a new execution record
    pub fn new(source: ExecutionSource, workspaces: Vec<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            source,
            timestamp: Utc::now(),
            workspaces,
            result: ExecutionSummary::default(),
            output_path: None,
        }
    }

    /// Set the result summary
    pub fn with_result(mut self, result: ExecutionSummary) -> Self {
        self.result = result;
        self
    }

    /// Set the output path
    pub fn with_output_path(mut self, path: PathBuf) -> Self {
        self.output_path = Some(path);
        self
    }

    /// Get a short ID for display (first 8 chars)
    pub fn short_id(&self) -> String {
        self.id.to_string()[..8].to_string()
    }

    /// Get the source name for display
    pub fn source_name(&self) -> String {
        match &self.source {
            ExecutionSource::Pack { name, .. } => name.clone(),
            ExecutionSource::AdHocQuery { .. } => "<ad-hoc query>".to_string(),
        }
    }
}

/// What was executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionSource {
    /// A pack was executed
    Pack {
        /// Pack name
        name: String,
        /// Path to pack file
        path: PathBuf,
        /// Input values provided
        inputs: std::collections::HashMap<String, String>,
    },

    /// An ad-hoc query was executed
    AdHocQuery {
        /// The query text
        query: String,
        /// Timespan (if any)
        timespan: Option<String>,
    },
}

/// Summary of execution results
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// Overall status
    pub status: ExecutionStatusSummary,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Total rows returned across all workspaces
    pub total_rows: usize,

    /// Steps completed (for packs)
    pub steps_completed: usize,

    /// Steps failed (for packs)
    pub steps_failed: usize,

    /// Steps skipped (for packs)
    pub steps_skipped: usize,
}

/// Simplified execution status for history
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionStatusSummary {
    #[default]
    Pending,
    Running,
    Completed,
    PartialSuccess,
    Failed,
}

impl From<ExecutionStatus> for ExecutionStatusSummary {
    fn from(status: ExecutionStatus) -> Self {
        match status {
            ExecutionStatus::Pending => ExecutionStatusSummary::Pending,
            ExecutionStatus::Running => ExecutionStatusSummary::Running,
            ExecutionStatus::Success => ExecutionStatusSummary::Completed,
            ExecutionStatus::Partial => ExecutionStatusSummary::PartialSuccess,
            ExecutionStatus::Failed => ExecutionStatusSummary::Failed,
        }
    }
}

impl std::fmt::Display for ExecutionStatusSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStatusSummary::Pending => write!(f, "pending"),
            ExecutionStatusSummary::Running => write!(f, "running"),
            ExecutionStatusSummary::Completed => write!(f, "completed"),
            ExecutionStatusSummary::PartialSuccess => write!(f, "partial"),
            ExecutionStatusSummary::Failed => write!(f, "failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_add_and_get() {
        let mut history = ExecutionHistory::new();
        assert!(history.is_empty());

        let record = ExecutionRecord::new(
            ExecutionSource::AdHocQuery {
                query: "test".to_string(),
                timespan: None,
            },
            vec!["workspace1".to_string()],
        );
        let id = record.id;

        history.add(record);
        assert_eq!(history.len(), 1);
        assert!(history.get(&id).is_some());
    }

    #[test]
    fn test_get_by_prefix() {
        let mut history = ExecutionHistory::new();

        let record = ExecutionRecord::new(
            ExecutionSource::AdHocQuery {
                query: "test".to_string(),
                timespan: None,
            },
            vec!["workspace1".to_string()],
        );
        let short_id = record.short_id();

        history.add(record);

        // Should find by first few characters
        assert!(history.get_by_prefix(&short_id[..4]).is_some());
    }

    #[test]
    fn test_execution_record_short_id() {
        let record = ExecutionRecord::new(
            ExecutionSource::AdHocQuery {
                query: "test".to_string(),
                timespan: None,
            },
            vec![],
        );
        assert_eq!(record.short_id().len(), 8);
    }
}
