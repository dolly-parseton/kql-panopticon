pub mod jobs;
pub mod packs;
pub mod query;
pub mod session;
pub mod settings;
pub mod workspaces;

use crate::client::Client;
use crate::query_job::QueryJobResult;
use crate::tui::message::Tab;
use jobs::JobsModel;
use packs::PacksModel;
use query::QueryModel;
use session::SessionModel;
use settings::SettingsModel;
use tokio::sync::mpsc;
use workspaces::WorkspacesModel;

/// Main application model (state)
pub struct Model {
    /// Current active tab
    pub current_tab: Tab,
    /// Settings state
    pub settings: SettingsModel,
    /// Workspaces state
    pub workspaces: WorkspacesModel,
    /// Query state
    pub query: QueryModel,
    /// Jobs state
    pub jobs: JobsModel,
    /// Sessions state
    pub sessions: SessionModel,
    /// Query packs state
    pub packs: PacksModel,
    /// Azure client
    pub client: Client,
    /// Current popup message (if any)
    pub popup: Option<Popup>,
    /// Channel for receiving job updates from background tasks
    pub job_update_rx: mpsc::UnboundedReceiver<JobUpdateMessage>,
    /// Channel for sending job updates from background tasks
    pub job_update_tx: mpsc::UnboundedSender<JobUpdateMessage>,
    /// Initialization state
    pub init_state: InitState,
    /// Spinner animation frame counter
    pub spinner_frame: usize,
}

/// Popup types
#[derive(Debug, Clone)]
pub enum Popup {
    /// Error message (red)
    Error(String),
    /// Success message (green)
    Success(String),
    /// Settings edit popup
    SettingsEdit,
    /// Job name input popup
    JobNameInput,
    /// Job details popup with job index
    JobDetails(usize),
    /// Session name input popup (for save as / new session)
    SessionNameInput,
}

/// Message for job status updates from background tasks
#[derive(Debug, Clone)]
pub enum JobUpdateMessage {
    Completed(u64, QueryJobResult), // Job ID (not index!) completed with result
}

/// Initialization state of the application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitState {
    /// Initializing - authenticating and loading workspaces
    Initializing,
    /// Fully initialized and ready
    Ready,
    /// Initialization failed
    Failed,
}

impl Model {
    /// Create a new Model
    pub fn new(client: Client) -> Self {
        let (job_update_tx, job_update_rx) = mpsc::unbounded_channel();

        Self {
            current_tab: Tab::Query,
            settings: SettingsModel::new(),
            workspaces: WorkspacesModel::new(),
            query: QueryModel::new(),
            jobs: JobsModel::new(),
            sessions: SessionModel::new(),
            packs: PacksModel::new(),
            client,
            popup: None,
            job_update_rx,
            job_update_tx,
            init_state: InitState::Initializing,
            spinner_frame: 0,
        }
    }

    /// Rebuild the client with current settings
    pub fn rebuild_client(&mut self) -> Result<(), crate::error::KqlPanopticonError> {
        use std::time::Duration;

        self.client = Client::with_config(
            Duration::from_secs(self.settings.validation_interval_secs),
            Duration::from_secs(self.settings.query_timeout_secs),
            self.settings.retry_count,
        )?;

        Ok(())
    }

    /// Process pending job updates from the channel
    pub fn process_job_updates(&mut self) {
        let mut should_sort = false;
        while let Ok(message) = self.job_update_rx.try_recv() {
            match message {
                JobUpdateMessage::Completed(job_idx, result) => {
                    self.jobs.complete_job(job_idx, result);
                    should_sort = true;
                }
            }
        }
        // Sort jobs after all updates are processed
        if should_sort {
            self.jobs.sort_by_timestamp();
        }
    }
}
