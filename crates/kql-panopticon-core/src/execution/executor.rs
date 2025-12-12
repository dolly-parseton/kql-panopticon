//! Pack executor for dependency-driven execution
//!
//! Executes pack steps in topological order based on dependencies.
//! Steps with no dependencies can run when ready; steps with dependencies
//! wait for their dependencies to complete.

use crate::client::Client;
use crate::error::{Error, Result};
use crate::pack::{AggregateStrategy, ForeachClause, OnEmpty, OnError, Pack, Step, StepType};
use crate::variable::{evaluate_condition, SubstitutionContext};
use crate::workspace::Workspace;

use super::engine::{ExecutionEngine, ExecutionOptions};
use super::handlers::{FileHandler, HttpHandler, KqlHandler};
use super::output::write_csv_results;
use super::progress::{JobType, ProgressSender};
use super::step::{StepContext, StepHandler, StepOutput};
use super::trace::{
    ErrorCategory, ErrorTrace, ExecutionTrace, StepTrace, TraceStatus,
    StepType as TraceStepType,
};
use super::types::{
    ExecutionStatus, PackExecutorConfig, PackExecutorResult, StepResult, StepStatus,
    WorkspaceResult,
};

use async_trait::async_trait;
use chrono::Local;
use log::{debug, info, warn};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;

/// Pack executor - coordinates step execution
pub struct PackExecutor {
    /// Registered step handlers
    handlers: Vec<Box<dyn StepHandler>>,

    /// Default timeout for steps
    default_timeout: Duration,

    /// Execution options
    options: ExecutionOptions,
}

impl PackExecutor {
    /// Create a new executor with the given client
    pub fn new(client: Client) -> Self {
        let client = Arc::new(client);
        Self {
            handlers: vec![
                Box::new(KqlHandler::new(client)),
                Box::new(HttpHandler::new()),
                Box::new(FileHandler::new()),
            ],
            default_timeout: Duration::from_secs(120),
            options: ExecutionOptions::default(),
        }
    }

    /// Create with custom options
    pub fn with_options(client: Client, options: ExecutionOptions) -> Self {
        let mut executor = Self::new(client);
        executor.options = options;
        executor
    }

    /// Set default timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Get handler for a step type
    fn get_handler(&self, step_type: StepType) -> Option<&dyn StepHandler> {
        self.handlers
            .iter()
            .find(|h| h.step_type() == step_type)
            .map(|h| h.as_ref())
    }

    /// Execute steps for a single workspace
    async fn execute_workspace(
        &self,
        pack: &Pack,
        workspace: &Workspace,
        config: &PackExecutorConfig,
        timestamp: &str,
        progress: Option<&ProgressSender>,
    ) -> WorkspaceResult {
        let start = Instant::now();
        let mut step_results: HashMap<String, StepResult> = HashMap::new();
        let mut step_data: HashMap<String, Vec<JsonValue>> = HashMap::new();
        let mut failed_step = None;
        let mut failure_reason = None;

        // Build output directory
        let output_dir = config
            .output_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("./output"));

        let workspace_output_dir = output_dir
            .join(Workspace::normalize_name(&workspace.subscription_name))
            .join(Workspace::normalize_name(&workspace.name))
            .join(timestamp);

        // Create output directory
        if let Err(e) = fs::create_dir_all(&workspace_output_dir).await {
            return WorkspaceResult {
                workspace_name: workspace.name.clone(),
                workspace_id: workspace.workspace_id.clone(),
                status: ExecutionStatus::Failed,
                step_results,
                step_data,
                duration_ms: start.elapsed().as_millis() as u64,
                failed_step: None,
                failure_reason: Some(format!("Failed to create output directory: {}", e)),
            };
        }

        // Get execution order
        let execution_order = match pack.execution_order() {
            Ok(order) => order,
            Err(e) => {
                return WorkspaceResult {
                    workspace_name: workspace.name.clone(),
                    workspace_id: workspace.workspace_id.clone(),
                    status: ExecutionStatus::Failed,
                    step_results,
                    step_data,
                    duration_ms: start.elapsed().as_millis() as u64,
                    failed_step: None,
                    failure_reason: Some(e.to_string()),
                };
            }
        };

