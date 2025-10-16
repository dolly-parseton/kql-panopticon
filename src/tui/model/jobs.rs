use crate::query_job::{QueryJobResult, QuerySettings};
use crate::workspace::Workspace;
use ratatui::widgets::TableState;
use std::time::Duration;

/// Context needed to retry a job
#[derive(Debug, Clone)]
pub struct RetryContext {
    pub workspace: Workspace,
    pub query: String,
    pub settings: QuerySettings,
}

/// Job execution state
#[derive(Debug, Clone)]
pub struct JobState {
    pub status: JobStatus,
    pub workspace_name: String,
    pub query_preview: String,
    pub duration: Option<Duration>,
    pub result: Option<QueryJobResult>,
    pub retry_context: Option<RetryContext>,
}

/// Job status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Queued => "QUEUED",
            JobStatus::Running => "RUNNING",
            JobStatus::Completed => "COMPLETED",
            JobStatus::Failed => "FAILED",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            JobStatus::Queued => Color::Yellow,
            JobStatus::Running => Color::Cyan,
            JobStatus::Completed => Color::Green,
            JobStatus::Failed => Color::Red,
        }
    }
}

/// Jobs tab state
#[derive(Debug, Clone)]
pub struct JobsModel {
    /// List of jobs
    pub jobs: Vec<JobState>,
    /// Table state for scrolling
    pub table_state: TableState,
}

impl JobsModel {
    /// Create a new JobsModel
    pub fn new() -> Self {
        Self {
            jobs: Vec::new(),
            table_state: TableState::default(),
        }
    }

    /// Add a new job in queued state (deprecated - use add_job_with_context)
    #[allow(dead_code)]
    pub fn add_job(&mut self, workspace_name: String, query_preview: String) {
        self.jobs.push(JobState {
            status: JobStatus::Queued,
            workspace_name,
            query_preview,
            duration: None,
            result: None,
            retry_context: None,
        });

        // Set initial selection to first job if this is the first one
        if self.jobs.len() == 1 {
            self.table_state.select(Some(0));
        }
    }

    /// Add a new job with full retry context
    pub fn add_job_with_context(
        &mut self,
        workspace_name: String,
        query_preview: String,
        retry_context: RetryContext,
    ) {
        self.jobs.push(JobState {
            status: JobStatus::Queued,
            workspace_name,
            query_preview,
            duration: None,
            result: None,
            retry_context: Some(retry_context),
        });

        // Set initial selection to first job if this is the first one
        if self.jobs.len() == 1 {
            self.table_state.select(Some(0));
        }
    }

    /// Update a job's status to completed
    pub fn complete_job(&mut self, index: usize, result: QueryJobResult) {
        if let Some(job) = self.jobs.get_mut(index) {
            job.duration = Some(result.elapsed);
            job.status = if result.result.is_ok() {
                JobStatus::Completed
            } else {
                JobStatus::Failed
            };
            job.result = Some(result);
        }
    }

    /// Clear completed and failed jobs
    pub fn clear_completed(&mut self) {
        self.jobs
            .retain(|job| job.status == JobStatus::Queued || job.status == JobStatus::Running);
        // If jobs remain after clearing, select the first one
        if !self.jobs.is_empty() {
            self.table_state.select(Some(0));
        } else {
            self.table_state.select(None);
        }
    }

    /// Get the currently selected job
    pub fn get_selected_job(&self) -> Option<&JobState> {
        self.table_state.selected().and_then(|i| self.jobs.get(i))
    }
}

impl Default for JobsModel {
    fn default() -> Self {
        Self::new()
    }
}
