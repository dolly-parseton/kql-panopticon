//! REPL context and state management
//!
//! Maintains the state of the REPL session including:
//! - Azure client and authentication
//! - Current workspace selection
//! - Loaded pack
//! - Background job tracking
//! - Execution history

use crate::history::{ExecutionHistory, ExecutionRecord};
use anyhow::Result;
use kql_panopticon_core::{Client, JobRegistry, Pack, Workspace};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared REPL context accessible across command handlers
pub type SharedContext = Arc<RwLock<ReplContext>>;

/// Main REPL context holding all session state
pub struct ReplContext {
    /// Azure client for API calls
    client: Option<Client>,

    /// All discovered workspaces
    available_workspaces: Vec<Workspace>,

    /// Currently selected workspace(s) for execution
    selected_workspaces: Vec<Workspace>,

    /// Currently loaded pack
    loaded_pack: Option<LoadedPack>,

    /// Background job registry (from core)
    job_registry: JobRegistry,

    /// Execution history for this session
    history: ExecutionHistory,

    /// Output directory for results
    output_dir: PathBuf,

    /// Whether client has been initialized
    initialized: bool,

    /// Whether workspace discovery is in progress
    discovering: bool,

    /// Error from background discovery (if any)
    discovery_error: Option<String>,
}

/// A loaded pack with its source path
pub struct LoadedPack {
    /// The pack definition
    pub pack: Pack,
    /// Where it was loaded from
    pub path: PathBuf,
}

impl ReplContext {
    /// Create a new REPL context (uninitialized)
    pub fn new() -> Self {
        let output_dir = dirs::home_dir()
            .map(|h| h.join(".kql-panopticon").join("output"))
            .unwrap_or_else(|| PathBuf::from("./output"));

        Self {
            client: None,
            available_workspaces: Vec::new(),
            selected_workspaces: Vec::new(),
            loaded_pack: None,
            job_registry: JobRegistry::new(),
            history: ExecutionHistory::new(),
            output_dir,
            initialized: false,
            discovering: false,
            discovery_error: None,
        }
    }

    /// Mark discovery as started
    pub fn start_discovery(&mut self) {
        self.discovering = true;
        self.discovery_error = None;
    }

    /// Complete discovery with workspaces
    pub fn complete_discovery(&mut self, client: Client, workspaces: Vec<Workspace>) {
        self.client = Some(client);
        self.available_workspaces = workspaces;
        self.initialized = true;
        self.discovering = false;
    }

    /// Mark discovery as failed
    pub fn fail_discovery(&mut self, error: String) {
        self.discovering = false;
        self.discovery_error = Some(error);
    }

    /// Check if discovery is in progress
    pub fn is_discovering(&self) -> bool {
        self.discovering
    }

    /// Get discovery error (if any)
    pub fn discovery_error(&self) -> Option<&str> {
        self.discovery_error.as_deref()
    }

    /// Initialize the Azure client and discover workspaces
    pub async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        let client = Client::new()?;
        self.available_workspaces = client.list_workspaces().await?;
        self.client = Some(client);
        self.initialized = true;

