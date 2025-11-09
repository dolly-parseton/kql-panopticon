use crate::error::KqlPanopticonError;
use crate::query_job::{QueryJobResult, QuerySettings};
use crate::query_pack::{PackQuery, QueryPack};
use crate::tui::model::jobs::{JobState, JobStatus, JobsModel, RetryContext};
use crate::tui::model::settings::SettingsModel;
use crate::workspace::Workspace;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Session file format version
const SESSION_VERSION: u32 = 1;

/// A saved session containing jobs and settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session format version
    pub version: u32,
    /// Session name (filename without extension)
    pub name: String,
    /// Timestamp when session was created
    pub created_at: String,
    /// Timestamp when session was last saved
    pub last_saved: String,
    /// Query pack that created this session (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_from_pack: Option<String>,
    /// Settings at time of save
    pub settings: SerializableSettings,
    /// Jobs at time of save
    pub jobs: Vec<SerializableJob>,
}

/// Serializable settings (subset of SettingsModel)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableSettings {
    pub output_folder: String,
    pub query_timeout_secs: u64,
    pub retry_count: u32,
    pub validation_interval_secs: u64,
    pub export_csv: bool,
    pub export_json: bool,
    pub parse_dynamics: bool,
}

impl From<&SettingsModel> for SerializableSettings {
    fn from(model: &SettingsModel) -> Self {
        Self {
            output_folder: model.output_folder.clone(),
            query_timeout_secs: model.query_timeout_secs,
            retry_count: model.retry_count,
            validation_interval_secs: model.validation_interval_secs,
            export_csv: model.export_csv,
            export_json: model.export_json,
            parse_dynamics: model.parse_dynamics,
        }
    }
}

/// Serializable job state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableJob {
    pub status: String, // "Queued", "Running", "Completed", "Failed"
    pub workspace_name: String,
    pub query_preview: String,
    pub duration_millis: Option<u64>,
    pub workspace: Option<Workspace>,
    pub query: Option<String>,
    pub settings: Option<QuerySettings>,
    pub error_message: Option<String>, // Kept for backwards compatibility
    pub error_details: Option<crate::tui::model::jobs::JobError>, // Structured error (v2+)
    #[serde(default)]
    pub timestamp: Option<String>, // ISO 8601 / RFC3339 format
}

impl From<&JobState> for SerializableJob {
    fn from(job: &JobState) -> Self {
        let (workspace, query, settings) = if let Some(ctx) = &job.retry_context {
            (
                Some(ctx.workspace.clone()),
                Some(ctx.query.clone()),
                Some(ctx.settings.clone()),
            )
        } else {
            (None, None, None)
        };

        let error_message = job
            .result
            .as_ref()
            .and_then(|r| r.result.as_ref().err())
            .map(|e| e.to_string());

        // Capture structured error details
        let error_details = job.error.clone();

        // Extract timestamp from result if available
        let timestamp = job.result.as_ref().map(|r| r.timestamp.to_rfc3339());

        Self {
            status: job.status.as_str().to_string(),
            workspace_name: job.workspace_name.clone(),
            query_preview: job.query_preview.clone(),
            duration_millis: job.duration.map(|d| d.as_millis() as u64),
            workspace,
            query,
            settings,
            error_message,
            error_details,
            timestamp,
        }
    }
}

impl Session {
    /// Create a new session from current state
    #[allow(dead_code)]
    pub fn new(name: String, settings: &SettingsModel, jobs: &[JobState]) -> Self {
        Self::new_with_pack(name, settings, jobs, None)
    }

    /// Create a new session with optional pack origin
    pub fn new_with_pack(
        name: String,
        settings: &SettingsModel,
        jobs: &[JobState],
        created_from_pack: Option<String>,
    ) -> Self {
        let now = chrono::Local::now().to_rfc3339();

        Self {
            version: SESSION_VERSION,
            name: name.clone(),
            created_at: now.clone(),
            last_saved: now,
            created_from_pack,
            settings: SerializableSettings::from(settings),
            jobs: jobs.iter().map(SerializableJob::from).collect(),
        }
    }

    /// Update the last_saved timestamp
    pub fn touch(&mut self) {
        self.last_saved = chrono::Local::now().to_rfc3339();
    }

