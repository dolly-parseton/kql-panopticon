//! Context events for execution lifecycle and state changes
//!
//! These events capture high-level operations in the execution context,
//! useful for debugging, auditing, and session replay.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Events that occur during execution context operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextEvent {
    // === Session Lifecycle ===
    /// Session started
    SessionStarted {
        session_id: Uuid,
    },

    /// Session ended
    SessionEnded {
        session_id: Uuid,
        reason: SessionEndReason,
    },

    // === Pack Operations ===
    /// Pack loaded into context
    PackLoaded {
        pack_name: String,
        step_count: usize,
        workspace_count: usize,
    },

    /// Pack unloaded from context
    PackUnloaded {
        pack_name: String,
    },

    // === Execution Lifecycle ===
    /// Execution job started
    ExecutionStarted {
        job_id: Uuid,
        pack_name: String,
        workspace: String,
        step_count: usize,
    },

    /// Individual step started
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
        duration_ms: u64,
    },

    /// Step was skipped
    StepSkipped {
        job_id: Uuid,
        step_name: String,
        step_index: usize,
        reason: String,
    },

    /// Execution job completed
    ExecutionCompleted {
        job_id: Uuid,
        success: bool,
        total_duration_ms: u64,
        steps_completed: usize,
        steps_failed: usize,
    },

    /// Execution was cancelled
    ExecutionCancelled {
        job_id: Uuid,
        reason: String,
    },

    // === Input Operations ===
    /// Input value set
    InputSet {
        name: String,
        /// Redacted for security - just indicates if value was set
        has_value: bool,
    },

    /// Input value cleared
    InputCleared {
        name: String,
    },

    // === Workspace Operations ===
    /// Workspace connected
    WorkspaceConnected {
        workspace_name: String,
    },

    /// Workspace disconnected
    WorkspaceDisconnected {
        workspace_name: String,
        reason: String,
    },

    // === Error Events ===
    /// General error occurred
    Error {
        context: String,
        message: String,
    },
}

/// Reason for session ending
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEndReason {
    /// User requested quit
    UserQuit,
    /// Error caused session end
    Error(String),
    /// Session timed out
    Timeout,
    /// External signal received
    Signal,
}

impl ContextEvent {
    /// Get a short description of the event for logging
    pub fn description(&self) -> String {
        match self {
            Self::SessionStarted { session_id } => {
                format!("Session started: {}", session_id)
            }
            Self::SessionEnded { session_id, reason } => {
                format!("Session ended: {} ({:?})", session_id, reason)
            }
            Self::PackLoaded { pack_name, step_count, workspace_count } => {
                format!("Pack loaded: {} ({} steps, {} workspaces)", pack_name, step_count, workspace_count)
            }
            Self::PackUnloaded { pack_name } => {
                format!("Pack unloaded: {}", pack_name)
            }
            Self::ExecutionStarted { job_id, pack_name, workspace, step_count } => {
                format!("Execution started: {} on {} ({} steps) [{}]", pack_name, workspace, step_count, job_id)
            }
            Self::StepStarted { step_name, step_index, .. } => {
                format!("Step {}: {} started", step_index, step_name)
            }
            Self::StepCompleted { step_name, step_index, row_count, duration_ms, .. } => {
                format!("Step {}: {} completed ({} rows, {}ms)", step_index, step_name, row_count, duration_ms)
            }
            Self::StepFailed { step_name, step_index, error, .. } => {
                format!("Step {}: {} failed: {}", step_index, step_name, error)
            }
            Self::StepSkipped { step_name, step_index, reason, .. } => {
                format!("Step {}: {} skipped: {}", step_index, step_name, reason)
            }
            Self::ExecutionCompleted { job_id, success, total_duration_ms, .. } => {
                let status = if *success { "success" } else { "failed" };
                format!("Execution completed: {} ({}ms) [{}]", status, total_duration_ms, job_id)
            }
            Self::ExecutionCancelled { job_id, reason } => {
                format!("Execution cancelled: {} [{}]", reason, job_id)
            }
            Self::InputSet { name, .. } => {
                format!("Input set: {}", name)
            }
            Self::InputCleared { name } => {
                format!("Input cleared: {}", name)
            }
            Self::WorkspaceConnected { workspace_name } => {
                format!("Workspace connected: {}", workspace_name)
            }
            Self::WorkspaceDisconnected { workspace_name, reason } => {
                format!("Workspace disconnected: {} ({})", workspace_name, reason)
            }
            Self::Error { context, message } => {
                format!("Error in {}: {}", context, message)
            }
        }
    }

    /// Check if this is an error event
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. } | Self::StepFailed { .. })
    }

    /// Get the job ID if this event is related to an execution
    pub fn job_id(&self) -> Option<Uuid> {
        match self {
            Self::ExecutionStarted { job_id, .. }
            | Self::StepStarted { job_id, .. }
            | Self::StepCompleted { job_id, .. }
            | Self::StepFailed { job_id, .. }
            | Self::StepSkipped { job_id, .. }
            | Self::ExecutionCompleted { job_id, .. }
            | Self::ExecutionCancelled { job_id, .. } => Some(*job_id),
            _ => None,
        }
    }
}
