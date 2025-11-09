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

/// Structured job error information for better user feedback
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum JobError {
    /// Query timed out
    Timeout {
        duration_secs: u64,
        workspace: String,
    },
    /// Authentication failure
    Authentication { message: String },
    /// Query syntax error from Azure
    QuerySyntax {
        message: String,
        details: Option<String>,
    },
    /// Network or HTTP error
    Network {
        message: String,
        status_code: Option<u16>,
    },
    /// Azure API error
    AzureApi { status: u16, message: String },
    /// General error
    Other { message: String },
}

impl JobError {
    /// Get a short description for display in job list
    pub fn short_description(&self) -> String {
        match self {
            JobError::Timeout { duration_secs, .. } => {
                format!("Timeout ({}s)", duration_secs)
            }
            JobError::Authentication { .. } => "Auth Failed".to_string(),
            JobError::QuerySyntax { .. } => "Query Error".to_string(),
            JobError::Network { status_code, .. } => {
                if let Some(code) = status_code {
                    format!("Network Error ({})", code)
                } else {
                    "Network Error".to_string()
                }
            }
            JobError::AzureApi { status, .. } => {
                format!("Azure API Error ({})", status)
            }
            JobError::Other { .. } => "Failed".to_string(),
        }
    }

    /// Get a detailed description for display in popup/details view
    pub fn detailed_description(&self) -> String {
        match self {
            JobError::Timeout {
                duration_secs,
                workspace,
            } => {
                format!(
                    "Query timed out after {} seconds on workspace '{}'",
                    duration_secs, workspace
                )
            }
            JobError::Authentication { message } => {
                format!("Authentication failed: {}", message)
            }
            JobError::QuerySyntax { message, details } => {
                if let Some(details) = details {
                    format!("Query syntax error: {}\n\nDetails: {}", message, details)
                } else {
                    format!("Query syntax error: {}", message)
                }
            }
            JobError::Network {
                message,
                status_code,
            } => {
                if let Some(code) = status_code {
                    format!("Network error (HTTP {}): {}", code, message)
                } else {
                    format!("Network error: {}", message)
                }
            }
            JobError::AzureApi { status, message } => {
                format!("Azure API error (status {}): {}", status, message)
            }
            JobError::Other { message } => message.clone(),
        }
    }

    /// Determine if this error type is worth retrying
    /// Returns true for transient errors that may recover, false for permanent errors
    pub fn is_retryable(&self) -> bool {
        match self {
            // Transient errors - may recover on retry
            JobError::Timeout { .. } => true, // Network/server may recover
            JobError::Network { .. } => true, // Connectivity issues are transient
            JobError::Authentication { .. } => true, // Token may refresh
            JobError::AzureApi { status, .. } => {
                // Retry 5xx server errors, not 4xx client errors
                *status >= 500
            }
            // Permanent errors - won't fix themselves
            JobError::QuerySyntax { .. } => false, // Query must be fixed first
            JobError::Other { .. } => false,       // Unknown error - don't retry
        }
    }
}

/// Job execution state
#[derive(Debug, Clone)]
pub struct JobState {
    /// Unique identifier for this job (stable across sorting/reordering)
    pub job_id: u64,
    pub status: JobStatus,
    pub workspace_name: String,
    pub query_preview: String,
    pub duration: Option<Duration>,
    pub result: Option<QueryJobResult>,
    pub error: Option<JobError>,
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
    /// Counter for generating unique job IDs
    next_job_id: u64,
}

impl JobsModel {
    /// Create a new JobsModel
    pub fn new() -> Self {
        Self {
            jobs: Vec::new(),
            table_state: TableState::default(),
            next_job_id: 1, // Start from 1 (0 reserved for invalid/unset)
        }
    }

    /// Generate a new unique job ID
    fn next_id(&mut self) -> u64 {
        let id = self.next_job_id;
        self.next_job_id += 1;
        id
    }

    /// Get mutable reference to the next job ID (for session loading)
    pub fn next_job_id_mut(&mut self) -> &mut u64 {
        &mut self.next_job_id
    }

