//! Processing phase handler
//!
//! Orchestrates execution of processing steps.

use crate::error::{Error, Result};
use crate::execution::progress::{ExecutionPhase, ProgressSender};
use crate::execution::result::ResultContext;
use crate::pack::{Processing, ProcessingStepConfig};
use crate::variable::evaluate_condition;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::{debug, info};

use super::context::ProcessingContext;
use super::handler::{ProcessingStepHandler, ProcessingStepType};

/// Phase-level output from processing
#[derive(Debug)]
pub struct ProcessingPhaseOutput {
    /// Processing results keyed by step name (file-backed via ResultContext)
    pub results: ResultContext,

    /// Per-step execution status
    pub step_statuses: HashMap<String, ProcessingStepStatus>,

    /// Total duration
    pub duration: Duration,

    /// First failed step (if any)
    pub failed_step: Option<String>,

    /// Failure reason
    pub failure_reason: Option<String>,
}

/// Status of a processing step execution
#[derive(Debug, Clone)]
pub struct ProcessingStepStatus {
    pub name: String,
    pub status: ProcessingStatus,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// Processing step execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingStatus {
    Success,
    Failed,
    Skipped,
}

/// Processing phase handler
///
/// Coordinates execution of all processing steps.
pub struct ProcessingPhaseHandler {
    handlers: HashMap<ProcessingStepType, Box<dyn ProcessingStepHandler>>,
}

impl ProcessingPhaseHandler {
    /// Create a new phase handler
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a step handler
    pub fn register(&mut self, handler: Box<dyn ProcessingStepHandler>) {
        self.handlers.insert(handler.handles(), handler);
    }

    /// Get handler for a step type
    fn get_handler(&self, step_type: ProcessingStepType) -> Option<&dyn ProcessingStepHandler> {
        self.handlers.get(&step_type).map(|h| h.as_ref())
    }

    /// Get step type from config
    fn step_type_from_config(config: &ProcessingStepConfig) -> ProcessingStepType {
        match config {
            ProcessingStepConfig::Scoring(_) => ProcessingStepType::Scoring,
        }
    }

    /// Execute the processing phase
    pub async fn execute(
        &self,
        processing: &Processing,
        acquisition_results: &ResultContext,
        output_dir: &Path,
        progress: Option<&ProgressSender>,
    ) -> Result<ProcessingPhaseOutput> {
        let phase_start = Instant::now();
        let mut step_statuses: HashMap<String, ProcessingStepStatus> = HashMap::new();
        let mut failed_step = None;
        let mut failure_reason = None;

        // Create context with output directory for writing results
        let mut ctx = ProcessingContext::new(acquisition_results, output_dir);

        debug!(
            phase = "processing",
            steps = processing.steps.len(),
            "Starting processing phase"
        );

        // Execute each step
        for step in &processing.steps {
            // Check `when` condition
            if let Some(when_condition) = &step.when {
                let condition_met =
                    evaluate_condition(when_condition, acquisition_results);

                debug!(
                    "Processing step '{}' when='{}' evaluated to: {}",
                    step.name, when_condition, condition_met
                );

                if !condition_met {
                    let reason = format!("Condition not met: {}", when_condition);
                    if let Some(tx) = progress {
                        tx.step_skipped(&step.name, "", ExecutionPhase::Processing, &reason);
                    }

                    step_statuses.insert(
                        step.name.clone(),
                        ProcessingStepStatus {
                            name: step.name.clone(),
                            status: ProcessingStatus::Skipped,
                            duration_ms: 0,
                            error: Some(reason),
                        },
                    );
                    continue;
                }
            }

            // Emit step started
            if let Some(tx) = progress {
                tx.step_started(&step.name, "", ExecutionPhase::Processing);
            }

            debug!(step = %step.name, "Starting processing step execution");

            // Get handler for this step type
            let step_type = Self::step_type_from_config(&step.config);
            let handler = match self.get_handler(step_type) {
                Some(h) => h,
                None => {
                    let error_msg = format!("No handler for step type {:?}", step_type);
                    failed_step = Some(step.name.clone());
                    failure_reason = Some(error_msg.clone());

                    step_statuses.insert(
                        step.name.clone(),
                        ProcessingStepStatus {
                            name: step.name.clone(),
                            status: ProcessingStatus::Failed,
                            duration_ms: 0,
                            error: Some(error_msg),
                        },
                    );
                    break;
                }
            };

            // Execute step
            let result = handler.execute(step, &ctx).await;

            match result {
                Ok(output) => {
                    let duration_ms = output.duration_ms();

                    // Emit completed
                    if let Some(tx) = progress {
                        tx.step_completed(&step.name, "", ExecutionPhase::Processing, 0, duration_ms);
                    }

                    // Register result handle in context
                    ctx.register_result(&step.name, output.into_handle());

                    step_statuses.insert(
                        step.name.clone(),
                        ProcessingStepStatus {
                            name: step.name.clone(),
                            status: ProcessingStatus::Success,
                            duration_ms,
                            error: None,
                        },
                    );

                    info!(
                        message = "processing_step.completed",
                        step = %step.name,
                        duration_ms = duration_ms,
                    );
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    if let Some(tx) = progress {
                        tx.step_failed(&step.name, "", ExecutionPhase::Processing, &error_msg);
                    }

                    failed_step = Some(step.name.clone());
                    failure_reason = Some(error_msg.clone());

                    step_statuses.insert(
                        step.name.clone(),
                        ProcessingStepStatus {
                            name: step.name.clone(),
                            status: ProcessingStatus::Failed,
                            duration_ms: 0,
                            error: Some(error_msg),
                        },
                    );

                    // Processing failures stop the phase
                    break;
                }
            }
        }

        Ok(ProcessingPhaseOutput {
            results: ctx.take_results(),
            step_statuses,
            duration: phase_start.elapsed(),
            failed_step,
            failure_reason,
        })
    }

    /// Validate all steps in the processing config
    pub fn validate(&self, processing: &Processing) -> Result<()> {
        for step in &processing.steps {
            let step_type = Self::step_type_from_config(&step.config);
            let handler = self.get_handler(step_type).ok_or_else(|| {
                Error::pack(format!(
                    "No handler for step type {:?} in step '{}'",
                    step_type, step.name
                ))
            })?;

            handler.validate(step)?;
        }

        Ok(())
    }
}

impl Default for ProcessingPhaseHandler {
    fn default() -> Self {
        Self::new()
    }
}
