//! Pack execution state management
//!
//! Handles async pack execution with progress reporting.
//! Uses unbounded channels to ensure no progress updates are lost.

use kql_panopticon_core::execution::{PackExecutorConfig, PackExecutorResult, ProgressUpdate};
use kql_panopticon_core::{Client, Error as CoreError, Pack, PackExecutor, Workspace};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Result from background pack execution task
#[derive(Debug)]
pub enum PackExecutionResult {
    /// Progress update message
    Progress(String),
    /// Execution completed successfully
    Completed(PackExecutorResult),
    /// Execution failed with error
    Failed(CoreError),
}

/// Receiver for pack execution results
pub type PackExecutionReceiver = mpsc::UnboundedReceiver<PackExecutionResult>;

/// Sender for pack execution results (internal use)
type PackExecutionSender = mpsc::UnboundedSender<PackExecutionResult>;

/// Spawn background task for pack execution
///
/// The task performs pack execution and communicates progress/results back
/// via the unbounded channel. Returns both the task handle and the receiver.
///
/// Accepts Arc<Pack> for cheap cloning - the expensive pack clone happens
/// in the background task rather than blocking the UI thread.
pub fn spawn_execution_task(
    client: Client,
    pack: Arc<Pack>,
    pack_path: Option<PathBuf>,
    inputs: HashMap<String, String>,
    workspaces: Vec<Workspace>,
    output_dir: Option<PathBuf>,
) -> (tokio::task::JoinHandle<()>, PackExecutionReceiver) {
    let (result_tx, result_rx) = mpsc::unbounded_channel();

    let task_handle = tokio::spawn(async move {
        // Clone the pack from Arc (happens in background, not blocking UI)
        let pack = (*pack).clone();

        // Build config
        let mut config = PackExecutorConfig::new(pack)
            .with_inputs(inputs)
            .with_output_dir(output_dir.unwrap_or_else(|| PathBuf::from("./output")));

        if let Some(path) = pack_path {
            config = config.with_pack_path(path);
        }

        // Create executor
        let executor = PackExecutor::new(client);

        // Create progress channel
        let job_id = uuid::Uuid::new_v4();
        let (progress_tx, mut progress_rx) =
            kql_panopticon_core::execution::progress::progress_channel(job_id);

        // Execute pack and monitor progress concurrently using select!
        let mut completed_count = 0;
        let total_workspaces = workspaces.len();
        let mut buffer = String::with_capacity(256);

        // Start execution future
        let exec_future = executor.execute(config, workspaces, Some(progress_tx));
        tokio::pin!(exec_future);

        // Process both execution and progress updates in same task
        let exec_result = loop {
            tokio::select! {
                // Execution completed
                result = &mut exec_future => {
                    break result;
                }
                // Progress update received
                Some(update) = progress_rx.recv() => {
                    buffer.clear(); // Reuse buffer

                    let should_send = match update {
                        ProgressUpdate::WorkspaceCompleted { workspace, .. } => {
                            completed_count += 1;
                            let _ = write!(
                                buffer,
                                "Completed workspace {}/{}: {}",
                                completed_count, total_workspaces, workspace
                            );
                            true
                        }
                        ProgressUpdate::StepCompleted {
                            step_name,
                            workspace,
                            rows,
                            ..
                        } => {
                            let _ = write!(buffer, "  {} / {} → {} rows", workspace, step_name, rows);
                            true
                        }
                        ProgressUpdate::StepFailed {
                            step_name,
                            workspace,
                            error,
                            ..
                        } => {
                            let _ = write!(
                                buffer,
                                "  {} / {} → FAILED: {}",
                                workspace, step_name, error
                            );
                            true
                        }
                        _ => false, // Ignore other progress events
                    };

                    if should_send {
                        // Send to channel - errors only occur if receiver dropped
                        let _ = result_tx.send(PackExecutionResult::Progress(buffer.clone()));
                    }
                }
            }
        };

        // Drain any remaining progress updates after execution completes
        while let Ok(update) = progress_rx.try_recv() {
            buffer.clear();

            let should_send = match update {
                ProgressUpdate::WorkspaceCompleted { workspace, .. } => {
                    completed_count += 1;
                    let _ = write!(
                        buffer,
                        "Completed workspace {}/{}: {}",
                        completed_count, total_workspaces, workspace
                    );
                    true
                }
                ProgressUpdate::StepCompleted {
                    step_name,
                    workspace,
                    rows,
                    ..
                } => {
                    let _ = write!(buffer, "  {} / {} → {} rows", workspace, step_name, rows);
                    true
                }
                ProgressUpdate::StepFailed {
                    step_name,
                    workspace,
                    error,
                    ..
                } => {
                    let _ = write!(
                        buffer,
                        "  {} / {} → FAILED: {}",
                        workspace, step_name, error
                    );
                    true
                }
                _ => false,
            };

            if should_send {
                let _ = result_tx.send(PackExecutionResult::Progress(buffer.clone()));
            }
        }

        // Now send the final result (all progress updates processed)
        let final_result = match exec_result {
            Ok(result) => PackExecutionResult::Completed(result),
            Err(e) => PackExecutionResult::Failed(e),
        };

        // Send final result - if this fails, receiver was dropped (app closed)
        let _ = result_tx.send(final_result);
    });

    (task_handle, result_rx)
}