        // Build initial substitution context with inputs
        let mut substitution = SubstitutionContext::new();
        for (key, value) in &config.inputs {
            substitution.inputs.insert(key.clone(), value.clone());
        }

        // Execute each step in order
        for step in execution_order {
            // Check if dependencies succeeded
            let deps = pack.get_all_dependencies(step);
            let deps_ok = deps.iter().all(|dep| {
                step_results
                    .get(dep)
                    .map(|r| matches!(r.status, StepStatus::Success))
                    .unwrap_or(false)
            });

            if !deps_ok && !deps.is_empty() {
                // Emit skip event
                if let Some(tx) = progress {
                    tx.step_skipped(&step.name, &workspace.name, "Dependency failed");
                }

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

            // Check `when` condition if specified
            if let Some(when_condition) = &step.when {
                let condition_met = evaluate_condition(when_condition, &substitution.step_results);

                debug!(
                    "Step '{}' when condition '{}' evaluated to: {}",
                    step.name, when_condition, condition_met
                );

                if !condition_met {
                    let reason = format!("Condition not met: {}", when_condition);

                    // Emit skip event
                    if let Some(tx) = progress {
                        tx.step_skipped(&step.name, &workspace.name, &reason);
                    }

                    step_results.insert(
                        step.name.clone(),
                        StepResult {
                            name: step.name.clone(),
                            status: StepStatus::Skipped,
                            row_count: None,
                            duration_ms: 0,
                            output_path: None,
                            error: Some(reason),
                        },
                    );
                    continue;
                }
            }

            // Emit step started event
            if let Some(tx) = progress {
                tx.step_started(&step.name, &workspace.name);
            }

            // Check if this is a foreach step
            let result = if let Some(foreach_str) = &step.foreach {
                self.execute_foreach_step(
                    step,
                    foreach_str,
                    workspace,
                    &substitution,
                    &workspace_output_dir,
                    progress,
                )
                .await
            } else {
                // Single execution
                self.execute_step(step, workspace, &substitution, &workspace_output_dir)
                    .await
            };

            match result {
                Ok(output) => {
                    let duration_ms = output.duration.as_millis() as u64;

                    // Emit step completed event
                    if let Some(tx) = progress {
                        tx.step_completed(&step.name, &workspace.name, output.row_count, duration_ms);
                    }

                    // Add results to substitution context for downstream steps
                    substitution
                        .step_results
                        .insert(step.name.clone(), output.rows.clone());
                    step_data.insert(step.name.clone(), output.rows.clone());

                    // Write results to file
                    let output_path = workspace_output_dir.join(format!("{}.csv", step.name));
                    if let Err(e) = write_csv_results(&output.rows, &output_path).await {
                        warn!("Failed to write results for step '{}': {}", step.name, e);
                    }

                    step_results.insert(
                        step.name.clone(),
                        StepResult {
                            name: step.name.clone(),
                            status: StepStatus::Success,
                            row_count: Some(output.row_count),
                            duration_ms,
                            output_path: Some(output_path),
                            error: None,
                        },
                    );

                    info!(
                        "Step '{}' completed: {} rows",
                        step.name, output.row_count
                    );
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    // Handle on_error behavior
                    let should_fail = match step.on_error.unwrap_or_default() {
                        OnError::Fail => true,
                        OnError::Skip | OnError::Continue => {
                            info!("Step '{}' error ignored due to on_error setting: {}", step.name, error_msg);
                            false
                        }
                    };

                    if should_fail {
                        // Emit step failed event
                        if let Some(tx) = progress {
                            tx.step_failed(&step.name, &workspace.name, &error_msg);
                        }

                        failed_step = Some(step.name.clone());
                        failure_reason = Some(error_msg.clone());

                        step_results.insert(
                            step.name.clone(),
                            StepResult {
                                name: step.name.clone(),
                                status: StepStatus::Failed,
                                row_count: None,
                                duration_ms: 0,
                                output_path: None,
                                error: Some(error_msg.clone()),
                            },
                        );

                        warn!("Step '{}' failed: {}", step.name, error_msg);
                    } else {
                        // Record as skipped with empty results
                        if let Some(tx) = progress {
                            tx.step_skipped(&step.name, &workspace.name, format!("Error ignored: {}", error_msg));
                        }

                        substitution.step_results.insert(step.name.clone(), vec![]);
                        step_data.insert(step.name.clone(), vec![]);

                        step_results.insert(
                            step.name.clone(),
                            StepResult {
                                name: step.name.clone(),
                                status: StepStatus::Skipped,
                                row_count: Some(0),
                                duration_ms: 0,
                                output_path: None,
                                error: Some(format!("Error ignored: {}", error_msg)),
                            },
                        );
                    }
                }
            }
        }

        // Determine overall status
        let status = if failed_step.is_some() {
            if step_results
                .values()
                .any(|r| matches!(r.status, StepStatus::Success))
            {
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
            step_data,
            duration_ms: start.elapsed().as_millis() as u64,
            failed_step,
            failure_reason,
        }
    }

    /// Execute a single step using the appropriate handler
    async fn execute_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        substitution: &SubstitutionContext,
        output_dir: &std::path::Path,
    ) -> Result<StepOutput> {
        let handler = self.get_handler(step.step_type).ok_or_else(|| {
            Error::execution(format!("No handler for step type {:?}", step.step_type))
        })?;

        let ctx = StepContext {
            substitution,
            workspace: Some(workspace),
            output_dir,
            timeout: self.default_timeout,
        };

        debug!("Executing step '{}' (type: {:?})", step.name, step.step_type);
        handler.execute(step, &ctx).await
    }