    /// Add a new job in queued state (deprecated - use add_job_with_context)
    #[allow(dead_code)]
    pub fn add_job(&mut self, workspace_name: String, query_preview: String) {
        let job_id = self.next_id();

        self.jobs.push(JobState {
            job_id,
            status: JobStatus::Queued,
            workspace_name,
            query_preview,
            duration: None,
            result: None,
            error: None,
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
    ) -> u64 {
        let job_id = self.next_id();

        self.jobs.push(JobState {
            job_id,
            status: JobStatus::Queued,
            workspace_name,
            query_preview,
            duration: None,
            result: None,
            error: None,
            retry_context: Some(retry_context),
        });

        // Set initial selection to first job if this is the first one
        if self.jobs.len() == 1 {
            self.table_state.select(Some(0));
        }

        job_id // Return the job ID for tracking
    }

    /// Update a job's status to completed
    /// Finds the job by ID (stable across sorting) instead of index
    pub fn complete_job(&mut self, job_id: u64, result: QueryJobResult) {
        // Find job by ID (not index!) since array may have been sorted
        if let Some(job) = self.jobs.iter_mut().find(|j| j.job_id == job_id) {
            job.duration = Some(result.elapsed);

            // Extract error information if the job failed
            if let Err(ref err) = result.result {
                job.status = JobStatus::Failed;
                job.error = Some(Self::categorize_error(
                    err,
                    &result.workspace_name,
                    result.elapsed,
                ));
            } else {
                job.status = JobStatus::Completed;
                job.error = None;
            }

            job.result = Some(result);
        } else {
            log::error!("Attempted to complete non-existent job with ID {}", job_id);
        }
    }

    /// Categorize a KqlPanopticonError into a JobError for better display
    pub fn categorize_error(
        error: &crate::error::KqlPanopticonError,
        workspace_name: &str,
        elapsed: Duration,
    ) -> JobError {
        use crate::error::KqlPanopticonError;

        match error {
            KqlPanopticonError::QueryExecutionFailed(msg) => {
                // Check if this is a timeout error
                if msg.contains("timed out") || msg.contains("timeout") {
                    JobError::Timeout {
                        duration_secs: elapsed.as_secs(),
                        workspace: workspace_name.to_string(),
                    }
                } else {
                    // Could be a query syntax error
                    JobError::QuerySyntax {
                        message: msg.clone(),
                        details: None,
                    }
                }
            }
            KqlPanopticonError::AuthenticationFailed(msg)
            | KqlPanopticonError::TokenAcquisitionFailed(msg) => JobError::Authentication {
                message: msg.clone(),
            },
            KqlPanopticonError::AzureApiError { status, message } => {
                // Check for specific status codes
                match *status {
                    401 | 403 => JobError::Authentication {
                        message: message.clone(),
                    },
                    400 => JobError::QuerySyntax {
                        message: message.clone(),
                        details: None,
                    },
                    504 => JobError::Timeout {
                        duration_secs: elapsed.as_secs(),
                        workspace: workspace_name.to_string(),
                    },
                    _ => JobError::AzureApi {
                        status: *status,
                        message: message.clone(),
                    },
                }
            }
            KqlPanopticonError::HttpRequestFailed(msg) => JobError::Network {
                message: msg.clone(),
                status_code: None,
            },
            _ => JobError::Other {
                message: error.to_string(),
            },
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

    /// Sort jobs by timestamp (newest first)
    pub fn sort_by_timestamp(&mut self) {
        self.jobs.sort_by(|a, b| {
            let timestamp_a = a.result.as_ref().map(|r| r.timestamp);
            let timestamp_b = b.result.as_ref().map(|r| r.timestamp);

            // Sort descending (newest first) - jobs without timestamps go to the end
            match (timestamp_a, timestamp_b) {
                (Some(a), Some(b)) => b.cmp(&a), // Reverse order for descending
                (Some(_), None) => std::cmp::Ordering::Less, // Jobs with timestamps come first
                (None, Some(_)) => std::cmp::Ordering::Greater, // Jobs without timestamps go last
                (None, None) => std::cmp::Ordering::Equal,
            }
        });
    }
}

impl Default for JobsModel {
    fn default() -> Self {
        Self::new()
    }
}
