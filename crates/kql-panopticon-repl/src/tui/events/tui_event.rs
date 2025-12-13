//! Real-time TUI event channel
//!
//! Provides async communication between background tasks (like execution)
//! and the TUI event loop for real-time UI updates.

use tokio::sync::mpsc;
use uuid::Uuid;

/// Events that can be sent to the TUI for real-time updates
#[derive(Debug, Clone)]
pub enum TuiEvent {
    // === Execution Progress ===
    /// Execution started
    ExecutionStarted {
        job_id: Uuid,
        pack_name: String,
        step_count: usize,
    },

    /// Step started executing
    StepStarted {
        job_id: Uuid,
        step_name: String,
        step_index: usize,
    },

    /// Step completed successfully
    StepCompleted {
        job_id: Uuid,
        step_name: String,
        step_index: usize,
        row_count: usize,
        duration_ms: u64,
    },

    /// Step failed with error
    StepFailed {
        job_id: Uuid,
        step_name: String,
        step_index: usize,
        error: String,
    },

    /// Execution completed (all steps done)
    ExecutionCompleted {
        job_id: Uuid,
        success: bool,
        total_duration_ms: u64,
    },

    /// Execution was cancelled
    ExecutionCancelled {
        job_id: Uuid,
    },

    // === Discovery Events ===
    /// Workspace discovery completed successfully
    /// Note: Context is updated in the background task; this is just for UI notification
    DiscoveryComplete {
        workspace_count: usize,
    },

    /// Workspace discovery failed
    DiscoveryFailed {
        error: String,
    },

    // === Connection Events ===
    /// Workspace connection established
    WorkspaceConnected {
        workspace_name: String,
    },

    /// Workspace connection lost
    WorkspaceDisconnected {
        workspace_name: String,
        reason: String,
    },

    // === General ===
    /// Request UI refresh
    Refresh,

    /// Display a notification/toast
    Notification {
        message: String,
        is_error: bool,
    },
}

impl TuiEvent {
    /// Get the job ID if this is an execution-related event
    pub fn job_id(&self) -> Option<Uuid> {
        match self {
            Self::ExecutionStarted { job_id, .. }
            | Self::StepStarted { job_id, .. }
            | Self::StepCompleted { job_id, .. }
            | Self::StepFailed { job_id, .. }
            | Self::ExecutionCompleted { job_id, .. }
            | Self::ExecutionCancelled { job_id } => Some(*job_id),
            _ => None,
        }
    }

    /// Create a notification event
    pub fn notification(message: impl Into<String>) -> Self {
        Self::Notification {
            message: message.into(),
            is_error: false,
        }
    }

    /// Create an error notification event
    pub fn error(message: impl Into<String>) -> Self {
        Self::Notification {
            message: message.into(),
            is_error: true,
        }
    }
}

/// Sender for TUI events (clone-able for use in multiple tasks)
pub type TuiEventSender = mpsc::UnboundedSender<TuiEvent>;

/// Receiver for TUI events (used by the main event loop)
pub type TuiEventReceiver = mpsc::UnboundedReceiver<TuiEvent>;

/// Create a new TUI event channel
pub fn channel() -> (TuiEventSender, TuiEventReceiver) {
    mpsc::unbounded_channel()
}