    /// Build execution trace from workspace results
    fn build_trace(
        &self,
        pack: &Pack,
        workspace_results: &HashMap<String, WorkspaceResult>,
    ) -> ExecutionTrace {
        let mut trace = ExecutionTrace::new();

        // Add pack inputs to context
        for input in &pack.inputs {
            trace.set_context(
                format!("input.{}", input.name),
                serde_json::json!({
                    "required": input.required,
                    "default": input.default,
                }),
            );
        }

        // Build step map for quick lookup
        let step_map: HashMap<&str, &Step> = pack.steps.iter().map(|s| (s.name.as_str(), s)).collect();

        // Create step traces from results
        for (workspace_id, ws_result) in workspace_results {
            for (step_name, step_result) in &ws_result.step_results {
                let step = step_map.get(step_name.as_str());
                let step_type = step
                    .map(|s| TraceStepType::from(s.step_type))
                    .unwrap_or(TraceStepType::Kql);

                let mut step_trace = StepTrace::new(step_name, step_type);
                step_trace.workspace = Some(ws_result.workspace_name.clone());
                step_trace.duration_ms = Some(step_result.duration_ms);
                step_trace.rows = step_result.row_count;

                // Set status based on step result
                step_trace.status = match step_result.status {
                    StepStatus::Success => TraceStatus::Success,
                    StepStatus::Failed => TraceStatus::Failed,
                    StepStatus::Skipped => TraceStatus::Skipped,
                    StepStatus::Pending => TraceStatus::Pending,
                    StepStatus::Running => TraceStatus::Running,
                };

                // Add error info if present
                if let Some(error_msg) = &step_result.error {
                    step_trace.error = Some(ErrorTrace::new(error_msg, ErrorCategory::Unknown));
                }

                trace.add_step(step_trace);
            }
        }

        trace
    }

