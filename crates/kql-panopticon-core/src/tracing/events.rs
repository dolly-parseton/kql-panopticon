//! Typed events for TUI consumption
//!
//! These events are extracted from tracing spans and events by the TuiLayer,
//! providing structured data that the TUI can easily render.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

// Re-export ExecutionPhase from progress module
pub use crate::execution::progress::ExecutionPhase;

/// Structured events sent to TUI via channel
///
/// These are typed representations of tracing spans and events,
/// designed for easy consumption by UI components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TuiEvent {
    // === Job Lifecycle ===
    /// A pack execution job has started
    JobStarted {
        job_id: Uuid,
        pack_name: String,
        workspace_count: usize,
        step_count: usize,
        timestamp: DateTime<Utc>,
    },

    /// A pack execution job has completed
    JobCompleted {
        job_id: Uuid,
        success: bool,
        duration: Duration,
        timestamp: DateTime<Utc>,
    },

    // === Phase Lifecycle ===
    /// A phase has started (acquisition, processing, reporting)
    PhaseStarted {
        job_id: Uuid,
        workspace: String,
        phase: ExecutionPhase,
        step_count: usize,
        timestamp: DateTime<Utc>,
    },

    /// A phase has completed
    PhaseCompleted {
        job_id: Uuid,
        workspace: String,
        phase: ExecutionPhase,
        duration: Duration,
        timestamp: DateTime<Utc>,
    },

    // === Step Lifecycle ===
    /// A step has started execution
    StepStarted {
        job_id: Uuid,
        step_name: String,
        workspace: String,
        phase: ExecutionPhase,
        step_type: String,
        timestamp: DateTime<Utc>,
    },

    /// A step has completed successfully
    StepCompleted {
        job_id: Uuid,
        step_name: String,
        workspace: String,
        row_count: Option<usize>,
        duration: Duration,
        timestamp: DateTime<Utc>,
    },

    /// A step has failed
    StepFailed {
        job_id: Uuid,
        step_name: String,
        workspace: String,
        error: String,
        duration: Duration,
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

    // === Progress Updates ===
    /// Foreach iteration progress
    ForeachProgress {
        job_id: Uuid,
        step_name: String,
        workspace: String,
        current: usize,
        total: usize,
        timestamp: DateTime<Utc>,
    },

    // === Log Messages ===
    /// A log message at any level
    Log {
        level: LogLevel,
        target: String,
        message: String,
        /// Optional associated job
        job_id: Option<Uuid>,
        /// Optional associated step
        step_name: Option<String>,
        timestamp: DateTime<Utc>,
    },
}


/// Log levels matching tracing levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<tracing::Level> for LogLevel {
    fn from(level: tracing::Level) -> Self {
        match level {
            tracing::Level::TRACE => Self::Trace,
            tracing::Level::DEBUG => Self::Debug,
            tracing::Level::INFO => Self::Info,
            tracing::Level::WARN => Self::Warn,
            tracing::Level::ERROR => Self::Error,
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace => write!(f, "TRACE"),
            Self::Debug => write!(f, "DEBUG"),
            Self::Info => write!(f, "INFO"),
            Self::Warn => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

impl TuiEvent {
    /// Get the timestamp of this event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::JobStarted { timestamp, .. }
            | Self::JobCompleted { timestamp, .. }
            | Self::PhaseStarted { timestamp, .. }
            | Self::PhaseCompleted { timestamp, .. }
            | Self::StepStarted { timestamp, .. }
            | Self::StepCompleted { timestamp, .. }
            | Self::StepFailed { timestamp, .. }
            | Self::StepSkipped { timestamp, .. }
            | Self::ForeachProgress { timestamp, .. }
            | Self::Log { timestamp, .. } => *timestamp,
        }
    }

    /// Get the job ID if this event is associated with a job
    pub fn job_id(&self) -> Option<Uuid> {
        match self {
            Self::JobStarted { job_id, .. }
            | Self::JobCompleted { job_id, .. }
            | Self::PhaseStarted { job_id, .. }
            | Self::PhaseCompleted { job_id, .. }
            | Self::StepStarted { job_id, .. }
            | Self::StepCompleted { job_id, .. }
            | Self::StepFailed { job_id, .. }
            | Self::StepSkipped { job_id, .. }
            | Self::ForeachProgress { job_id, .. } => Some(*job_id),
            Self::Log { job_id, .. } => *job_id,
        }
    }

    /// Check if this is an error event
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            Self::StepFailed { .. } | Self::Log { level: LogLevel::Error, .. }
        )
    }

    /// Get a short description for logging
    pub fn summary(&self) -> String {
        match self {
            Self::JobStarted { pack_name, workspace_count, .. } => {
                format!("Job started: {} ({} workspaces)", pack_name, workspace_count)
            }
            Self::JobCompleted { success, duration, .. } => {
                let status = if *success { "success" } else { "failed" };
                format!("Job {}: {:?}", status, duration)
            }
            Self::PhaseStarted { phase, workspace, step_count, .. } => {
                format!("{} phase started on {} ({} steps)", phase, workspace, step_count)
            }
            Self::PhaseCompleted { phase, workspace, duration, .. } => {
                format!("{} phase completed on {}: {:?}", phase, workspace, duration)
            }
            Self::StepStarted { step_name, workspace, .. } => {
                format!("Step '{}' started on {}", step_name, workspace)
            }
            Self::StepCompleted { step_name, row_count, duration, .. } => {
                let rows = row_count.map(|r| format!("{} rows", r)).unwrap_or_default();
                format!("Step '{}' completed: {} {:?}", step_name, rows, duration)
            }
            Self::StepFailed { step_name, error, .. } => {
                format!("Step '{}' failed: {}", step_name, error)
            }
            Self::StepSkipped { step_name, reason, .. } => {
                format!("Step '{}' skipped: {}", step_name, reason)
            }
            Self::ForeachProgress { step_name, current, total, .. } => {
                format!("Step '{}': {}/{}", step_name, current, total)
            }
            Self::Log { level, message, .. } => {
                format!("[{}] {}", level, message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn test_event_job_id() {
        let job_id = Uuid::new_v4();
        let event = TuiEvent::StepStarted {
            job_id,
            step_name: "test".into(),
            workspace: "ws".into(),
            phase: ExecutionPhase::Acquisition,
            step_type: "kql".into(),
            timestamp: Utc::now(),
        };
        assert_eq!(event.job_id(), Some(job_id));

        let log_event = TuiEvent::Log {
            level: LogLevel::Info,
            target: "test".into(),
            message: "hello".into(),
            job_id: None,
            step_name: None,
            timestamp: Utc::now(),
        };
        assert_eq!(log_event.job_id(), None);
    }
}
