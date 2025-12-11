//! Pack executor for dependency-driven execution
//!
//! Executes pack steps in topological order based on dependencies.
//! Steps with no dependencies can run concurrently (based on config).

use crate::client::{Client, QueryResponse};
use crate::error::{Error, Result};
use crate::pack::{Pack, Step, StepType};
use crate::workspace::Workspace;
use super::engine::{ExecutionEngine, ExecutionOptions};
use super::progress::{JobType, ProgressSender};
use super::trace::ExecutionTrace;
use async_trait::async_trait;
use chrono::Local;
use log::{debug, info};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::fs;

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

/// Pack executor
pub struct PackExecutor {
    client: Client,
    options: ExecutionOptions,
}

impl PackExecutor {
    /// Create a new executor
    pub fn new(client: Client) -> Self {
        Self {
            client,
            options: ExecutionOptions::default(),
        }
    }

    /// Create with options
    pub fn with_options(client: Client, options: ExecutionOptions) -> Self {
        Self { client, options }
    }

    /// Set options
    pub fn options(mut self, options: ExecutionOptions) -> Self {
        self.options = options;
        self
    }

    /// Execute steps for a single workspace
    async fn execute_workspace(
        &self,
        pack: &Pack,
        workspace: &Workspace,
        config: &PackExecutorConfig,
        timestamp: &str,
    ) -> WorkspaceResult {
        let start = Instant::now();
        let mut step_results: HashMap<String, StepResult> = HashMap::new();
        let mut failed_step = None;
        let mut failure_reason = None;

        // Get execution order
        let execution_order = match pack.execution_order() {
            Ok(order) => order,
            Err(e) => {
                return WorkspaceResult {
                    workspace_name: workspace.name.clone(),
                    workspace_id: workspace.workspace_id.clone(),
                    status: ExecutionStatus::Failed,
                    step_results,
                    duration_ms: start.elapsed().as_millis() as u64,
                    failed_step: None,
                    failure_reason: Some(e.to_string()),
                };
            }
        };

        // Execute each step in order
        for step in execution_order {
            // Check if dependencies succeeded
            let deps_ok = pack.get_all_dependencies(step).iter().all(|dep| {
                step_results
                    .get(dep)
                    .map(|r| matches!(r.status, StepStatus::Success))
                    .unwrap_or(false)
            });

            if !deps_ok && !pack.get_all_dependencies(step).is_empty() {
                step_results.insert(
                    step.name.clone(),
                    StepResult {
                        name: step.name.clone(),
                        status: StepStatus::Skipped,
                        row_count: None,
                        duration_ms: 0,
                        output_path: None,
                        error: Some("Dependency failed".to_string()),
                    },
                );
                continue;
            }

            // Execute the step
            let step_result = self
                .execute_step(step, workspace, config, timestamp)
                .await;

            if matches!(step_result.status, StepStatus::Failed) {
                failed_step = Some(step.name.clone());
                failure_reason = step_result.error.clone();
            }

            step_results.insert(step.name.clone(), step_result);
        }

        // Determine overall status
        let status = if failed_step.is_some() {
            if step_results.values().any(|r| matches!(r.status, StepStatus::Success)) {
                ExecutionStatus::Partial
            } else {
                ExecutionStatus::Failed
            }
        } else {
            ExecutionStatus::Success
        };

        WorkspaceResult {
            workspace_name: workspace.name.clone(),
            workspace_id: workspace.workspace_id.clone(),
            status,
            step_results,
            duration_ms: start.elapsed().as_millis() as u64,
            failed_step,
            failure_reason,
        }
    }