        Ok(())
    }

    /// Check if context is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get the Azure client (if initialized)
    pub fn client(&self) -> Option<&Client> {
        self.client.as_ref()
    }

    /// Get all available workspaces
    pub fn available_workspaces(&self) -> &[Workspace] {
        &self.available_workspaces
    }

    /// Get currently selected workspaces
    pub fn selected_workspaces(&self) -> &[Workspace] {
        &self.selected_workspaces
    }

    /// Select a workspace by name
    pub fn select_workspace(&mut self, name: &str) -> Result<()> {
        let workspace = self
            .available_workspaces
            .iter()
            .find(|w| w.name.eq_ignore_ascii_case(name))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Workspace '{}' not found", name))?;

        // Replace selection (single workspace mode for now)
        self.selected_workspaces = vec![workspace];
        Ok(())
    }

    /// Select multiple workspaces
    pub fn select_workspaces(&mut self, workspaces: Vec<Workspace>) {
        self.selected_workspaces = workspaces;
    }

    /// Select all available workspaces
    pub fn select_all_workspaces(&mut self) {
        self.selected_workspaces = self.available_workspaces.clone();
    }

    /// Clear workspace selection
    pub fn clear_workspace_selection(&mut self) {
        self.selected_workspaces.clear();
    }

    /// Get the loaded pack
    pub fn loaded_pack(&self) -> Option<&LoadedPack> {
        self.loaded_pack.as_ref()
    }

    /// Load a pack from a file
    pub fn load_pack(&mut self, path: PathBuf) -> Result<()> {
        let pack = Pack::load_from_file(&path)?;
        self.loaded_pack = Some(LoadedPack { pack, path });
        Ok(())
    }

    /// Unload the current pack
    pub fn unload_pack(&mut self) {
        self.loaded_pack = None;
    }

    /// Get the job registry
    pub fn job_registry(&self) -> &JobRegistry {
        &self.job_registry
    }

    /// Get mutable job registry
    pub fn job_registry_mut(&mut self) -> &mut JobRegistry {
        &mut self.job_registry
    }

    /// Get execution history
    pub fn history(&self) -> &ExecutionHistory {
        &self.history
    }

    /// Get mutable execution history
    pub fn history_mut(&mut self) -> &mut ExecutionHistory {
        &mut self.history
    }

    /// Record an execution
    pub fn record_execution(&mut self, record: ExecutionRecord) {
        self.history.add(record);
    }

    /// Get the output directory
    pub fn output_dir(&self) -> &PathBuf {
        &self.output_dir
    }

    /// Set the output directory
    pub fn set_output_dir(&mut self, path: PathBuf) {
        self.output_dir = path;
    }

    /// Get a summary of current state (for prompt/status)
    pub fn status_summary(&self) -> StatusSummary {
        StatusSummary {
            initialized: self.initialized,
            discovering: self.discovering,
            discovery_error: self.discovery_error.clone(),
            workspace_count: self.available_workspaces.len(),
            selected_workspace: self
                .selected_workspaces
                .first()
                .map(|w| w.name.clone()),
            selected_count: self.selected_workspaces.len(),
            loaded_pack: self.loaded_pack.as_ref().map(|p| p.pack.name.clone()),
            running_jobs: self.job_registry.running_count(),
        }
    }
}

impl Default for ReplContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of context state for display
#[derive(Debug, Clone)]
pub struct StatusSummary {
    pub initialized: bool,
    pub discovering: bool,
    pub discovery_error: Option<String>,
    pub workspace_count: usize,
    pub selected_workspace: Option<String>,
    pub selected_count: usize,
    pub loaded_pack: Option<String>,
    pub running_jobs: usize,
}

impl StatusSummary {
    /// Format as a status line
    pub fn as_status_line(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ws) = &self.selected_workspace {
            if self.selected_count > 1 {
                parts.push(format!("{} (+{})", ws, self.selected_count - 1));
            } else {
                parts.push(ws.clone());
            }
        } else if self.initialized {
            parts.push(format!("{} workspaces", self.workspace_count));
        } else {
            parts.push("not connected".to_string());
        }

        if let Some(pack) = &self.loaded_pack {
            parts.push(format!("pack:{}", pack));
        }

        if self.running_jobs > 0 {
            parts.push(format!("jobs:{}", self.running_jobs));
        }

        parts.join(" | ")
    }
}

/// Create a new shared context
pub fn create_shared_context() -> SharedContext {
    Arc::new(RwLock::new(ReplContext::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_summary_not_initialized() {
        let ctx = ReplContext::new();
        let summary = ctx.status_summary();
        assert!(!summary.initialized);
        assert_eq!(summary.as_status_line(), "not connected");
    }

    #[test]
    fn test_output_dir_default() {
        let ctx = ReplContext::new();
        assert!(ctx.output_dir().to_string_lossy().contains("kql-panopticon"));
    }
}
