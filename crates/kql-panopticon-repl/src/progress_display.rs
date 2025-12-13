//! Progress display for execution monitoring
//!
//! Simple text-based progress output.

use kql_panopticon_core::execution::progress::{
    progress_channel, ProgressReceiver, ProgressSender, ProgressUpdate,
};
use uuid::Uuid;

/// Create a progress channel for execution
pub fn create_progress_channel() -> (ProgressSender, ProgressReceiver, Uuid) {
    let job_id = Uuid::new_v4();
    let (sender, receiver) = progress_channel(job_id);
    (sender, receiver, job_id)
}

/// Run progress display with simple text output
///
/// Prints step progress as simple text lines.
pub async fn run_progress_display(
    _step_names: Vec<String>,
    workspace: String,
    mut receiver: ProgressReceiver,
) -> anyhow::Result<()> {
    println!("Running on {}...", workspace);

    // Process updates until complete
    while let Some(update) = receiver.recv().await {
        match &update {
            ProgressUpdate::StepStarted { step_name, .. } => {
                print!("  ⟳ {} running...", step_name);
                // Flush to show immediately
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
            ProgressUpdate::StepCompleted {
                step_name,
                rows,
                duration_ms,
                ..
            } => {
                // Clear the "running" line and print completed
                print!("\r  \x1b[32m✓\x1b[0m {} - {} rows ({}ms)\n", step_name, rows, duration_ms);
            }
            ProgressUpdate::StepFailed {
                step_name, error, ..
            } => {
                print!("\r  \x1b[31m✗\x1b[0m {} - {}\n", step_name, error);
            }
            ProgressUpdate::StepSkipped {
                step_name, reason, ..
            } => {
                println!("  ○ {} - skipped: {}", step_name, reason);
            }
            ProgressUpdate::Completed { .. } | ProgressUpdate::Failed { .. } => {
                break;
            }
            _ => {}
        }
    }

    Ok(())
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
}