    /// Execute a step with foreach iteration
    async fn execute_foreach_step(
        &self,
        step: &Step,
        foreach_str: &str,
        workspace: &Workspace,
        substitution: &SubstitutionContext,
        output_dir: &std::path::Path,
        progress: Option<&ProgressSender>,
    ) -> Result<StepOutput> {
        let start = Instant::now();

        // Parse foreach clause: "source_step as alias"
        let foreach_clause = ForeachClause::parse(foreach_str).ok_or_else(|| {
            Error::pack(format!(
                "Invalid foreach syntax '{}' in step '{}'. Expected: 'step_name as alias'",
                foreach_str, step.name
            ))
        })?;

        // Get source step results
        let source_rows = substitution
            .step_results
            .get(&foreach_clause.source_step)
            .cloned()
            .unwrap_or_default();

        // Handle empty source
        if source_rows.is_empty() {
            let on_empty = step.on_empty.unwrap_or_default();
            match on_empty {
                OnEmpty::Skip => {
                    debug!(
                        "Step '{}' skipped: foreach source '{}' is empty",
                        step.name, foreach_clause.source_step
                    );
                    return Ok(StepOutput::empty(start.elapsed()));
                }
                OnEmpty::Error => {
                    return Err(Error::execution(format!(
                        "Foreach source '{}' is empty for step '{}'",
                        foreach_clause.source_step, step.name
                    )));
                }
            }
        }

        let total_iterations = source_rows.len();
        let mut all_results: Vec<JsonValue> = Vec::new();
        let mut failed_count = 0;
        let on_error = step.on_error.unwrap_or_default();

        debug!(
            "Step '{}' starting foreach over {} rows from '{}'",
            step.name, total_iterations, foreach_clause.source_step
        );

        for (index, row) in source_rows.iter().enumerate() {
            // Emit foreach progress
            if let Some(tx) = progress {
                tx.send(super::progress::ProgressUpdate::ForeachProgress {
                    job_id: tx.job_id(),
                    step_name: step.name.clone(),
                    workspace: workspace.name.clone(),
                    current: index + 1,
                    total: total_iterations,
                    timestamp: chrono::Utc::now(),
                });
            }

            // Create context with foreach row
            let mut iter_context = substitution.clone();
            iter_context.foreach_row = Some((foreach_clause.alias.clone(), row.clone()));

            // Execute single iteration
            let iter_result = self
                .execute_step(step, workspace, &iter_context, output_dir)
                .await;

            match iter_result {
                Ok(output) => {
                    // Aggregate results based on strategy
                    match step.aggregate.unwrap_or_default() {
                        AggregateStrategy::Append => {
                            all_results.extend(output.rows);
                        }
                        AggregateStrategy::Collect => {
                            // Wrap each iteration's results as a single array element
                            all_results.push(serde_json::json!({
                                "_iteration": index,
                                "_source_row": row,
                                "results": output.rows,
                            }));
                        }
                        AggregateStrategy::Merge => {
                            // For merge, we take the first row from each iteration
                            if let Some(first) = output.rows.into_iter().next() {
                                all_results.push(first);
                            }
                        }
                        AggregateStrategy::Replace => {
                            // Replace keeps only the last iteration's results
                            all_results = output.rows;
                        }
                    }
                }
                Err(e) => {
                    failed_count += 1;
                    warn!(
                        "Foreach iteration {}/{} failed for step '{}': {}",
                        index + 1,
                        total_iterations,
                        step.name,
                        e
                    );

                    match on_error {
                        OnError::Fail => {
                            return Err(Error::execution(format!(
                                "Foreach iteration {} failed: {}",
                                index + 1,
                                e
                            )));
                        }
                        OnError::Skip | OnError::Continue => {
                            // Continue to next iteration
                            continue;
                        }
                    }
                }
            }
        }

        let successful = total_iterations - failed_count;
        debug!(
            "Step '{}' foreach completed: {}/{} iterations successful, {} total results",
            step.name,
            successful,
            total_iterations,
            all_results.len()
        );

        Ok(StepOutput::from_rows(all_results, start.elapsed()))
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

        info!(
            "Starting pack '{}' execution across {} workspaces",
            pack.name, num_workspaces
        );

        // Execute across workspaces
        let mut workspace_results: HashMap<String, WorkspaceResult> = HashMap::new();

        for workspace in &workspaces {
            let result = self
                .execute_workspace(pack, workspace, &config, &timestamp, progress.as_ref())
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

        // Build execution trace from results
        let trace = self.build_trace(pack, &workspace_results);

        let result = PackExecutorResult {
            job_id,
            pack_name: pack.name.clone(),
            status,
            workspace_results,
            duration_ms: start.elapsed().as_millis() as u64,
            output_dir: config.output_dir,
            trace: Some(trace),
        };

        info!(
            "Pack '{}' completed: {} succeeded, {} failed in {}ms",
            pack.name,
            result.success_count(),
            result.failure_count(),
            result.duration_ms
        );

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
        // Validate pack structure
        config.pack.validate()?;

        // Validate each step with its handler
        for step in &config.pack.steps {
            if let Some(handler) = self.get_handler(step.step_type) {
                handler.validate(step)?;
            } else {
                return Err(Error::pack(format!(
                    "No handler for step type {:?} in step '{}'",
                    step.step_type, step.name
                )));
            }
        }

        Ok(())
    }

    fn executor_name(&self) -> &'static str {
        "PackExecutor"
    }
}
