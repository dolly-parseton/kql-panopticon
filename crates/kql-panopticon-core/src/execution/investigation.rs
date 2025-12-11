//! Investigation executor for chained, ordered execution
//!
//! Handles execution of investigation packs - running steps in dependency
//! order with variable passing between steps.

use crate::client::Client;
use crate::error::Result;
use crate::pack::InvestigationPack;
use crate::workspace::Workspace;
use super::engine::{ExecutionEngine, ExecutionOptions};
use super::progress::ProgressSender;
use super::trace::ExecutionTrace;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;

/// Executor for investigation packs
///
/// Runs steps in dependency order, extracting variables from each step's
/// results to substitute into subsequent queries.
pub struct InvestigationExecutor {
    client: Client,
    options: ExecutionOptions,
}

impl InvestigationExecutor {
    /// Create a new investigation executor
    pub fn new(client: Client) -> Self {
        Self {
            client,
            options: ExecutionOptions::default(),
        }
    }

    /// Create with custom options
    pub fn with_options(client: Client, options: ExecutionOptions) -> Self {
        Self { client, options }
    }

    /// Set execution options
    pub fn options(mut self, options: ExecutionOptions) -> Self {
        self.options = options;
        self
    }
}

#[async_trait]
impl ExecutionEngine for InvestigationExecutor {
    type Config = InvestigationExecutorConfig;
    type Result = InvestigationExecutorResult;

    async fn execute(
        &self,
        config: Self::Config,
        workspaces: Vec<Workspace>,
        progress: Option<ProgressSender>,
    ) -> Result<Self::Result> {
        // TODO: Move implementation from investigation/runner.rs
        //
        // High-level flow:
        // 1. Validate pack and resolve secrets
        // 2. Build execution order (topological sort by depends_on)
        // 3. Send Started progress event
        // 4. For each workspace (can be parallel across workspaces):
        //    a. Create WorkspaceContext with empty extractions
        //    b. For each step in execution order:
        //       i.   Check dependencies (skip if any failed)
        //       ii.  Evaluate `when` condition
        //       iii. Handle `foreach` iteration if present
        //       iv.  Substitute variables in query/request
        //       v.   Execute (KQL or HTTP)
        //       vi.  Extract variables from results
        //       vii. Store results in context
        //       viii. Send progress events
        // 5. Evaluate verdict rules
        // 6. Calculate risk score
        // 7. Generate report (if configured)
        // 8. Send Completed progress event
        // 9. Return InvestigationExecutorResult

        let _ = (config, workspaces, progress, &self.client, &self.options);
        todo!("Move InvestigationRunner implementation to InvestigationExecutor")
    }

    fn validate(&self, config: &Self::Config) -> Result<()> {
        // Delegate to pack's built-in validation
        config.pack.validate().map_err(|e| {
            crate::error::Error::Pack {
                message: e.to_string(),
                path: config.pack_path.as_ref().map(|p| p.display().to_string()),
            }
        })
    }

    fn executor_name(&self) -> &'static str {
        "InvestigationExecutor"
    }
}

/// Configuration for investigation execution
#[derive(Debug, Clone)]
pub struct InvestigationExecutorConfig {
    /// The investigation pack to execute
    pub pack: InvestigationPack,
    /// Path the pack was loaded from (for error messages)
    pub pack_path: Option<PathBuf>,
    /// User-provided input values
    pub inputs: HashMap<String, String>,
    /// Output directory for results
    pub output_dir: Option<PathBuf>,
}

impl InvestigationExecutorConfig {
    /// Create a new config from a pack
    pub fn new(pack: InvestigationPack) -> Self {
        Self {
            pack,
            pack_path: None,
            inputs: HashMap::new(),
            output_dir: None,
        }
    }

    /// Set pack path (for error messages)
    pub fn with_pack_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.pack_path = Some(path.into());
        self
    }

    /// Set input values
    pub fn with_inputs(mut self, inputs: HashMap<String, String>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Add a single input value
    pub fn with_input(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.inputs.insert(key.into(), value.into());
        self
    }

    /// Set output directory
    pub fn with_output_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.output_dir = Some(path.into());
        self
    }
}

