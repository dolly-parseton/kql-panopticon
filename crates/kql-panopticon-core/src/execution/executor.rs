//! Pack executor for phase-based execution
//!
//! Orchestrates pack execution through three phases:
//! 1. Acquisition - Data collection (per workspace)
//! 2. Processing - Data transformation (global)
//! 3. Reporting - Output generation (global)

use crate::client::Client;
use crate::error::Result;
use crate::pack::Pack;
use crate::workspace::Workspace;

use super::acquisition::steps::{FileStepHandler, HttpStepHandler, KqlStepHandler};
use super::acquisition::AcquisitionPhaseHandler;
use super::processing::steps::ScoringStepHandler;
use super::processing::ProcessingPhaseHandler;
use super::progress::{JobType, ProgressSender};
use super::reporting::steps::TemplateStepHandler;
use super::reporting::ReportingPhaseHandler;
use super::result::ResultContext;
use super::trace::{ExecutionTrace, TraceStatus};
use super::types::{
    ExecutionPhase, ExecutionStatus, PackExecutorConfig, PackExecutorResult, StepResult,
    WorkspaceResult,
};

use chrono::Local;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
use tracing::{debug, info};

/// Execution mode for the pack
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    /// Normal execution
    #[default]
    Normal,
    /// Dry run - validate but don't execute
    DryRun,
    /// Debug mode - extra logging
    Debug,
}

/// Execution options
#[derive(Debug, Clone)]
pub struct ExecutionOptions {
    /// Execution mode
    pub mode: ExecutionMode,
    /// Enable execution tracing
    pub trace: bool,
    /// Default timeout for steps
    pub timeout: Duration,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            mode: ExecutionMode::Normal,
            trace: false,
            timeout: Duration::from_secs(120),
        }
    }
}

/// Pack executor - coordinates phase-based execution
///
/// Executes packs through three phases:
/// 1. **Acquisition**: Per-workspace data collection
/// 2. **Processing**: Global data transformation
/// 3. **Reporting**: Global report generation
pub struct PackExecutor {
    /// Acquisition phase handler
    acquisition: AcquisitionPhaseHandler,

    /// Processing phase handler
    processing: ProcessingPhaseHandler,

    /// Reporting phase handler
    reporting: ReportingPhaseHandler,

    /// Execution options
    options: ExecutionOptions,
}

impl PackExecutor {
    /// Create a new executor with the given client
    pub fn new(client: Client) -> Self {
        let client = Arc::new(client);

        // Initialize acquisition phase with handlers
        let mut acquisition = AcquisitionPhaseHandler::new();
        acquisition.register(Box::new(KqlStepHandler::new(client)));
        acquisition.register(Box::new(HttpStepHandler::new()));
        acquisition.register(Box::new(FileStepHandler::new()));

        // Initialize processing phase with handlers
        let mut processing = ProcessingPhaseHandler::new();
        processing.register(Box::new(ScoringStepHandler::new()));

        // Initialize reporting phase with handlers
        let mut reporting = ReportingPhaseHandler::new();
        reporting.register(Box::new(TemplateStepHandler::new()));

        Self {
            acquisition,
            processing,
            reporting,
            options: ExecutionOptions::default(),
        }
    }

    /// Create with custom options
    pub fn with_options(client: Client, options: ExecutionOptions) -> Self {
        let mut executor = Self::new(client);
        executor.options = options;
        executor
    }

    /// Set execution options
    pub fn set_options(&mut self, options: ExecutionOptions) {
        self.options = options;
    }

