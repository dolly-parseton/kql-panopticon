//! Reporting phase handler
//!
//! Orchestrates generation of reports with lazy data access.

use crate::error::{Error, Result};
use crate::execution::progress::{ExecutionPhase, ProgressSender};
use crate::execution::result::ResultContext;
use crate::pack::Reporting;
use crate::workspace::Workspace;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::{debug, info};

use super::context::{ReportMetadata, ReportingContext};
use super::handler::{ReportingStepHandler, ReportingStepType};
use super::output::GeneratedReport;

/// Phase-level output from reporting
#[derive(Debug)]
pub struct ReportingPhaseOutput {
    /// Generated reports
    pub reports: Vec<GeneratedReport>,

    /// Per-report execution status
    pub report_statuses: HashMap<String, ReportingStepStatus>,

    /// Total duration
    pub duration: Duration,

    /// First failed report (if any)
    pub failed_report: Option<String>,

    /// Failure reason
    pub failure_reason: Option<String>,
}

/// Status of a report generation
#[derive(Debug, Clone)]
pub struct ReportingStepStatus {
    pub name: String,
    pub status: ReportingStatus,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// Report generation status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportingStatus {
    Success,
    Failed,
    Skipped,
}

/// Configuration for reporting phase execution
#[derive(Debug)]
pub struct ReportingConfig<'a> {
    /// Reporting configuration from pack
    pub reporting: &'a Reporting,
    /// Pack name for metadata
    pub pack_name: &'a str,
    /// Workspace being reported on
    pub workspace: &'a Workspace,
    /// User inputs for template substitution
    pub inputs: &'a HashMap<String, String>,
    /// Results from acquisition phase
    pub acquisition_results: &'a ResultContext,
    /// Results from processing phase (optional)
    pub processing_results: Option<&'a ResultContext>,
    /// Output directory for generated reports
    pub output_dir: &'a Path,
    /// Path to pack file (for relative template resolution)
    pub pack_path: Option<&'a Path>,
}

impl<'a> ReportingConfig<'a> {
    /// Create a new reporting config with required fields
    pub fn new(
        reporting: &'a Reporting,
        pack_name: &'a str,
        workspace: &'a Workspace,
        inputs: &'a HashMap<String, String>,
        acquisition_results: &'a ResultContext,
        output_dir: &'a Path,
    ) -> Self {
        Self {
            reporting,
            pack_name,
            workspace,
            inputs,
            acquisition_results,
            processing_results: None,
            output_dir,
            pack_path: None,
        }
    }

    /// Add processing results
    pub fn with_processing_results(mut self, results: &'a ResultContext) -> Self {
        self.processing_results = Some(results);
        self
    }

    /// Add pack path for template resolution
    pub fn with_pack_path(mut self, path: &'a Path) -> Self {
        self.pack_path = Some(path);
        self
    }
}

/// Reporting phase handler
///
/// Coordinates generation of all reports with lazy data access.
pub struct ReportingPhaseHandler {
    handlers: HashMap<ReportingStepType, Box<dyn ReportingStepHandler>>,
}

impl ReportingPhaseHandler {
    /// Create a new phase handler
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a step handler
    pub fn register(&mut self, handler: Box<dyn ReportingStepHandler>) {
        self.handlers.insert(handler.handles(), handler);
    }

    /// Get handler for a step type
    fn get_handler(&self, step_type: ReportingStepType) -> Option<&dyn ReportingStepHandler> {
        self.handlers.get(&step_type).map(|h| h.as_ref())
    }

