//! Progress reporting for execution engines
//!
//! Provides a unified progress reporting system that works for both
//! query packs and investigation packs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

/// Progress update message sent during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProgressUpdate {
    // === Lifecycle Events ===

    /// Execution has started
    Started {
        job_id: Uuid,
        job_type: JobType,
        total_steps: usize,
        total_workspaces: usize,
        timestamp: DateTime<Utc>,
    },

    /// Execution completed successfully
    Completed {
        job_id: Uuid,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    /// Execution failed
    Failed {
        job_id: Uuid,
        error: String,
        timestamp: DateTime<Utc>,
    },

    // === Step-Level Events ===

    /// A step/query has started
    StepStarted {
        job_id: Uuid,
        step_name: String,
        workspace: String,
        timestamp: DateTime<Utc>,
    },

    /// A step/query completed successfully
    StepCompleted {
        job_id: Uuid,
        step_name: String,
        workspace: String,
        rows: usize,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    /// A step/query failed
    StepFailed {
        job_id: Uuid,
        step_name: String,
        workspace: String,
        error: String,
        timestamp: DateTime<Utc>,
    },

    /// A step was skipped (condition not met, dependency failed, etc.)
    StepSkipped {
        job_id: Uuid,
        step_name: String,
        workspace: String,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    // === Investigation-Specific Events ===

    /// Variables extracted from a step (investigation only)
    VariablesExtracted {
        job_id: Uuid,
        step_name: String,
        workspace: String,
        variables: Vec<String>,
        timestamp: DateTime<Utc>,
    },

    /// Condition evaluated (investigation only)
    ConditionEvaluated {
        job_id: Uuid,
        step_name: String,
        condition: String,
        result: bool,
        timestamp: DateTime<Utc>,
    },

    /// Foreach iteration progress (investigation only)
    ForeachProgress {
        job_id: Uuid,
        step_name: String,
        workspace: String,
        current: usize,
        total: usize,
        timestamp: DateTime<Utc>,
    },

    /// HTTP step executed (investigation only)
    HttpExecuted {
        job_id: Uuid,
        step_name: String,
        url: String,
        status: u16,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    // === Debug/Verbose Events ===

    /// Debug message (only when verbose mode is enabled)
    Debug {
        job_id: Uuid,
        message: String,
        timestamp: DateTime<Utc>,
    },
}

impl ProgressUpdate {
    /// Get the job ID from any progress update
    pub fn job_id(&self) -> Uuid {
        match self {
            Self::Started { job_id, .. }
            | Self::Completed { job_id, .. }
            | Self::Failed { job_id, .. }
            | Self::StepStarted { job_id, .. }
            | Self::StepCompleted { job_id, .. }
            | Self::StepFailed { job_id, .. }
            | Self::StepSkipped { job_id, .. }
            | Self::VariablesExtracted { job_id, .. }
            | Self::ConditionEvaluated { job_id, .. }
            | Self::ForeachProgress { job_id, .. }
            | Self::HttpExecuted { job_id, .. }
            | Self::Debug { job_id, .. } => *job_id,
        }
    }

    /// Get the timestamp from any progress update
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Started { timestamp, .. }
            | Self::Completed { timestamp, .. }
            | Self::Failed { timestamp, .. }
            | Self::StepStarted { timestamp, .. }
            | Self::StepCompleted { timestamp, .. }
            | Self::StepFailed { timestamp, .. }
            | Self::StepSkipped { timestamp, .. }
            | Self::VariablesExtracted { timestamp, .. }
            | Self::ConditionEvaluated { timestamp, .. }
            | Self::ForeachProgress { timestamp, .. }
            | Self::HttpExecuted { timestamp, .. }
            | Self::Debug { timestamp, .. } => *timestamp,
        }
    }

    /// Check if this is an error event
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Failed { .. } | Self::StepFailed { .. })
    }

    /// Create a debug message
    pub fn debug(job_id: Uuid, message: impl Into<String>) -> Self {
        Self::Debug {
            job_id,
            message: message.into(),
            timestamp: Utc::now(),
        }
    }
}

/// Type of job being executed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobType {
    /// Simple query or query pack
    Query,
    /// Investigation pack with chained steps
    Investigation,
}