/// Result of investigation execution
#[derive(Debug, Clone)]
pub struct InvestigationExecutorResult {
    /// Job ID
    pub job_id: uuid::Uuid,
    /// Investigation name
    pub investigation_name: String,
    /// Overall status
    pub status: InvestigationStatus,
    /// Results per workspace
    pub workspace_results: HashMap<String, WorkspaceInvestigationResult>,
    /// Total duration in milliseconds
    pub duration_ms: u64,
    /// Output directory
    pub output_dir: Option<PathBuf>,
    /// Execution trace (for debugging)
    pub trace: Option<ExecutionTrace>,
    /// Verdict (if verdict rules configured)
    pub verdict: Option<Verdict>,
    /// Risk score (if scoring configured)
    pub risk_score: Option<RiskScore>,
    /// Report path (if report generated)
    pub report_path: Option<PathBuf>,
}

impl InvestigationExecutorResult {
    /// Check if investigation succeeded across all workspaces
    pub fn all_succeeded(&self) -> bool {
        self.workspace_results
            .values()
            .all(|r| matches!(r.status, InvestigationStatus::Success))
    }

    /// Get count of successful workspaces
    pub fn success_count(&self) -> usize {
        self.workspace_results
            .values()
            .filter(|r| matches!(r.status, InvestigationStatus::Success))
            .count()
    }

    /// Get list of failed workspaces with errors
    pub fn failures(&self) -> Vec<(&str, &str)> {
        self.workspace_results
            .iter()
            .filter_map(|(ws, r)| {
                r.failure_reason
                    .as_ref()
                    .map(|reason| (ws.as_str(), reason.as_str()))
            })
            .collect()
    }
}

/// Result for a single workspace investigation
#[derive(Debug, Clone)]
pub struct WorkspaceInvestigationResult {
    /// Workspace name
    pub workspace_name: String,
    /// Status
    pub status: InvestigationStatus,
    /// Step that failed (if any)
    pub failed_step: Option<String>,
    /// Failure reason (if failed)
    pub failure_reason: Option<String>,
    /// Per-step results
    pub step_results: HashMap<String, StepResult>,
}

/// Status of an investigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvestigationStatus {
    /// Not yet started
    Pending,
    /// Currently running
    Running,
    /// Completed successfully
    Success,
    /// Failed with error
    Failed,
    /// Partially completed (some workspaces failed)
    Partial,
}

impl std::fmt::Display for InvestigationStatus {
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

/// Result for a single step
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Step name
    pub name: String,
    /// Status
    pub status: StepStatus,
    /// Row count (if applicable)
    pub rows: Option<usize>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Number of chunks executed (for chunked queries)
    pub chunks_executed: Option<usize>,
}

/// Status of an individual step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

/// Verdict from verdict rules evaluation
#[derive(Debug, Clone)]
pub struct Verdict {
    /// Rule name that matched
    pub rule_name: String,
    /// Verdict level (e.g., "CRITICAL", "HIGH", "LOW")
    pub level: String,
    /// Summary message
    pub summary: String,
    /// Recommendation
    pub recommendation: Option<String>,
}

/// Risk score from scoring evaluation
#[derive(Debug, Clone)]
pub struct RiskScore {
    /// Total score
    pub score: i32,
    /// Risk level based on thresholds
    pub level: String,
    /// Individual indicator scores
    pub indicators: Vec<IndicatorScore>,
}

/// Score from a single indicator
#[derive(Debug, Clone)]
pub struct IndicatorScore {
    /// Indicator name
    pub name: String,
    /// Whether the condition matched
    pub matched: bool,
    /// Weight (positive = risk, negative = benign)
    pub weight: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_investigation_status_display() {
        assert_eq!(InvestigationStatus::Success.to_string(), "Success");
        assert_eq!(InvestigationStatus::Failed.to_string(), "Failed");
    }
}