    /// Save session to file
    pub fn save(&self) -> Result<PathBuf, KqlPanopticonError> {
        let sessions_dir = get_sessions_dir()?;
        fs::create_dir_all(&sessions_dir)?;

        let file_path = sessions_dir.join(format!("{}.json", self.name));
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&file_path, json)?;

        Ok(file_path)
    }

    /// Load session from file
    pub fn load(name: &str) -> Result<Self, KqlPanopticonError> {
        let sessions_dir = get_sessions_dir()?;
        let file_path = sessions_dir.join(format!("{}.json", name));

        let json = fs::read_to_string(&file_path)?;
        let session: Session = serde_json::from_str(&json)?;

        Ok(session)
    }

    /// Delete session file
    pub fn delete(name: &str) -> Result<(), KqlPanopticonError> {
        let sessions_dir = get_sessions_dir()?;
        let file_path = sessions_dir.join(format!("{}.json", name));
        fs::remove_file(&file_path)?;
        Ok(())
    }

    /// Convert session to a reusable query pack
    pub fn to_query_pack(&self) -> Result<QueryPack, KqlPanopticonError> {
        // Deduplicate queries - use HashMap to track unique queries
        let mut unique_queries: HashMap<String, PackQuery> = HashMap::new();

        for (idx, job) in self.jobs.iter().enumerate() {
            if let Some(query) = &job.query {
                // Use query text as key for deduplication
                if !unique_queries.contains_key(query) {
                    let query_name = if self.jobs.len() == 1 {
                        // Single query: use session name
                        self.name.clone()
                    } else {
                        // Multiple queries: generate names
                        format!("Query {}", idx + 1)
                    };

                    unique_queries.insert(
                        query.clone(),
                        PackQuery {
                            name: query_name,
                            description: Some(format!("From workspace: {}", job.workspace_name)),
                            query: query.clone(),
                        },
                    );
                }
            }
        }

        if unique_queries.is_empty() {
            return Err(KqlPanopticonError::QueryPackValidation(
                "Session contains no queries to export".into(),
            ));
        }

        // Generate pack name from session name (remove timestamp suffix if present)
        let pack_name = self
            .name
            .rsplit_once('_')
            .and_then(|(prefix, suffix)| {
                // Check if suffix looks like a timestamp (8 digits or date-like)
                if suffix.chars().all(|c| c.is_ascii_digit()) && suffix.len() >= 6 {
                    Some(prefix.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| self.name.clone());

        // Extract settings
        let settings = QuerySettings {
            output_folder: PathBuf::from(&self.settings.output_folder),
            job_name: "exported-query".to_string(),
            export_csv: self.settings.export_csv,
            export_json: self.settings.export_json,
            parse_dynamics: self.settings.parse_dynamics,
        };

        // Build query pack
        let queries: Vec<PackQuery> = unique_queries.into_values().collect();

        let pack = if queries.len() == 1 {
            // Single query: use simple format
            QueryPack {
                name: pack_name,
                description: Some(format!("Exported from session: {}", self.name)),
                author: Some("kql-panopticon".to_string()),
                version: Some("1.0".to_string()),
                query: Some(queries[0].query.clone()),
                queries: None,
                settings: Some(settings),
                workspaces: None, // Don't include workspace scope
            }
        } else {
            // Multiple queries: use multi-query format
            QueryPack {
                name: pack_name,
                description: Some(format!("Exported from session: {}", self.name)),
                author: Some("kql-panopticon".to_string()),
                version: Some("1.0".to_string()),
                query: None,
                queries: Some(queries),
                settings: Some(settings),
                workspaces: None,
            }
        };

        Ok(pack)
    }

    /// List all available sessions
    pub fn list_all() -> Result<Vec<String>, KqlPanopticonError> {
        let sessions_dir = get_sessions_dir()?;

        // Create directory if it doesn't exist
        if !sessions_dir.exists() {
            fs::create_dir_all(&sessions_dir)?;
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();

        for entry in fs::read_dir(&sessions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    sessions.push(stem.to_string());
                }
            }
        }

        sessions.sort();
        Ok(sessions)
    }

    /// Apply this session's settings to a SettingsModel
    pub fn apply_to_settings(&self, model: &mut SettingsModel) {
        model.output_folder = self.settings.output_folder.clone();
        model.query_timeout_secs = self.settings.query_timeout_secs;
        model.retry_count = self.settings.retry_count;
        model.validation_interval_secs = self.settings.validation_interval_secs;
        model.export_csv = self.settings.export_csv;
        model.export_json = self.settings.export_json;
        model.parse_dynamics = self.settings.parse_dynamics;
    }

    /// Convert this session's jobs to JobState vector
    pub fn to_job_states(&self, next_id: &mut u64) -> Vec<JobState> {
        self.jobs
            .iter()
            .map(|job| {
                let status = match job.status.as_str() {
                    "QUEUED" => JobStatus::Queued,
                    "RUNNING" => JobStatus::Running,
                    "COMPLETED" => JobStatus::Completed,
                    "FAILED" => JobStatus::Failed,
                    _ => JobStatus::Queued,
                };

                let retry_context = if let (Some(workspace), Some(query), Some(settings)) =
                    (&job.workspace, &job.query, &job.settings)
                {
                    Some(RetryContext {
                        workspace: workspace.clone(),
                        query: query.clone(),
                        settings: settings.clone(),
                    })
                } else {
                    None
                };

                let duration = job.duration_millis.map(Duration::from_millis);

                // Parse timestamp from ISO 8601 / RFC3339 format, or use current time as fallback
                let timestamp = job
                    .timestamp
                    .as_ref()
                    .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
                    .map(|dt| dt.with_timezone(&chrono::Local))
                    .unwrap_or_else(chrono::Local::now);

                // Reconstruct result and error info
                let (result, error) = if let Some(err) = &job.error_message {
                    let kql_error = KqlPanopticonError::QueryExecutionFailed(err.clone());

                    // Prefer structured error details if available (v2+), otherwise re-categorize (v1)
                    let job_error = if let Some(error_details) = &job.error_details {
                        error_details.clone()
                    } else {
                        // Backwards compatibility: re-categorize from error message
                        JobsModel::categorize_error(
                            &kql_error,
                            &job.workspace_name,
                            duration.unwrap_or_default(),
                        )
                    };

                    (
                        Some(QueryJobResult {
                            workspace_id: job
                                .workspace
                                .as_ref()
                                .map(|w| w.workspace_id.clone())
                                .unwrap_or_default(),
                            workspace_name: job.workspace_name.clone(),
                            query: job.query.clone().unwrap_or_default(),
                            result: Err(kql_error),
                            elapsed: duration.unwrap_or_default(),
                            timestamp,
                        }),
                        Some(job_error),
                    )
                } else if status == JobStatus::Completed {
                    // Completed jobs - create success result placeholder
                    (
                        Some(QueryJobResult {
                            workspace_id: job
                                .workspace
                                .as_ref()
                                .map(|w| w.workspace_id.clone())
                                .unwrap_or_default(),
                            workspace_name: job.workspace_name.clone(),
                            query: job.query.clone().unwrap_or_default(),
                            result: Ok(crate::query_job::JobSuccess {
                                row_count: 0,  // We don't save row count, but it's not critical
                                page_count: 1, // Default to 1 page
                                output_path: PathBuf::from(""),
                                file_size: 0,
                            }),
                            elapsed: duration.unwrap_or_default(),
                            timestamp,
                        }),
                        None,
                    )
                } else {
                    (None, None)
                };

                // Generate a new job ID for each loaded job
                let job_id = *next_id;
                *next_id += 1;

                JobState {
                    job_id,
                    status,
                    workspace_name: job.workspace_name.clone(),
                    query_preview: job.query_preview.clone(),
                    duration,
                    result,
                    error,
                    retry_context,
                }
            })
            .collect()
    }
}

/// Get the sessions directory path (~/.kql-panopticon/sessions)
pub fn get_sessions_dir() -> Result<PathBuf, KqlPanopticonError> {
    let home = dirs::home_dir().ok_or_else(|| {
        KqlPanopticonError::InvalidConfiguration("Could not find home directory".to_string())
    })?;

    Ok(home.join(".kql-panopticon").join("sessions"))
}
