//! Execution tracing for debugging
//!
//! Provides detailed tracing of execution for debugging and troubleshooting.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete execution trace for debugging
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Traced steps
    pub steps: Vec<StepTrace>,
    /// Global context (inputs, resolved secrets, etc.)
    pub context: HashMap<String, serde_json::Value>,
}

impl ExecutionTrace {
    /// Create a new empty trace
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a step trace
    pub fn add_step(&mut self, step: StepTrace) {
        self.steps.push(step);
    }

    /// Get a step trace by name
    pub fn get_step(&self, name: &str) -> Option<&StepTrace> {
        self.steps.iter().find(|s| s.name == name)
    }

    /// Set context value
    pub fn set_context(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.context.insert(key.into(), value);
    }

    /// Get total execution time
    pub fn total_duration_ms(&self) -> u64 {
        self.steps.iter().map(|s| s.duration_ms.unwrap_or(0)).sum()
    }

    /// Get count of failed steps
    pub fn failed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s.status, TraceStatus::Failed))
            .count()
    }

    /// Get count of skipped steps
    pub fn skipped_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s.status, TraceStatus::Skipped))
            .count()
    }
}

/// Trace for a single step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTrace {
    /// Step name
    pub name: String,
    /// Workspace (if workspace-specific)
    pub workspace: Option<String>,
    /// Step type
    pub step_type: StepType,
    /// Execution status
    pub status: TraceStatus,
    /// When step started
    pub started_at: DateTime<Utc>,
    /// When step completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Input variables that were resolved
    pub resolved_inputs: HashMap<String, String>,
    /// The actual query/request after variable substitution
    pub submitted: Option<SubmittedRequest>,
    /// Row count returned
    pub rows: Option<usize>,
    /// Variables extracted
    pub extracted_variables: HashMap<String, ExtractedVariable>,
    /// Error details (if failed)
    pub error: Option<ErrorTrace>,
    /// Condition evaluation (if `when` clause present)
    pub condition: Option<ConditionTrace>,
    /// Foreach iteration details (if foreach step)
    pub foreach: Option<ForeachTrace>,
}

impl StepTrace {
    /// Create a new step trace
    pub fn new(name: impl Into<String>, step_type: StepType) -> Self {
        Self {
            name: name.into(),
            workspace: None,
            step_type,
            status: TraceStatus::Pending,
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            resolved_inputs: HashMap::new(),
            submitted: None,
            rows: None,
            extracted_variables: HashMap::new(),
            error: None,
            condition: None,
            foreach: None,
        }
    }

    /// Mark step as started
    pub fn start(&mut self) {
        self.status = TraceStatus::Running;
        self.started_at = Utc::now();
    }

    /// Mark step as completed
    pub fn complete(&mut self, rows: usize) {
        self.status = TraceStatus::Success;
        self.completed_at = Some(Utc::now());
        self.rows = Some(rows);
        self.duration_ms = Some(
            (Utc::now() - self.started_at).num_milliseconds() as u64
        );
    }

    /// Mark step as failed
    pub fn fail(&mut self, error: ErrorTrace) {
        self.status = TraceStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.error = Some(error);
        self.duration_ms = Some(
            (Utc::now() - self.started_at).num_milliseconds() as u64
        );
    }

    /// Mark step as skipped
    pub fn skip(&mut self, reason: impl Into<String>) {
        self.status = TraceStatus::Skipped;
        self.completed_at = Some(Utc::now());
        self.error = Some(ErrorTrace {
            message: reason.into(),
            category: ErrorCategory::Skipped,
            details: HashMap::new(),
            suggestion: None,
        });
    }

    /// Record resolved input
    pub fn record_input(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.resolved_inputs.insert(name.into(), value.into());
    }

    /// Record extracted variable
    pub fn record_extraction(&mut self, name: impl Into<String>, variable: ExtractedVariable) {
        self.extracted_variables.insert(name.into(), variable);
    }
}

/// Type of step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepType {
    Kql,
    Http,
    File,
}

impl From<crate::pack::StepType> for StepType {
    fn from(st: crate::pack::StepType) -> Self {
        match st {
            crate::pack::StepType::Kql => StepType::Kql,
            crate::pack::StepType::Http => StepType::Http,
            crate::pack::StepType::File => StepType::File,
        }
    }
}

/// Status in trace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

/// The actual request/query submitted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubmittedRequest {
    /// KQL query
    Kql {
        query: String,
        timespan: Option<String>,
    },
    /// HTTP request
    Http {
        method: String,
        url: String,
        headers: HashMap<String, String>,
        body: Option<String>,
    },
}

/// Details of an extracted variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedVariable {
    /// Column or field extracted from
    pub source: String,
    /// Number of values extracted
    pub count: usize,
    /// Sample values (first few)
    pub sample: Vec<String>,
    /// Whether values were deduplicated
    pub deduped: bool,
}

/// Error trace information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorTrace {
    /// Error message
    pub message: String,
    /// Error category
    pub category: ErrorCategory,
    /// Additional context
    pub details: HashMap<String, String>,
    /// Suggested fix (if known)
    pub suggestion: Option<String>,
}

impl ErrorTrace {
    /// Create a new error trace
    pub fn new(message: impl Into<String>, category: ErrorCategory) -> Self {
        Self {
            message: message.into(),
            category,
            details: HashMap::new(),
            suggestion: None,
        }
    }

    /// Add detail
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    /// Add suggestion
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Category of error
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Variable substitution failed
    Variable,
    /// Query syntax or execution error
    Query,
    /// HTTP request failed
    Http,
    /// Dependency not met
    Dependency,
    /// Condition evaluated to false
    Condition,
    /// Step was intentionally skipped
    Skipped,
    /// Timeout
    Timeout,
    /// Unknown/other
    Unknown,
}

/// Trace of condition evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionTrace {
    /// The condition expression
    pub expression: String,
    /// The result (true/false)
    pub result: bool,
    /// Values that were evaluated
    pub evaluated_values: HashMap<String, serde_json::Value>,
}

/// Trace of foreach iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeachTrace {
    /// Source step
    pub source_step: String,
    /// Alias used
    pub alias: String,
    /// Total iterations
    pub total_iterations: usize,
    /// Successful iterations
    pub successful_iterations: usize,
    /// Failed iterations
    pub failed_iterations: usize,
    /// Batch size
    pub batch_size: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_trace_lifecycle() {
        let mut trace = StepTrace::new("test_step", StepType::Kql);
        assert!(matches!(trace.status, TraceStatus::Pending));

        trace.start();
        assert!(matches!(trace.status, TraceStatus::Running));

        trace.complete(100);
        assert!(matches!(trace.status, TraceStatus::Success));
        assert_eq!(trace.rows, Some(100));
        assert!(trace.duration_ms.is_some());
    }

    #[test]
    fn test_execution_trace() {
        let mut trace = ExecutionTrace::new();

        let mut step1 = StepTrace::new("step1", StepType::Kql);
        step1.complete(50);
        trace.add_step(step1);

        let mut step2 = StepTrace::new("step2", StepType::Kql);
        step2.fail(ErrorTrace::new("test error", ErrorCategory::Query));
        trace.add_step(step2);

        assert_eq!(trace.failed_count(), 1);
        assert!(trace.get_step("step1").is_some());
    }
}