impl std::fmt::Display for JobType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Query => write!(f, "Query"),
            Self::Investigation => write!(f, "Investigation"),
        }
    }
}

/// Wrapper around mpsc sender for progress updates
///
/// Provides helper methods for sending common progress events.
#[derive(Clone)]
pub struct ProgressSender {
    tx: mpsc::UnboundedSender<ProgressUpdate>,
    job_id: Uuid,
}

impl ProgressSender {
    /// Create a new progress sender
    pub fn new(tx: mpsc::UnboundedSender<ProgressUpdate>, job_id: Uuid) -> Self {
        Self { tx, job_id }
    }

    /// Get the job ID
    pub fn job_id(&self) -> Uuid {
        self.job_id
    }

    /// Send a progress update (ignores send errors)
    pub fn send(&self, update: ProgressUpdate) {
        let _ = self.tx.send(update);
    }

    /// Send the started event
    pub fn started(&self, job_type: JobType, total_steps: usize, total_workspaces: usize) {
        self.send(ProgressUpdate::Started {
            job_id: self.job_id,
            job_type,
            total_steps,
            total_workspaces,
            timestamp: Utc::now(),
        });
    }

    /// Send the completed event
    pub fn completed(&self, duration_ms: u64) {
        self.send(ProgressUpdate::Completed {
            job_id: self.job_id,
            duration_ms,
            timestamp: Utc::now(),
        });
    }

    /// Send the failed event
    pub fn failed(&self, error: impl Into<String>) {
        self.send(ProgressUpdate::Failed {
            job_id: self.job_id,
            error: error.into(),
            timestamp: Utc::now(),
        });
    }

    /// Send step started event
    pub fn step_started(&self, step_name: impl Into<String>, workspace: impl Into<String>) {
        self.send(ProgressUpdate::StepStarted {
            job_id: self.job_id,
            step_name: step_name.into(),
            workspace: workspace.into(),
            timestamp: Utc::now(),
        });
    }

    /// Send step completed event
    pub fn step_completed(
        &self,
        step_name: impl Into<String>,
        workspace: impl Into<String>,
        rows: usize,
        duration_ms: u64,
    ) {
        self.send(ProgressUpdate::StepCompleted {
            job_id: self.job_id,
            step_name: step_name.into(),
            workspace: workspace.into(),
            rows,
            duration_ms,
            timestamp: Utc::now(),
        });
    }

    /// Send step failed event
    pub fn step_failed(
        &self,
        step_name: impl Into<String>,
        workspace: impl Into<String>,
        error: impl Into<String>,
    ) {
        self.send(ProgressUpdate::StepFailed {
            job_id: self.job_id,
            step_name: step_name.into(),
            workspace: workspace.into(),
            error: error.into(),
            timestamp: Utc::now(),
        });
    }

    /// Send step skipped event
    pub fn step_skipped(
        &self,
        step_name: impl Into<String>,
        workspace: impl Into<String>,
        reason: impl Into<String>,
    ) {
        self.send(ProgressUpdate::StepSkipped {
            job_id: self.job_id,
            step_name: step_name.into(),
            workspace: workspace.into(),
            reason: reason.into(),
            timestamp: Utc::now(),
        });
    }

    /// Send debug message
    pub fn debug(&self, message: impl Into<String>) {
        self.send(ProgressUpdate::debug(self.job_id, message));
    }
}

/// Type alias for the receiver side
pub type ProgressReceiver = mpsc::UnboundedReceiver<ProgressUpdate>;

/// Create a new progress channel
pub fn progress_channel(job_id: Uuid) -> (ProgressSender, ProgressReceiver) {
    let (tx, rx) = mpsc::unbounded_channel();
    (ProgressSender::new(tx, job_id), rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_progress_sender() {
        let job_id = Uuid::new_v4();
        let (sender, mut receiver) = progress_channel(job_id);

        sender.started(JobType::Query, 5, 3);
        sender.step_started("query1", "workspace1");
        sender.step_completed("query1", "workspace1", 100, 500);
        sender.completed(1000);

        let mut count = 0;
        while let Ok(update) = receiver.try_recv() {
            assert_eq!(update.job_id(), job_id);
            count += 1;
        }

        assert_eq!(count, 4);
    }
}
