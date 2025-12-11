//! Job registry for managing background execution
//!
//! The `JobRegistry` manages the lifecycle of background jobs, providing:
//! - Job spawning and tracking
//! - Progress subscription (push) and polling (pull)
//! - Result retrieval
//! - Job cancellation
//!
//! ## Usage Patterns
//!
//! ### Shell Command (polling)
//! ```rust,ignore
//! // `jobs` command - list all jobs
//! for summary in registry.list() {
//!     println!("{}: {} - {}", summary.id, summary.name, summary.status);
//! }
//!
//! // Prompt update - quick poll
//! let running = registry.running_count();
//! if running > 0 {
//!     print!("[{}⟳] ", running);
//! }
//! ```
//!
//! ### TUI Monitor (subscription)
//! ```rust,ignore
//! // `monitor` command - live updates
//! let rx = registry.subscribe(job_id)?;
//! while let Some(update) = rx.recv().await {
//!     render_progress(update);
//! }
//! ```

use super::progress::{JobType, ProgressReceiver, ProgressSender, ProgressUpdate};
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use uuid::Uuid;

/// Registry for managing background jobs
///
/// Thread-safe container for tracking job execution, progress, and results.
/// Designed to support both the shell REPL (polling) and TUI monitor (subscription).
#[derive(Clone)]
pub struct JobRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

struct RegistryInner {
    jobs: HashMap<Uuid, ManagedJob>,
    /// Channel for broadcasting job events to interested parties
    event_tx: mpsc::UnboundedSender<JobEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<JobEvent>>,
}

/// A job managed by the registry
struct ManagedJob {
    /// Job metadata
    pub info: JobInfo,
    /// Current status
    pub status: JobStatus,
    /// Latest progress update (for polling)
    pub latest_progress: Option<ProgressUpdate>,
    /// Progress broadcast sender (for subscriptions)
    pub progress_tx: mpsc::UnboundedSender<ProgressUpdate>,
    /// Cancellation token
    pub cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// Final result (when completed)
    pub result: Option<JobResult>,
}

/// Public job information
#[derive(Debug, Clone)]
pub struct JobInfo {
    /// Unique job ID
    pub id: Uuid,
    /// Human-readable name
    pub name: String,
    /// Job type
    pub job_type: JobType,
    /// When the job was created
    pub created_at: DateTime<Utc>,
    /// When the job started executing
    pub started_at: Option<DateTime<Utc>>,
    /// When the job completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Associated workspace(s)
    pub workspaces: Vec<String>,
}

/// Job execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    /// Job is queued but not yet started
    Pending,
    /// Job is currently executing
    Running,
    /// Job completed successfully
    Completed,
    /// Job failed with error
    Failed,
    /// Job was cancelled by user
    Cancelled,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Summary of a job for listing
#[derive(Debug, Clone)]
pub struct JobSummary {
    pub id: Uuid,
    pub name: String,
    pub job_type: JobType,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub progress_percent: Option<u8>,
    pub progress_message: Option<String>,
    pub error: Option<String>,
}

/// Final result of a completed job
#[derive(Debug, Clone)]
pub struct JobResult {
    /// Total rows across all workspaces
    pub total_rows: usize,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Output paths
    pub output_paths: Vec<std::path::PathBuf>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Per-workspace results
    pub workspace_results: HashMap<String, WorkspaceJobResult>,
}

/// Result for a single workspace
#[derive(Debug, Clone)]
pub struct WorkspaceJobResult {
    pub workspace: String,
    pub status: JobStatus,
    pub rows: usize,
    pub duration_ms: u64,
    pub output_path: Option<std::path::PathBuf>,
    pub error: Option<String>,
}

/// Events emitted by the registry
#[derive(Debug, Clone)]
pub enum JobEvent {
    /// New job created
    Created { id: Uuid, name: String, job_type: JobType },
    /// Job started executing
    Started { id: Uuid },
    /// Job progress update
    Progress { id: Uuid, percent: Option<u8>, message: String },
    /// Job completed
    Completed { id: Uuid, result: JobResult },
    /// Job failed
    Failed { id: Uuid, error: String },
    /// Job cancelled
    Cancelled { id: Uuid },
}

