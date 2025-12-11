//! Shell context - manages state for the REPL session
//!
//! Holds the Azure client, loaded workspaces, current pack, background jobs, etc.

use crate::client::Client;
use crate::error::Result;
use crate::query_pack::QueryPack;
use crate::workspace::Workspace;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;

/// The main shell context holding all session state
pub struct ShellContext {
    /// Azure client for API calls
    pub client: Client,

    /// Discovered workspaces
    pub workspaces: Vec<Workspace>,

    /// Currently selected workspace indices (for multi-workspace ops)
    pub selected_workspaces: Vec<usize>,

    /// Currently loaded query pack
    pub current_pack: Option<LoadedPack>,

    /// Current query text (if not from pack)
    pub current_query: Option<String>,

    /// Background jobs
    pub background_jobs: HashMap<String, BackgroundJob>,

    /// Event receiver for background job updates
    pub event_rx: mpsc::UnboundedReceiver<JobEvent>,

    /// Event sender for background jobs
    pub event_tx: mpsc::UnboundedSender<JobEvent>,

    /// Session name (if saved/loaded)
    pub session_name: Option<String>,

    /// Whether session has unsaved changes
    pub session_dirty: bool,

    /// Output directory for results
    pub output_dir: PathBuf,
}

/// A loaded query pack with metadata
#[derive(Debug, Clone)]
pub struct LoadedPack {
    /// The pack itself
    pub pack: QueryPack,

    /// Where it was loaded from
    pub path: PathBuf,

    /// Currently selected query index
    pub selected_query: Option<usize>,
}

/// A background job (query or investigation)
pub struct BackgroundJob {
    pub id: String,
    pub name: String,
    pub job_type: JobType,
    pub status: JobStatus,
    pub started_at: chrono::DateTime<chrono::Local>,
    pub progress_percent: Option<u8>,
    pub progress_message: Option<String>,
}

/// Type of background job
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobType {
    Query,
    Investigation,
}

/// Status of a background job
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Running,
    Completed,
    Failed,
}

/// Event from a background job
#[derive(Debug, Clone)]
pub enum JobEvent {
    Started { id: String, name: String, job_type: JobType },
    Progress { id: String, percent: u8, message: String },
    Completed { id: String, row_count: usize, output_path: Option<PathBuf> },
    Failed { id: String, error: String },
}

impl ShellContext {
    /// Create a new shell context
    pub fn new(client: Client) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Self {
            client,
            workspaces: Vec::new(),
            selected_workspaces: Vec::new(),
            current_pack: None,
            current_query: None,
            background_jobs: HashMap::new(),
            event_rx,
            event_tx,
            session_name: None,
            session_dirty: false,
            output_dir: PathBuf::from("./output"),
        }
    }

    /// Discover available workspaces
    pub async fn discover_workspaces(&mut self) -> Result<()> {
        let workspaces = self.client.list_workspaces().await?;
        self.workspaces = workspaces;

        // Select first workspace by default
        if !self.workspaces.is_empty() && self.selected_workspaces.is_empty() {
            self.selected_workspaces.push(0);
        }

        Ok(())
    }

    /// Get the currently selected workspace(s)
    pub fn selected_workspace_list(&self) -> Vec<&Workspace> {
        self.selected_workspaces
            .iter()
            .filter_map(|&i| self.workspaces.get(i))
            .collect()
    }

    /// Get the primary selected workspace
    pub fn primary_workspace(&self) -> Option<&Workspace> {
        self.selected_workspaces
            .first()
            .and_then(|&i| self.workspaces.get(i))
    }

    /// Get count of running jobs
    pub fn running_job_count(&self) -> usize {
        self.background_jobs
            .values()
            .filter(|j| matches!(j.status, JobStatus::Running))
            .count()
    }

    /// Poll for background job events and update state
    pub fn poll_background_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                JobEvent::Started { id, name, job_type } => {
                    self.background_jobs.insert(
                        id.clone(),
                        BackgroundJob {
                            id,
                            name,
                            job_type,
                            status: JobStatus::Running,
                            started_at: chrono::Local::now(),
                            progress_percent: Some(0),
                            progress_message: Some("Starting...".to_string()),
                        },
                    );
                }
                JobEvent::Progress { id, percent, message } => {
                    if let Some(job) = self.background_jobs.get_mut(&id) {
                        job.progress_percent = Some(percent);
                        job.progress_message = Some(message);
                    }
                }
                JobEvent::Completed { id, row_count, output_path } => {
                    if let Some(job) = self.background_jobs.get_mut(&id) {
                        job.status = JobStatus::Completed;
                        job.progress_percent = Some(100);
                        job.progress_message = Some(format!(
                            "{} rows{}",
                            row_count,
                            output_path
                                .map(|p| format!(" → {}", p.display()))
                                .unwrap_or_default()
                        ));
                    }
                }
                JobEvent::Failed { id, error } => {
                    if let Some(job) = self.background_jobs.get_mut(&id) {
                        job.status = JobStatus::Failed;
                        job.progress_message = Some(error);
                    }
                }
            }
        }
    }

    /// Generate a unique job ID
    pub fn new_job_id(&self) -> String {
        Uuid::new_v4().to_string()[..8].to_string()
    }

    /// Get event sender for spawning jobs
    pub fn job_event_sender(&self) -> mpsc::UnboundedSender<JobEvent> {
        self.event_tx.clone()
    }
}
