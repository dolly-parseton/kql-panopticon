use crate::workspace::Workspace;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Extracted value from a query result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExtractedValue {
    /// A single value (first row)
    Single(String),
    /// Multiple values (all rows, possibly deduped)
    Array(Vec<String>),
}

#[allow(dead_code)]
impl ExtractedValue {
    /// Check if this value is empty
    pub fn is_empty(&self) -> bool {
        match self {
            ExtractedValue::Single(s) => s.is_empty(),
            ExtractedValue::Array(arr) => arr.is_empty(),
        }
    }

    /// Get the count of values
    pub fn len(&self) -> usize {
        match self {
            ExtractedValue::Single(_) => 1,
            ExtractedValue::Array(arr) => arr.len(),
        }
    }
}

/// Per-workspace context holding extracted values and step status
#[derive(Debug, Clone)]
pub struct WorkspaceContext {
    /// The workspace being processed
    pub workspace: Workspace,
    /// Extracted values keyed by "step_name.variable_name" (legacy extract system)
    pub extractions: HashMap<String, ExtractedValue>,
    /// Raw step results (rows as JSON objects) keyed by step name
    pub step_results: HashMap<String, Vec<serde_json::Value>>,
    /// Column metadata for each step (needed for row-to-object conversion)
    pub step_columns: HashMap<String, Vec<String>>,
    /// Status of each step
    pub step_status: HashMap<String, StepStatus>,
}

impl WorkspaceContext {
    /// Create a new workspace context
    pub fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            extractions: HashMap::new(),
            step_results: HashMap::new(),
            step_columns: HashMap::new(),
            step_status: HashMap::new(),
        }
    }

    /// Get an extracted value by full key (step.variable)
    pub fn get_extraction(&self, key: &str) -> Option<&ExtractedValue> {
        self.extractions.get(key)
    }

    /// Set an extracted value
    pub fn set_extraction(&mut self, step_name: &str, var_name: &str, value: ExtractedValue) {
        let key = format!("{}.{}", step_name, var_name);
        self.extractions.insert(key, value);
    }

    /// Store step results (raw rows converted to JSON objects)
    pub fn set_step_results(&mut self, step_name: &str, columns: Vec<String>, rows: Vec<serde_json::Value>) {
        self.step_columns.insert(step_name.to_string(), columns);
        self.step_results.insert(step_name.to_string(), rows);
    }

    /// Get step results for a given step name
    pub fn get_step_results(&self, step_name: &str) -> Option<&Vec<serde_json::Value>> {
        self.step_results.get(step_name)
    }

    /// Get column names for a step
    pub fn get_step_columns(&self, step_name: &str) -> Option<&Vec<String>> {
        self.step_columns.get(step_name)
    }

    /// Get a column value from all rows of a step
    pub fn get_column_values(&self, step_name: &str, column: &str) -> Vec<String> {
        self.step_results
            .get(step_name)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| {
                        row.get(column).map(|v| match v {
                            serde_json::Value::Null => String::new(),
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get a column value from the first row of a step
    pub fn get_first_column_value(&self, step_name: &str, column: &str) -> Option<String> {
        self.step_results.get(step_name).and_then(|rows| {
            rows.first().and_then(|row| {
                row.get(column).map(|v| match v {
                    serde_json::Value::Null => String::new(),
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
            })
        })
    }

    /// Get a column value from a specific row index of a step
    pub fn get_indexed_column_value(&self, step_name: &str, index: usize, column: &str) -> Option<String> {
        self.step_results.get(step_name).and_then(|rows| {
            rows.get(index).and_then(|row| {
                row.get(column).map(|v| match v {
                    serde_json::Value::Null => String::new(),
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
            })
        })
    }
}

/// Status of a single step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepStatus {
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunks_executed: Option<usize>,
}

/// Execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

/// Progress update sent during investigation execution
#[derive(Debug, Clone)]
pub enum ProgressUpdate {
    /// Investigation started
    Started {
        workspace_count: usize,
        step_count: usize,
    },
    /// A step started for a workspace
    StepStarted {
        workspace_name: String,
        step_name: String,
    },
    /// A step completed for a workspace
    StepCompleted {
        workspace_name: String,
        step_name: String,
        rows: usize,
        duration: Duration,
    },
    /// A step failed for a workspace
    StepFailed {
        workspace_name: String,
        step_name: String,
        error: String,
    },
    /// Investigation completed
    Completed {
        result: InvestigationResult,
    },
}

/// Result of an investigation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestigationResult {
    pub investigation_name: String,
    pub pack_path: Option<String>,
    pub started_at: String,
    pub completed_at: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    pub output_folder: PathBuf,
    pub workspaces: HashMap<String, WorkspaceResult>,
}

/// Result for a single workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceResult {
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    pub steps: HashMap<String, StepStatus>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extracted_value_len() {
        let single = ExtractedValue::Single("test".into());
        assert_eq!(single.len(), 1);

        let array = ExtractedValue::Array(vec!["a".into(), "b".into(), "c".into()]);
        assert_eq!(array.len(), 3);

        let empty = ExtractedValue::Array(vec![]);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_workspace_context() {
        let workspace = Workspace {
            workspace_id: "test-id".into(),
            resource_id: "/subscriptions/test".into(),
            name: "test-workspace".into(),
            location: "eastus".into(),
            subscription_id: "sub-id".into(),
            resource_group: "rg".into(),
            tenant_id: "tenant".into(),
            subscription_name: "Test Sub".into(),
        };

        let mut ctx = WorkspaceContext::new(workspace);

        ctx.set_extraction("step1", "users", ExtractedValue::Array(vec!["user1".into(), "user2".into()]));

        let extracted = ctx.get_extraction("step1.users");
        assert!(extracted.is_some());

        match extracted.unwrap() {
            ExtractedValue::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr[0], "user1");
            }
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn test_step_status_serialization() {
        let status = StepStatus {
            status: Status::Success,
            rows: Some(42),
            duration_ms: Some(1234),
            error: None,
            chunks_executed: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"success\""));
        assert!(json.contains("42"));
        assert!(!json.contains("error")); // skip_serializing_if works
    }
}