impl JobRegistry {
    /// Create a new job registry
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                jobs: HashMap::new(),
                event_tx,
                event_rx: Some(event_rx),
            })),
        }
    }

    /// Take the event receiver (can only be called once)
    ///
    /// Used by the shell to receive job events for prompt updates, notifications, etc.
    pub fn take_event_receiver(&self) -> Option<mpsc::UnboundedReceiver<JobEvent>> {
        self.inner.write().ok()?.event_rx.take()
    }

    /// Register a new job and get a progress sender
    ///
    /// Returns (job_id, progress_sender, cancel_receiver)
    pub fn register(
        &self,
        name: impl Into<String>,
        job_type: JobType,
        workspaces: Vec<String>,
    ) -> Result<(Uuid, ProgressSender, tokio::sync::oneshot::Receiver<()>)> {
        let id = Uuid::new_v4();
        let name = name.into();
        let (progress_tx, _) = mpsc::unbounded_channel();
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();

        let job = ManagedJob {
            info: JobInfo {
                id,
                name: name.clone(),
                job_type,
                created_at: Utc::now(),
                started_at: None,
                completed_at: None,
                workspaces,
            },
            status: JobStatus::Pending,
            latest_progress: None,
            progress_tx: progress_tx.clone(),
            cancel_tx: Some(cancel_tx),
            result: None,
        };

        {
            let mut inner = self.inner.write().map_err(|_| Error::other("Lock poisoned"))?;
            inner.jobs.insert(id, job);
            let _ = inner.event_tx.send(JobEvent::Created { id, name, job_type });
        }

        // Create a progress sender that also updates the registry
        let registry = self.clone();
        let sender = ProgressSender::new(progress_tx, id);

        // Wrap to intercept progress updates
        // TODO: In actual implementation, we'd wrap the sender to update latest_progress

        Ok((id, sender, cancel_rx))
    }

    /// Mark a job as started
    pub fn mark_started(&self, id: Uuid) -> Result<()> {
        let mut inner = self.inner.write().map_err(|_| Error::other("Lock poisoned"))?;
        if let Some(job) = inner.jobs.get_mut(&id) {
            job.status = JobStatus::Running;
            job.info.started_at = Some(Utc::now());
            let _ = inner.event_tx.send(JobEvent::Started { id });
        }
        Ok(())
    }

    /// Update job progress (called by executor via intercepted sender)
    pub fn update_progress(&self, id: Uuid, update: ProgressUpdate) -> Result<()> {
        let mut inner = self.inner.write().map_err(|_| Error::other("Lock poisoned"))?;
        if let Some(job) = inner.jobs.get_mut(&id) {
            // Broadcast to subscribers
            let _ = job.progress_tx.send(update.clone());
            // Store latest for polling
            job.latest_progress = Some(update.clone());

            // Extract progress info for event
            let (percent, message) = match &update {
                ProgressUpdate::StepCompleted { step_name, rows, .. } => {
                    (None, format!("{}: {} rows", step_name, rows))
                }
                ProgressUpdate::StepStarted { step_name, .. } => {
                    (None, format!("Running: {}", step_name))
                }
                ProgressUpdate::ForeachProgress { current, total, .. } => {
                    let pct = (*current as f32 / *total as f32 * 100.0) as u8;
                    (Some(pct), format!("{}/{}", current, total))
                }
                _ => (None, String::new()),
            };

            if !message.is_empty() {
                let _ = inner.event_tx.send(JobEvent::Progress { id, percent, message });
            }
        }
        Ok(())
    }

    /// Mark a job as completed
    pub fn mark_completed(&self, id: Uuid, result: JobResult) -> Result<()> {
        let mut inner = self.inner.write().map_err(|_| Error::other("Lock poisoned"))?;
        let should_send = if let Some(job) = inner.jobs.get_mut(&id) {
            job.status = if result.error.is_some() {
                JobStatus::Failed
            } else {
                JobStatus::Completed
            };
            job.info.completed_at = Some(Utc::now());
            job.result = Some(result.clone());
            true
        } else {
            false
        };
        if should_send {
            let _ = inner.event_tx.send(JobEvent::Completed { id, result });
        }
        Ok(())
    }

    /// Mark a job as failed
    pub fn mark_failed(&self, id: Uuid, error: impl Into<String>) -> Result<()> {
        let error = error.into();
        let mut inner = self.inner.write().map_err(|_| Error::other("Lock poisoned"))?;
        let should_send = if let Some(job) = inner.jobs.get_mut(&id) {
            job.status = JobStatus::Failed;
            job.info.completed_at = Some(Utc::now());
            job.result = Some(JobResult {
                total_rows: 0,
                duration_ms: 0,
                output_paths: vec![],
                error: Some(error.clone()),
                workspace_results: HashMap::new(),
            });
            true
        } else {
            false
        };
        if should_send {
            let _ = inner.event_tx.send(JobEvent::Failed { id, error });
        }
        Ok(())
    }

    /// Cancel a job
    pub fn cancel(&self, id: Uuid) -> Result<()> {
        let mut inner = self.inner.write().map_err(|_| Error::other("Lock poisoned"))?;
        if let Some(job) = inner.jobs.get_mut(&id) {
            if let Some(cancel_tx) = job.cancel_tx.take() {
                let _ = cancel_tx.send(());
            }
            job.status = JobStatus::Cancelled;
            job.info.completed_at = Some(Utc::now());
            let _ = inner.event_tx.send(JobEvent::Cancelled { id });
        }
        Ok(())
    }

    /// Subscribe to progress updates for a job (for TUI monitor)
    pub fn subscribe(&self, id: Uuid) -> Result<mpsc::UnboundedReceiver<ProgressUpdate>> {
        let inner = self.inner.read().map_err(|_| Error::other("Lock poisoned"))?;
        if let Some(job) = inner.jobs.get(&id) {
            // Create a new receiver by subscribing to the broadcast
            let (tx, rx) = mpsc::unbounded_channel();
            // TODO: In actual implementation, use broadcast channel or similar
            // For now, this is a stub
            Ok(rx)
        } else {
            Err(Error::other(format!("Job {} not found", id)))
        }
    }

    /// Get latest progress for a job (for shell polling)
    pub fn poll_progress(&self, id: Uuid) -> Option<ProgressUpdate> {
        let inner = self.inner.read().ok()?;
        inner.jobs.get(&id)?.latest_progress.clone()
    }

    /// Get job info
    pub fn get(&self, id: Uuid) -> Option<JobInfo> {
        let inner = self.inner.read().ok()?;
        inner.jobs.get(&id).map(|j| j.info.clone())
    }

    /// Get job status
    pub fn status(&self, id: Uuid) -> Option<JobStatus> {
        let inner = self.inner.read().ok()?;
        inner.jobs.get(&id).map(|j| j.status)
    }

    /// Get job result (if completed)
    pub fn result(&self, id: Uuid) -> Option<JobResult> {
        let inner = self.inner.read().ok()?;
        inner.jobs.get(&id)?.result.clone()
    }

    /// List all jobs
    pub fn list(&self) -> Vec<JobSummary> {
        let inner = match self.inner.read() {
            Ok(i) => i,
            Err(_) => return vec![],
        };

        inner
            .jobs
            .values()
            .map(|job| {
                let (progress_percent, progress_message) = match &job.latest_progress {
                    Some(ProgressUpdate::ForeachProgress { current, total, .. }) => {
                        let pct = (*current as f32 / *total as f32 * 100.0) as u8;
                        (Some(pct), Some(format!("{}/{}", current, total)))
                    }
                    Some(ProgressUpdate::StepStarted { step_name, .. }) => {
                        (None, Some(format!("Running: {}", step_name)))
                    }
                    _ => (None, None),
                };

                JobSummary {
                    id: job.info.id,
                    name: job.info.name.clone(),
                    job_type: job.info.job_type,
                    status: job.status,
                    created_at: job.info.created_at,
                    progress_percent,
                    progress_message,
                    error: job.result.as_ref().and_then(|r| r.error.clone()),
                }
            })
            .collect()
    }

    /// List jobs with a specific status
    pub fn list_by_status(&self, status: JobStatus) -> Vec<JobSummary> {
        self.list().into_iter().filter(|j| j.status == status).collect()
    }

    /// Count of running jobs (for prompt display)
    pub fn running_count(&self) -> usize {
        let inner = match self.inner.read() {
            Ok(i) => i,
            Err(_) => return 0,
        };
        inner.jobs.values().filter(|j| j.status == JobStatus::Running).count()
    }

    /// Count of pending jobs
    pub fn pending_count(&self) -> usize {
        let inner = match self.inner.read() {
            Ok(i) => i,
            Err(_) => return 0,
        };
        inner.jobs.values().filter(|j| j.status == JobStatus::Pending).count()
    }

    /// Remove completed/failed/cancelled jobs older than duration
    pub fn cleanup(&self, older_than: chrono::Duration) -> usize {
        let cutoff = Utc::now() - older_than;
        let mut inner = match self.inner.write() {
            Ok(i) => i,
            Err(_) => return 0,
        };

        let to_remove: Vec<Uuid> = inner
            .jobs
            .iter()
            .filter(|(_, j)| {
                matches!(j.status, JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled)
                    && j.info.completed_at.map_or(false, |t| t < cutoff)
            })
            .map(|(id, _)| *id)
            .collect();

        let count = to_remove.len();
        for id in to_remove {
            inner.jobs.remove(&id);
        }
        count
    }
}

