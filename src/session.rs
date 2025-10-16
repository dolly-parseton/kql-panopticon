use crate::error::KqlPanopticonError;
use crate::query_job::{QueryJobResult, QuerySettings};
use crate::tui::model::jobs::{JobState, JobStatus, RetryContext};
use crate::tui::model::settings::SettingsModel;
use crate::workspace::Workspace;
use serde::{Deserialize, Serialize};
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
    pub error_message: Option<String>,
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

        Self {
            status: job.status.as_str().to_string(),
            workspace_name: job.workspace_name.clone(),
            query_preview: job.query_preview.clone(),
            duration_millis: job.duration.map(|d| d.as_millis() as u64),
            workspace,
            query,
            settings,
            error_message,
        }
    }
}

impl Session {
    /// Create a new session from current state
    pub fn new(
        name: String,
        settings: &SettingsModel,
        jobs: &[JobState],
    ) -> Self {
        let now = chrono::Local::now().to_rfc3339();

        Self {
            version: SESSION_VERSION,
            name: name.clone(),
            created_at: now.clone(),
            last_saved: now,
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
    pub fn to_job_states(&self) -> Vec<JobState> {
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

                // Reconstruct result if we have error info
                let result = if let Some(err) = &job.error_message {
                    Some(QueryJobResult {
                        workspace_id: job.workspace.as_ref().map(|w| w.workspace_id.clone()).unwrap_or_default(),
                        workspace_name: job.workspace_name.clone(),
                        query: job.query.clone().unwrap_or_default(),
                        result: Err(KqlPanopticonError::QueryExecutionFailed(err.clone())),
                        elapsed: duration.unwrap_or_default(),
                    })
                } else if status == JobStatus::Completed {
                    // Completed jobs - create success result placeholder
                    Some(QueryJobResult {
                        workspace_id: job.workspace.as_ref().map(|w| w.workspace_id.clone()).unwrap_or_default(),
                        workspace_name: job.workspace_name.clone(),
                        query: job.query.clone().unwrap_or_default(),
                        result: Ok(crate::query_job::JobSuccess {
                            row_count: 0, // We don't save row count, but it's not critical
                            page_count: 1, // Default to 1 page
                            output_path: PathBuf::from(""),
                            file_size: 0,
                        }),
                        elapsed: duration.unwrap_or_default(),
                    })
                } else {
                    None
                };

                JobState {
                    status,
                    workspace_name: job.workspace_name.clone(),
                    query_preview: job.query_preview.clone(),
                    duration,
                    result,
                    retry_context,
                }
            })
            .collect()
    }
}

/// Get the sessions directory path (~/.kql-panopticon/sessions)
pub fn get_sessions_dir() -> Result<PathBuf, KqlPanopticonError> {
    let home = dirs::home_dir()
        .ok_or_else(|| KqlPanopticonError::InvalidConfiguration(
            "Could not find home directory".to_string(),
        ))?;

    Ok(home.join(".kql-panopticon").join("sessions"))
}
