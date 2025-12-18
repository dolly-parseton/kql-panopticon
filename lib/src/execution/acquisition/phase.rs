//! Acquisition phase handler
//!
//! Orchestrates execution of acquisition steps within a workspace.

use crate::error::{Error, Result};
use crate::execution::progress::{ExecutionPhase, ProgressSender};
use crate::execution::result::ResultContext;
use crate::pack::{Acquisition, AcquisitionStepType, OnError, Pack, Step};
use crate::variable::evaluate_condition_new as evaluate_condition;
use crate::workspace::Workspace;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::context::AcquisitionContext;
use super::handler::AcquisitionStepHandler;
use super::output::AcquisitionStepOutput;

/// Phase-level output from acquisition
#[derive(Debug)]
pub struct AcquisitionPhaseOutput {
    /// Accumulated step results
    pub results: ResultContext,

    /// Per-step execution status
    pub step_statuses: HashMap<String, StepExecutionStatus>,

    /// Total duration
    pub duration: Duration,

    /// First failed step (if any)
    pub failed_step: Option<String>,

    /// Failure reason
    pub failure_reason: Option<String>,
}

/// Status of a step execution
#[derive(Debug, Clone)]
pub struct StepExecutionStatus {
    pub name: String,
    pub status: AcquisitionStepStatus,
    pub row_count: Option<usize>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// Step execution status for acquisition phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquisitionStepStatus {
    Success,
    Failed,
    Skipped,
}

/// Acquisition phase handler
///
/// Coordinates execution of all acquisition steps in dependency order.
pub struct AcquisitionPhaseHandler {
    handlers: HashMap<AcquisitionStepType, Box<dyn AcquisitionStepHandler>>,
    default_timeout: Duration,
}