    /// Execute a single step
    async fn execute_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        config: &PackExecutorConfig,
        timestamp: &str,
    ) -> StepResult {
        let start = Instant::now();

        match step.step_type {
            StepType::Kql => {
                self.execute_kql_step(step, workspace, config, timestamp, start)
                    .await
            }
            StepType::Http => {
                // TODO: Implement HTTP step execution
                StepResult {
                    name: step.name.clone(),
                    status: StepStatus::Failed,
                    row_count: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    output_path: None,
                    error: Some("HTTP steps not yet implemented".to_string()),
                }
            }
        }
    }

    /// Execute a KQL step
    async fn execute_kql_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        config: &PackExecutorConfig,
        timestamp: &str,
        start: Instant,
    ) -> StepResult {
        let query = match &step.query {
            Some(q) => q.clone(),
            None => {
                return StepResult {
                    name: step.name.clone(),
                    status: StepStatus::Failed,
                    row_count: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    output_path: None,
                    error: Some("No query defined".to_string()),
                };
            }
        };

        // TODO: Variable substitution from inputs and previous step results

        debug!(
            "Executing step '{}' on workspace '{}'",
            step.name, workspace.name
        );

        // Build output path
        let output_dir = config
            .output_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("./output"));

        let normalized_subscription = Workspace::normalize_name(&workspace.subscription_name);
        let normalized_workspace = Workspace::normalize_name(&workspace.name);

        let step_output_dir = output_dir
            .join(normalized_subscription)
            .join(normalized_workspace)
            .join(timestamp);

        // Create directory
        if let Err(e) = fs::create_dir_all(&step_output_dir).await {
            return StepResult {
                name: step.name.clone(),
                status: StepStatus::Failed,
                row_count: None,
                duration_ms: start.elapsed().as_millis() as u64,
                output_path: None,
                error: Some(format!("Failed to create output directory: {}", e)),
            };
        }

        // Execute query
        let timeout = self.client.query_timeout();
        let query_future = self
            .client
            .query_workspace(&workspace.workspace_id, &query, step.timespan.as_deref());

        let response = match tokio::time::timeout(timeout, query_future).await {
            Ok(Ok(response)) => response,
            Ok(Err(e)) => {
                return StepResult {
                    name: step.name.clone(),
                    status: StepStatus::Failed,
                    row_count: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    output_path: None,
                    error: Some(e.to_string()),
                };
            }
            Err(_) => {
                return StepResult {
                    name: step.name.clone(),
                    status: StepStatus::Failed,
                    row_count: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    output_path: None,
                    error: Some(format!("Query timed out after {}s", timeout.as_secs())),
                };
            }
        };

        // Write results
        let output_path = step_output_dir.join(format!("{}.csv", step.name));
        match self
            .write_csv_results(&response, &output_path, workspace, timestamp)
            .await
        {
            Ok(row_count) => {
                info!(
                    "Step '{}' completed: {} rows -> {}",
                    step.name,
                    row_count,
                    output_path.display()
                );
                StepResult {
                    name: step.name.clone(),
                    status: StepStatus::Success,
                    row_count: Some(row_count),
                    duration_ms: start.elapsed().as_millis() as u64,
                    output_path: Some(output_path),
                    error: None,
                }
            }
            Err(e) => StepResult {
                name: step.name.clone(),
                status: StepStatus::Failed,
                row_count: None,
                duration_ms: start.elapsed().as_millis() as u64,
                output_path: None,
                error: Some(format!("Failed to write results: {}", e)),
            },
        }
    }

    /// Write query response to CSV
    async fn write_csv_results(
        &self,
        response: &QueryResponse,
        output_path: &Path,
        _workspace: &Workspace,
        _timestamp: &str,
    ) -> Result<usize> {
        if response.tables.is_empty() {
            return Err(Error::query("Query returned no tables"));
        }

        let table = &response.tables[0];
        let mut content = String::new();

        // Header
        let headers: Vec<_> = table.columns.iter().map(|c| c.name.clone()).collect();
        content.push_str(&headers.join(","));
        content.push('\n');

        // Rows
        let mut row_count = 0;
        for row in &table.rows {
            if let Some(row_array) = row.as_array() {
                let values: Vec<String> = row_array
                    .iter()
                    .map(|v| format_csv_value(v))
                    .collect();
                content.push_str(&values.join(","));
                content.push('\n');
                row_count += 1;
            }
        }

        fs::write(output_path, content).await?;
        Ok(row_count)
    }
}

#[async_trait]
impl ExecutionEngine for PackExecutor {
    type Config = PackExecutorConfig;
    type Result = PackExecutorResult;

    async fn execute(
        &self,
        config: Self::Config,
        workspaces: Vec<Workspace>,
        progress: Option<ProgressSender>,
    ) -> Result<Self::Result> {
        let start = Instant::now();
        let job_id = progress
            .as_ref()
            .map(|p| p.job_id())
            .unwrap_or_else(uuid::Uuid::new_v4);

        let num_workspaces = workspaces.len();
        let total_steps = config.pack.steps.len() * num_workspaces;

        // Notify start
        if let Some(ref tx) = progress {
            tx.started(JobType::Query, total_steps, num_workspaces);
        }

        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let pack = &config.pack;

        // Execute across workspaces
        let mut workspace_results: HashMap<String, WorkspaceResult> = HashMap::new();

        for workspace in &workspaces {
            let result = self
                .execute_workspace(pack, workspace, &config, &timestamp)
                .await;
            workspace_results.insert(workspace.workspace_id.clone(), result);
        }

        // Determine overall status
        let status = if workspace_results
            .values()
            .all(|r| matches!(r.status, ExecutionStatus::Success))
        {
            ExecutionStatus::Success
        } else if workspace_results
            .values()
            .all(|r| matches!(r.status, ExecutionStatus::Failed))
        {
            ExecutionStatus::Failed
        } else {
            ExecutionStatus::Partial
        };

        let result = PackExecutorResult {
            job_id,
            pack_name: pack.name.clone(),
            status,
            workspace_results,
            duration_ms: start.elapsed().as_millis() as u64,
            output_dir: config.output_dir,
            trace: None,
        };

        // Notify completion
        if let Some(ref tx) = progress {
            if result.all_succeeded() {
                tx.completed(result.duration_ms);
            } else {
                tx.failed(format!(
                    "{} succeeded, {} failed",
                    result.success_count(),
                    result.failure_count()
                ));
            }
        }

        Ok(result)
    }

    fn validate(&self, config: &Self::Config) -> Result<()> {
        config.pack.validate()
    }

    fn executor_name(&self) -> &'static str {
        "PackExecutor"
    }
}

/// Format a JSON value for CSV
fn format_csv_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            if s.contains(',') || s.contains('"') || s.contains('\n') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.clone()
            }
        }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            let json_str = value.to_string();
            format!("\"{}\"", json_str.replace('"', "\"\""))
        }
    }
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
