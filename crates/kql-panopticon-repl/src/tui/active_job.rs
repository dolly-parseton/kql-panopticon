//! Active job tracking with cancellation support
//!
//! Provides a mechanism to track running executions and cancel them on request.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

/// A cancellation token for cooperative cancellation of async tasks
#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Create a new cancellation token
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if cancellation has been requested
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Request cancellation
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// An active job being executed
#[derive(Debug, Clone)]
pub struct ActiveJob {
    /// Unique job identifier
    pub job_id: Uuid,
    /// Pack name being executed
    pub pack_name: String,
    /// Workspace name being used
    pub workspace: String,
    /// Cancellation token for this job
    cancellation: CancellationToken,
    /// Block ID of the progress widget displaying this job
    pub block_id: crate::tui::app::BlockId,
}

impl ActiveJob {
    /// Create a new active job
    pub fn new(
        job_id: Uuid,
        pack_name: impl Into<String>,
        workspace: impl Into<String>,
        block_id: crate::tui::app::BlockId,
    ) -> Self {
        Self {
            job_id,
            pack_name: pack_name.into(),
            workspace: workspace.into(),
            cancellation: CancellationToken::new(),
            block_id,
        }
    }

    /// Get the cancellation token for this job
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    /// Check if this job has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// Request cancellation of this job
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }
}

/// Manager for active jobs
#[derive(Debug, Default)]
pub struct ActiveJobManager {
    /// Currently active jobs
    jobs: std::collections::HashMap<Uuid, ActiveJob>,
}

impl ActiveJobManager {
    /// Create a new job manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Start tracking a new job
    pub fn start_job(&mut self, job: ActiveJob) -> CancellationToken {
        let token = job.cancellation_token();
        self.jobs.insert(job.job_id, job);
        token
    }

    /// Get a job by ID
    pub fn get(&self, job_id: &Uuid) -> Option<&ActiveJob> {
        self.jobs.get(job_id)
    }

    /// Get a mutable job by ID
    pub fn get_mut(&mut self, job_id: &Uuid) -> Option<&mut ActiveJob> {
        self.jobs.get_mut(job_id)
    }

    /// Get a job by block ID
    pub fn get_by_block(&self, block_id: crate::tui::app::BlockId) -> Option<&ActiveJob> {
        self.jobs.values().find(|j| j.block_id == block_id)
    }

    /// Cancel a job by ID
    pub fn cancel(&self, job_id: &Uuid) -> bool {
        if let Some(job) = self.jobs.get(job_id) {
            job.cancel();
            true
        } else {
            false
        }
    }

    /// Cancel a job by block ID
    pub fn cancel_by_block(&self, block_id: crate::tui::app::BlockId) -> bool {
        if let Some(job) = self.get_by_block(block_id) {
            job.cancel();
            true
        } else {
            false
        }
    }

    /// Complete a job (remove from active tracking)
    pub fn complete(&mut self, job_id: &Uuid) -> Option<ActiveJob> {
        self.jobs.remove(job_id)
    }

    /// Get all active jobs
    pub fn active_jobs(&self) -> impl Iterator<Item = &ActiveJob> {
        self.jobs.values()
    }

    /// Get count of active jobs
    pub fn active_count(&self) -> usize {
        self.jobs.len()
    }

    /// Check if any jobs are active
    pub fn has_active_jobs(&self) -> bool {
        !self.jobs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancellation_token() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());

        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_cancellation_token_clone() {
        let token1 = CancellationToken::new();
        let token2 = token1.clone();

        assert!(!token1.is_cancelled());
        assert!(!token2.is_cancelled());

        token1.cancel();

        assert!(token1.is_cancelled());
        assert!(token2.is_cancelled()); // Clone sees the cancellation
    }
}