    /// Execute the reporting phase
    ///
    /// Takes ResultContext references for lazy data access. Data is only
    /// materialized when a report actually needs to render.
    pub async fn execute(
        &self,
        config: ReportingConfig<'_>,
        progress: Option<&ProgressSender>,
    ) -> Result<ReportingPhaseOutput> {
        let phase_start = Instant::now();
        let mut reports: Vec<GeneratedReport> = Vec::new();
        let mut report_statuses: HashMap<String, ReportingStepStatus> = HashMap::new();
        let mut failed_report = None;
        let mut failure_reason = None;

        // Create metadata
        let metadata = ReportMetadata::new(config.pack_name, config.workspace);

        // Create context with lazy result access
        let ctx = ReportingContext::new(
            config.acquisition_results,
            config.processing_results,
            config.inputs,
            metadata,
            config.output_dir,
            config.pack_path,
        );

        let reporting = config.reporting;

        debug!(
            phase = "reporting",
            reports = reporting.reports.len(),
            "Starting reporting phase"
        );

        // Execute each report
        for report in &reporting.reports {
            // Check `when` condition using lazy context access
            if let Some(when_condition) = &report.when {
                let condition_met = evaluate_report_condition(when_condition, &ctx);

                debug!(
                    "Report '{}' when='{}' evaluated to: {}",
                    report.name, when_condition, condition_met
                );

                if !condition_met {
                    let reason = format!("Condition not met: {}", when_condition);
                    if let Some(tx) = progress {
                        tx.step_skipped(&report.name, "", ExecutionPhase::Reporting, &reason);
                    }

                    report_statuses.insert(
                        report.name.clone(),
                        ReportingStepStatus {
                            name: report.name.clone(),
                            status: ReportingStatus::Skipped,
                            duration_ms: 0,
                            error: Some(reason),
                        },
                    );
                    continue;
                }
            }

            // Emit report started
            if let Some(tx) = progress {
                tx.step_started(&report.name, "", ExecutionPhase::Reporting);
            }

            debug!(report = %report.name, "Starting report generation");

            // Get handler (all reports use Template handler for now)
            let handler = match self.get_handler(ReportingStepType::Template) {
                Some(h) => h,
                None => {
                    let error_msg = "No template handler registered".to_string();
                    failed_report = Some(report.name.clone());
                    failure_reason = Some(error_msg.clone());

                    report_statuses.insert(
                        report.name.clone(),
                        ReportingStepStatus {
                            name: report.name.clone(),
                            status: ReportingStatus::Failed,
                            duration_ms: 0,
                            error: Some(error_msg),
                        },
                    );
                    break;
                }
            };

            // Execute report generation
            let result = handler.execute(report, &ctx).await;

            match result {
                Ok(output) => {
                    let duration_ms = output.duration_ms();

                    // Emit completed
                    if let Some(tx) = progress {
                        tx.step_completed(&report.name, "", ExecutionPhase::Reporting, 0, duration_ms);
                    }

                    // Store result
                    reports.push(output.clone().into());

                    report_statuses.insert(
                        report.name.clone(),
                        ReportingStepStatus {
                            name: report.name.clone(),
                            status: ReportingStatus::Success,
                            duration_ms,
                            error: None,
                        },
                    );

                    info!(
                        message = "report.generated",
                        report = %report.name,
                        path = ?output.path(),
                        bytes = output.byte_count(),
                        duration_ms = duration_ms,
                    );
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    // Log the error prominently
                    tracing::error!(
                        report = %report.name,
                        error = %error_msg,
                        "Report generation failed"
                    );

                    if let Some(tx) = progress {
                        tx.step_failed(&report.name, "", ExecutionPhase::Reporting, &error_msg);
                    }

                    // Reports default to continue on error (don't fail whole execution)
                    report_statuses.insert(
                        report.name.clone(),
                        ReportingStepStatus {
                            name: report.name.clone(),
                            status: ReportingStatus::Failed,
                            duration_ms: 0,
                            error: Some(error_msg.clone()),
                        },
                    );

                    // Track first failure but continue with other reports
                    if failed_report.is_none() {
                        failed_report = Some(report.name.clone());
                        failure_reason = Some(error_msg);
                    }
                }
            }
        }

        Ok(ReportingPhaseOutput {
            reports,
            report_statuses,
            duration: phase_start.elapsed(),
            failed_report,
            failure_reason,
        })
    }

    /// Validate all reports in the reporting config
    pub fn validate(&self, reporting: &Reporting) -> Result<()> {
        for report in &reporting.reports {
            let handler = self.get_handler(ReportingStepType::Template).ok_or_else(|| {
                Error::pack(format!(
                    "No handler for report '{}'",
                    report.name
                ))
            })?;

            handler.validate(report)?;
        }

        Ok(())
    }
}

impl Default for ReportingPhaseHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluate a report condition using the reporting context
///
/// Supports simple conditions like:
/// - `processing.risk_score.score > 30`
/// - `acquisition.signins is not empty`
/// - `signins is not empty`
fn evaluate_report_condition(condition: &str, ctx: &ReportingContext<'_>) -> bool {
    let condition = condition.trim();

    // Check for "is empty" / "is not empty"
    if condition.ends_with("is empty") {
        let path = condition.trim_end_matches("is empty").trim();
        return ctx.is_empty(path);
    }

    if condition.ends_with("is not empty") {
        let path = condition.trim_end_matches("is not empty").trim();
        return !ctx.is_empty(path);
    }

    // Check for comparison operators
    for op in [">=", "<=", ">", "<", "==", "!="] {
        if let Some(pos) = condition.find(op) {
            let left = condition[..pos].trim();
            let right = condition[pos + op.len()..].trim();

            let left_val = ctx.get_value(left);

            if let Some(left_val) = left_val {
                return evaluate_comparison(&left_val, op, right);
            }

            return false;
        }
    }

    // Unknown condition format, default to true
    debug!("Unknown condition format: '{}', defaulting to true", condition);
    true
}

/// Evaluate a comparison operation
fn evaluate_comparison(left: &JsonValue, op: &str, right: &str) -> bool {
    // Try to parse right as number
    let right_num: Option<f64> = right.trim().parse().ok();

    match (left, right_num) {
        (JsonValue::Number(n), Some(rn)) => {
            let ln = n.as_f64().unwrap_or(0.0);
            match op {
                ">" => ln > rn,
                "<" => ln < rn,
                ">=" => ln >= rn,
                "<=" => ln <= rn,
                "==" => (ln - rn).abs() < f64::EPSILON,
                "!=" => (ln - rn).abs() >= f64::EPSILON,
                _ => false,
            }
        }
        (JsonValue::String(s), _) => {
            let right_str = right.trim().trim_matches('"').trim_matches('\'');
            match op {
                "==" => s == right_str,
                "!=" => s != right_str,
                _ => false,
            }
        }
        _ => false,
    }
}