    /// Set default timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = timeout;
        self
    }

    /// Validate a pack configuration
    ///
    /// Validates all phases (acquisition, processing, reporting) to ensure
    /// the pack can be executed successfully.
    pub fn validate(&self, config: &PackExecutorConfig) -> Result<()> {
        let pack = &config.pack;

        // Validate acquisition phase
        self.acquisition.validate(&pack.acquisition)?;

        // Validate processing phase (if present)
        if let Some(ref processing) = pack.processing {
            self.processing.validate(processing)?;
        }

        // Validate reporting phase (if present)
        if let Some(ref reporting) = pack.reporting {
            self.reporting.validate(reporting)?;
        }

        Ok(())
    }

    /// Execute a pack across workspaces
    pub async fn execute(
        &self,
        config: PackExecutorConfig,
        workspaces: Vec<Workspace>,
        progress: Option<ProgressSender>,
    ) -> Result<PackExecutorResult> {
        let job_id = uuid::Uuid::new_v4();
        let start = Instant::now();
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();

        let pack = &config.pack;
        let mut workspace_results: HashMap<String, WorkspaceResult> = HashMap::new();
        let mut overall_status = ExecutionStatus::Success;
        let mut trace = if self.options.trace {
            Some(ExecutionTrace::new())
        } else {
            None
        };

        info!(
            job_id = %job_id,
            pack = %pack.name,
            workspaces = workspaces.len(),
            "Starting pack execution"
        );

        // Emit job started
        if let Some(ref tx) = progress {
            tx.started(JobType::Investigation, pack.acquisition.steps.len(), workspaces.len());
        }

        // Execute per workspace
        for workspace in &workspaces {
            debug!(
                workspace = %workspace.name,
                workspace_id = %workspace.workspace_id,
                "Starting workspace execution"
            );

            // Emit workspace started
            if let Some(ref tx) = progress {
                tx.workspace_started(&workspace.name);
            }

            let ws_result = self
                .execute_workspace(pack, workspace, &config, &timestamp, progress.as_ref())
                .await;

            let ws_status = ws_result.status;

            // Emit workspace completed
            if let Some(ref tx) = progress {
                tx.workspace_completed(&workspace.name, ws_result.duration_ms);
            }

            // Update overall status
            if ws_status == ExecutionStatus::Failed {
                overall_status = ExecutionStatus::Partial;
            }

            workspace_results.insert(workspace.name.clone(), ws_result);
        }

        // Determine final status
        let all_failed = workspace_results
            .values()
            .all(|r| r.status == ExecutionStatus::Failed);
        let all_succeeded = workspace_results
            .values()
            .all(|r| r.status == ExecutionStatus::Success);

        overall_status = if all_failed {
            ExecutionStatus::Failed
        } else if all_succeeded {
            ExecutionStatus::Success
        } else {
            ExecutionStatus::Partial
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        // Emit job completed
        if let Some(ref tx) = progress {
            tx.completed(duration_ms);
        }

        info!(
            job_id = %job_id,
            status = ?overall_status,
            duration_ms = duration_ms,
            "Pack execution completed"
        );

        // Finalize trace
        if let Some(ref mut t) = trace {
            t.set_status(match overall_status {
                ExecutionStatus::Success => TraceStatus::Success,
                ExecutionStatus::Failed => TraceStatus::Failed,
                ExecutionStatus::Partial => TraceStatus::Partial,
                ExecutionStatus::Pending | ExecutionStatus::Running => TraceStatus::Running,
            });
        }

        // Build output directory for result
        let output_dir = config.output_dir.clone();

        Ok(PackExecutorResult {
            job_id,
            pack_name: pack.name.clone(),
            status: overall_status,
            workspace_results,
            duration_ms,
            output_dir,
            trace,
        })
    }

    /// Execute pack for a single workspace
    async fn execute_workspace(
        &self,
        pack: &Pack,
        workspace: &Workspace,
        config: &PackExecutorConfig,
        timestamp: &str,
        progress: Option<&ProgressSender>,
    ) -> WorkspaceResult {
        let start = Instant::now();

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
                step_results: HashMap::new(),
                step_handles: ResultContext::new(),
                duration_ms: start.elapsed().as_millis() as u64,
                failed_step: None,
                failure_reason: Some(format!("Failed to create output directory: {}", e)),
            };
        }

        // Phase 1: Acquisition
        debug!(phase = "acquisition", "Starting acquisition phase");

        let acq_output = self
            .acquisition
            .execute(
                pack,
                workspace,
                config.inputs.clone(),
                &workspace_output_dir,
                progress,
            )
            .await;

        let acq_output = match acq_output {
            Ok(output) => output,
            Err(e) => {
                return WorkspaceResult {
                    workspace_name: workspace.name.clone(),
                    workspace_id: workspace.workspace_id.clone(),
                    status: ExecutionStatus::Failed,
                    step_results: HashMap::new(),
                    step_handles: ResultContext::new(),
                    duration_ms: start.elapsed().as_millis() as u64,
                    failed_step: None,
                    failure_reason: Some(format!("Acquisition phase failed: {}", e)),
                };
            }
        };

        // Check if acquisition had a failure
        if acq_output.failed_step.is_some() {
            return WorkspaceResult {
                workspace_name: workspace.name.clone(),
                workspace_id: workspace.workspace_id.clone(),
                status: ExecutionStatus::Failed,
                step_results: convert_step_statuses(&acq_output.step_statuses),
                step_handles: acq_output.results,
                duration_ms: start.elapsed().as_millis() as u64,
                failed_step: acq_output.failed_step,
                failure_reason: acq_output.failure_reason,
            };
        }

        // Phase 2: Processing (if configured)
        let proc_output = if let Some(ref processing) = pack.processing {
            if !processing.is_empty() {
                debug!(phase = "processing", "Starting processing phase");

                let output = self
                    .processing
                    .execute(processing, &acq_output.results, &workspace_output_dir, progress)
                    .await;

                match output {
                    Ok(o) => Some(o),
                    Err(e) => {
                        return WorkspaceResult {
                            workspace_name: workspace.name.clone(),
                            workspace_id: workspace.workspace_id.clone(),
                            status: ExecutionStatus::Failed,
                            step_results: convert_step_statuses(&acq_output.step_statuses),
                            step_handles: acq_output.results,
                            duration_ms: start.elapsed().as_millis() as u64,
                            failed_step: None,
                            failure_reason: Some(format!("Processing phase failed: {}", e)),
                        };
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        // Phase 3: Reporting (if configured)
        let rep_output = if let Some(ref reporting) = pack.reporting {
            if !reporting.is_empty() {
                debug!(phase = "reporting", "Starting reporting phase");

                // Pass ResultContext refs for lazy materialization
                match self
                    .reporting
                    .execute(
                        reporting,
                        &pack.name,
                        workspace,
                        &config.inputs,
                        &acq_output.results,
                        proc_output.as_ref().map(|p| &p.results),
                        &workspace_output_dir,
                        config.pack_path.as_deref(),
                        progress,
                    )
                    .await
                {
                    Ok(output) => Some(output),
                    Err(e) => {
                        // Reporting failures don't fail the workspace (logged but continued)
                        tracing::warn!(error = %e, "Reporting phase failed");
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        // Build final step results
        let mut step_results = convert_step_statuses(&acq_output.step_statuses);

        // Add processing step results and merge handles
        let mut step_handles = acq_output.results;
        if let Some(proc) = proc_output {
            // Merge processing result handles
            step_handles.merge(proc.results);

            // Add processing step statuses
            for (name, status) in &proc.step_statuses {
                step_results.insert(
                    name.clone(),
                    StepResult {
                        name: name.clone(),
                        phase: ExecutionPhase::Processing,
                        status: match status.status {
                            super::processing::ProcessingStatus::Success => {
                                super::types::StepStatus::Success
                            }
                            super::processing::ProcessingStatus::Failed => {
                                super::types::StepStatus::Failed
                            }
                            super::processing::ProcessingStatus::Skipped => {
                                super::types::StepStatus::Skipped
                            }
                        },
                        row_count: None,
                        duration_ms: status.duration_ms,
                        output_path: None,
                        error: status.error.clone(),
                    },
                );
            }
        }

        // Add reporting step results
        if let Some(rep) = rep_output {
            for (name, status) in &rep.report_statuses {
                step_results.insert(
                    name.clone(),
                    StepResult {
                        name: name.clone(),
                        phase: ExecutionPhase::Reporting,
                        status: match status.status {
                            super::reporting::ReportingStatus::Success => {
                                super::types::StepStatus::Success
                            }
                            super::reporting::ReportingStatus::Failed => {
                                super::types::StepStatus::Failed
                            }
                            super::reporting::ReportingStatus::Skipped => {
                                super::types::StepStatus::Skipped
                            }
                        },
                        row_count: None,
                        duration_ms: status.duration_ms,
                        output_path: None,
                        error: status.error.clone(),
                    },
                );
            }
        }

        WorkspaceResult {
            workspace_name: workspace.name.clone(),
            workspace_id: workspace.workspace_id.clone(),
            status: ExecutionStatus::Success,
            step_results,
            step_handles,
            duration_ms: start.elapsed().as_millis() as u64,
            failed_step: None,
            failure_reason: None,
        }
    }
}

/// Convert acquisition step statuses to StepResult map
fn convert_step_statuses(
    statuses: &HashMap<String, super::acquisition::StepExecutionStatus>,
) -> HashMap<String, StepResult> {
    statuses
        .iter()
        .map(|(name, status)| {
            (
                name.clone(),
                StepResult {
                    name: name.clone(),
                    phase: ExecutionPhase::Acquisition,
                    status: match status.status {
                        super::acquisition::AcquisitionStepStatus::Success => super::types::StepStatus::Success,
                        super::acquisition::AcquisitionStepStatus::Failed => super::types::StepStatus::Failed,
                        super::acquisition::AcquisitionStepStatus::Skipped => super::types::StepStatus::Skipped,
                    },
                    row_count: status.row_count,
                    duration_ms: status.duration_ms,
                    output_path: None,
                    error: status.error.clone(),
                },
            )
        })
        .collect()
}
