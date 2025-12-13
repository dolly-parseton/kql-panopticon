//! Progress display for execution monitoring
//!
//! Collects progress updates into formatted output.
//! Also provides a TUI forwarder that converts progress updates to TuiEvents.

use kql_panopticon_core::execution::progress::{
    progress_channel, ProgressReceiver, ProgressSender, ProgressUpdate,
};
use uuid::Uuid;

#[cfg(feature = "tui")]
use crate::tui::events::{TuiEvent, TuiEventSender};

/// Create a progress channel for execution
pub fn create_progress_channel() -> (ProgressSender, ProgressReceiver, Uuid) {
    let job_id = Uuid::new_v4();
    let (sender, receiver) = progress_channel(job_id);
    (sender, receiver, job_id)
}

/// Collected progress output
#[derive(Debug, Default)]
pub struct ProgressOutput {
    /// Lines of progress output
    pub lines: Vec<String>,
}

impl ProgressOutput {
    /// Convert to a single string for display
    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }
}

/// Run progress display and collect output
///
/// Collects step progress as formatted lines for later display.
pub async fn run_progress_display(
    _step_names: Vec<String>,
    workspace: String,
    mut receiver: ProgressReceiver,
) -> ProgressOutput {
    let mut output = ProgressOutput::default();
    output.lines.push(format!("Running on {}...", workspace));

    // Process updates until complete
    while let Some(update) = receiver.recv().await {
        match &update {
            ProgressUpdate::StepStarted { step_name, .. } => {
                output.lines.push(format!("  ○ {} running...", step_name));
            }
            ProgressUpdate::StepCompleted {
                step_name,
                rows,
                duration_ms,
                ..
            } => {
                // Update the last line if it was the "running" line for this step
                if let Some(last) = output.lines.last_mut() {
                    if last.contains(&format!("{} running...", step_name)) {
                        *last = format!("  ✓ {} - {} rows ({}ms)", step_name, rows, duration_ms);
                        continue;
                    }
                }
                output.lines.push(format!("  ✓ {} - {} rows ({}ms)", step_name, rows, duration_ms));
            }
            ProgressUpdate::StepFailed {
                step_name, error, ..
            } => {
                // Update the last line if it was the "running" line for this step
                if let Some(last) = output.lines.last_mut() {
                    if last.contains(&format!("{} running...", step_name)) {
                        *last = format!("  ✗ {} - {}", step_name, error);
                        continue;
                    }
                }
                output.lines.push(format!("  ✗ {} - {}", step_name, error));
            }
            ProgressUpdate::StepSkipped {
                step_name, reason, ..
            } => {
                output.lines.push(format!("  ○ {} - skipped: {}", step_name, reason));
            }
            ProgressUpdate::Completed { .. } | ProgressUpdate::Failed { .. } => {
                break;
            }
            _ => {}
        }
    }

    output
}

/// Forward progress updates to the TUI as TuiEvents
///
/// This function runs in a background task and converts ProgressUpdate
/// messages to TuiEvents, sending them to the App for real-time UI updates.
///
/// # Arguments
/// * `pack_name` - Name of the pack being executed
/// * `step_names` - Names of steps in execution order
/// * `receiver` - Progress updates from the executor
/// * `event_tx` - TuiEvent sender to the App
#[cfg(feature = "tui")]
pub async fn run_tui_progress_forwarder(
    pack_name: String,
    step_names: Vec<String>,
    mut receiver: ProgressReceiver,
    event_tx: TuiEventSender,
) {
    let mut step_indices: std::collections::HashMap<String, usize> = step_names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.clone(), i))
        .collect();

    let mut start_time = std::time::Instant::now();

    while let Some(update) = receiver.recv().await {
        let event = match update {
            ProgressUpdate::Started { job_id, total_steps, .. } => {
                start_time = std::time::Instant::now();
                Some(TuiEvent::ExecutionStarted {
                    job_id,
                    pack_name: pack_name.clone(),
                    step_count: total_steps,
                })
            }

            ProgressUpdate::StepStarted { job_id, step_name, .. } => {
                let step_index = step_indices.get(&step_name).copied().unwrap_or(0);
                Some(TuiEvent::StepStarted {
                    job_id,
                    step_name,
                    step_index,
                })
            }

            ProgressUpdate::StepCompleted { job_id, step_name, rows, duration_ms, .. } => {
                let step_index = step_indices.get(&step_name).copied().unwrap_or(0);
                Some(TuiEvent::StepCompleted {
                    job_id,
                    step_name,
                    step_index,
                    row_count: rows,
                    duration_ms,
                })
            }

            ProgressUpdate::StepFailed { job_id, step_name, error, .. } => {
                let step_index = step_indices.get(&step_name).copied().unwrap_or(0);
                Some(TuiEvent::StepFailed {
                    job_id,
                    step_name,
                    step_index,
                    error,
                })
            }

            ProgressUpdate::StepSkipped { .. } => {
                // Skip events are not currently displayed in real-time
                None
            }

            ProgressUpdate::Completed { job_id, .. } => {
                let total_duration_ms = start_time.elapsed().as_millis() as u64;
                Some(TuiEvent::ExecutionCompleted {
                    job_id,
                    success: true,
                    total_duration_ms,
                })
            }

            ProgressUpdate::Failed { job_id, .. } => {
                let total_duration_ms = start_time.elapsed().as_millis() as u64;
                Some(TuiEvent::ExecutionCompleted {
                    job_id,
                    success: false,
                    total_duration_ms,
                })
            }

            // Other events (VariablesExtracted, ConditionEvaluated, etc.) are not forwarded
            _ => None,
        };

        if let Some(event) = event {
            if event_tx.send(event).is_err() {
                // Channel closed, stop forwarding
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_progress_channel() {
        let (sender, _receiver, job_id) = create_progress_channel();
        assert!(!job_id.is_nil());
        assert_eq!(sender.job_id(), job_id);
    }

    #[test]
    fn test_progress_output_to_string() {
        let mut output = ProgressOutput::default();
        output.lines.push("Line 1".to_string());
        output.lines.push("Line 2".to_string());
        assert_eq!(output.to_string(), "Line 1\nLine 2");
    }
}