impl AcquisitionPhaseHandler {
    /// Create a new phase handler
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            default_timeout: Duration::from_secs(120),
        }
    }

    /// Register a step handler
    pub fn register(&mut self, handler: Box<dyn AcquisitionStepHandler>) {
        self.handlers.insert(handler.handles(), handler);
    }

    /// Set default timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Get handler for a step type
    fn get_handler(&self, step_type: AcquisitionStepType) -> Option<&dyn AcquisitionStepHandler> {
        self.handlers.get(&step_type).map(|h| h.as_ref())
    }

    /// Execute the acquisition phase
    pub async fn execute(
        &self,
        pack: &Pack,
        workspace: &Workspace,
        inputs: HashMap<String, String>,
        output_dir: &Path,
        progress: Option<&ProgressSender>,
    ) -> Result<AcquisitionPhaseOutput> {
        let phase_start = Instant::now();
        let mut step_statuses: HashMap<String, StepExecutionStatus> = HashMap::new();
        let mut failed_step = None;
        let mut failure_reason = None;

        // Build input types map
        let input_types = pack
            .acquisition
            .inputs
            .iter()
            .map(|i| (i.name.clone(), i.input_type))
            .collect();

        // Create context
        let mut ctx = AcquisitionContext::with_inputs(
            workspace,
            output_dir,
            self.default_timeout,
            inputs,
            input_types,
        );

        // Get execution order
        let execution_order = pack.execution_order()?;

        debug!(
            phase = "acquisition",
            workspace = %workspace.name,
            steps = execution_order.len(),
            "Starting acquisition phase"
        );

        // Execute each step in order
        for step in execution_order {
            // Check dependencies
            let deps = pack.get_all_dependencies(step);
            let deps_ok = deps.iter().all(|dep| {
                step_statuses
                    .get(dep)
                    .map(|s| s.status == AcquisitionStepStatus::Success)
                    .unwrap_or(false)
            });

            if !deps_ok && !deps.is_empty() {
                let reason = "Dependency failed".to_string();
                if let Some(tx) = progress {
                    tx.step_skipped(&step.name, &workspace.name, ExecutionPhase::Acquisition, &reason);
                }

                step_statuses.insert(
                    step.name.clone(),
                    StepExecutionStatus {
                        name: step.name.clone(),
                        status: AcquisitionStepStatus::Skipped,
                        row_count: None,
                        duration_ms: 0,
                        error: Some(reason),
                    },
                );
                continue;
            }

            // Check `when` condition
            if let Some(when_condition) = &step.when {
                let condition_met = match evaluate_condition(when_condition, ctx.evaluation()) {
                    Ok(met) => met,
                    Err(e) => {
                        warn!(
                            "Step '{}' when='{}' evaluation failed: {}",
                            step.name, when_condition, e
                        );
                        false
                    }
                };

                debug!(
                    "Step '{}' when='{}' evaluated to: {}",
                    step.name, when_condition, condition_met
                );

                if !condition_met {
                    let reason = format!("Condition not met: {}", when_condition);
                    if let Some(tx) = progress {
                        tx.step_skipped(&step.name, &workspace.name, ExecutionPhase::Acquisition, &reason);
                    }

                    step_statuses.insert(
                        step.name.clone(),
                        StepExecutionStatus {
                            name: step.name.clone(),
                            status: AcquisitionStepStatus::Skipped,
                            row_count: None,
                            duration_ms: 0,
                            error: Some(reason),
                        },
                    );
                    continue;
                }
            }

            // Emit step started
            if let Some(tx) = progress {
                tx.step_started(&step.name, &workspace.name, ExecutionPhase::Acquisition);
            }

            debug!(
                step = %step.name,
                step_type = ?step.step_type,
                "Starting step execution"
            );

            // Execute step
            // Note: for_each iteration is now handled at the HTTP step level,
            // not at the phase orchestration level
            let result = self.execute_single_step(step, &mut ctx).await;

            match result {
                Ok(output) => {
                    let duration_ms = output.duration_ms();
                    let row_count = output.row_count();

                    // Emit completed
                    if let Some(tx) = progress {
                        tx.step_completed(&step.name, &workspace.name, ExecutionPhase::Acquisition, row_count, duration_ms);
                    }

                    // Register result for downstream steps
                    ctx.register_result(&step.name, output.into_handle());

                    step_statuses.insert(
                        step.name.clone(),
                        StepExecutionStatus {
                            name: step.name.clone(),
                            status: AcquisitionStepStatus::Success,
                            row_count: Some(row_count),
                            duration_ms,
                            error: None,
                        },
                    );

                    info!(
                        message = "step.completed",
                        step = %step.name,
                        rows = row_count,
                        duration_ms = duration_ms,
                    );
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    let on_error = step.on_error.unwrap_or_default();

                    match on_error {
                        OnError::Fail => {
                            if let Some(tx) = progress {
                                tx.step_failed(&step.name, &workspace.name, ExecutionPhase::Acquisition, &error_msg);
                            }

                            failed_step = Some(step.name.clone());
                            failure_reason = Some(error_msg.clone());

                            step_statuses.insert(
                                step.name.clone(),
                                StepExecutionStatus {
                                    name: step.name.clone(),
                                    status: AcquisitionStepStatus::Failed,
                                    row_count: None,
                                    duration_ms: 0,
                                    error: Some(error_msg),
                                },
                            );

                            // Stop execution on failure
                            break;
                        }
                        OnError::Skip | OnError::Continue => {
                            info!(
                                "Step '{}' error ignored due to on_error setting: {}",
                                step.name, error_msg
                            );

                            if let Some(tx) = progress {
                                tx.step_skipped(
                                    &step.name,
                                    &workspace.name,
                                    ExecutionPhase::Acquisition,
                                    format!("Error ignored: {}", error_msg),
                                );
                            }

                            // Create empty placeholder
                            if let Ok(writer) = ctx.writer(&step.name) {
                                if let Ok(handle) = writer.finish() {
                                    ctx.register_result(&step.name, handle);
                                }
                            }

                            step_statuses.insert(
                                step.name.clone(),
                                StepExecutionStatus {
                                    name: step.name.clone(),
                                    status: AcquisitionStepStatus::Skipped,
                                    row_count: Some(0),
                                    duration_ms: 0,
                                    error: Some(format!("Error ignored: {}", error_msg)),
                                },
                            );
                        }
                    }
                }
            }
        }

        Ok(AcquisitionPhaseOutput {
            results: ctx.take_results(),
            step_statuses,
            duration: phase_start.elapsed(),
            failed_step,
            failure_reason,
        })
    }

    /// Execute a single step (no foreach)
    async fn execute_single_step(
        &self,
        step: &Step,
        ctx: &mut AcquisitionContext<'_>,
    ) -> Result<AcquisitionStepOutput> {
        let handler = self.get_handler(step.step_type).ok_or_else(|| {
            Error::execution(format!("No handler for step type {:?}", step.step_type))
        })?;

        handler.execute(step, ctx).await
    }

    /// Validate all steps in the acquisition config
    pub fn validate(&self, acquisition: &Acquisition) -> Result<()> {
        for step in &acquisition.steps {
            let handler = self.get_handler(step.step_type).ok_or_else(|| {
                Error::pack(format!(
                    "No handler for step type {:?} in step '{}'",
                    step.step_type, step.name
                ))
            })?;

            handler.validate(step)?;
        }

        Ok(())
    }
}

impl Default for AcquisitionPhaseHandler {
    fn default() -> Self {
        Self::new()
    }
}