impl Default for JobRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_lifecycle() {
        let registry = JobRegistry::new();

        // Register a job
        let (id, _sender, _cancel_rx) = registry
            .register("test-job", JobType::Query, vec!["ws1".to_string()])
            .unwrap();

        // Check initial state
        assert_eq!(registry.status(id), Some(JobStatus::Pending));
        assert_eq!(registry.running_count(), 0);

        // Mark started
        registry.mark_started(id).unwrap();
        assert_eq!(registry.status(id), Some(JobStatus::Running));
        assert_eq!(registry.running_count(), 1);

        // Mark completed
        registry
            .mark_completed(
                id,
                JobResult {
                    total_rows: 100,
                    duration_ms: 1000,
                    output_paths: vec![],
                    error: None,
                    workspace_results: HashMap::new(),
                },
            )
            .unwrap();

        assert_eq!(registry.status(id), Some(JobStatus::Completed));
        assert_eq!(registry.running_count(), 0);

        // Check result
        let result = registry.result(id).unwrap();
        assert_eq!(result.total_rows, 100);
    }

    #[test]
    fn test_list_jobs() {
        let registry = JobRegistry::new();

        registry
            .register("job1", JobType::Query, vec![])
            .unwrap();
        registry
            .register("job2", JobType::Investigation, vec![])
            .unwrap();

        let jobs = registry.list();
        assert_eq!(jobs.len(), 2);
    }
}
