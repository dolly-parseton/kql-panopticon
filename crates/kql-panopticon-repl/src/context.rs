//! REPL context and state management
//!
//! Maintains the state of the REPL session including:
//! - Azure client and authentication
//! - Current workspace selection
//! - Loaded pack
//! - Background job tracking
//! - Execution history
//! - Pack authoring session with exploring interpreter state graph

use crate::history::{ExecutionHistory, ExecutionRecord};
use crate::session::PackSession;
use crate::state_graph::{StateGraph, StateId};
use anyhow::Result;
use kql_panopticon_core::schema::{SchemaRegistry, SchemaStatus, get_schema_status};
use kql_panopticon_core::{Client, JobRegistry, Pack, Workspace};
use std::collections::HashSet;
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

    /// State graph for exploring interpreter pattern
    /// Maintains snapshots for backtracking and checkpoints
    state_graph: StateGraph,

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

    /// Schema registry for table definitions
    schema_registry: Option<SchemaRegistry>,

    /// Workspaces currently being captured (schema discovery in progress)
    schema_capturing: HashSet<String>,

    /// Schema capture errors by workspace ID
    schema_capture_errors: std::collections::HashMap<String, String>,
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

        // Try to load schema registry from disk
        let schema_registry = SchemaRegistry::load_default()
            .map_err(|e| log::warn!("Failed to load schema registry: {}", e))
            .ok();

        Self {
            client: None,
            available_workspaces: Vec::new(),
            selected_workspaces: Vec::new(),
            loaded_pack: None,
            state_graph: StateGraph::new(),
            job_registry: JobRegistry::new(),
            history: ExecutionHistory::new(),
            output_dir,
            initialized: false,
            discovering: false,
            discovery_error: None,
            schema_registry,
            schema_capturing: HashSet::new(),
            schema_capture_errors: std::collections::HashMap::new(),
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
    #[allow(dead_code)]
    pub fn is_discovering(&self) -> bool {
        self.discovering
    }

    /// Get discovery error (if any)
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

    // ========== Schema Registry Methods ==========

    /// Get the schema registry (if loaded)
    pub fn schema_registry(&self) -> Option<&SchemaRegistry> {
        self.schema_registry.as_ref()
    }

    /// Get mutable schema registry (if loaded)
    pub fn schema_registry_mut(&mut self) -> Option<&mut SchemaRegistry> {
        self.schema_registry.as_mut()
    }

    /// Ensure schema registry exists (create with defaults if not)
    pub fn ensure_schema_registry(&mut self) -> &mut SchemaRegistry {
        if self.schema_registry.is_none() {
            self.schema_registry = Some(SchemaRegistry::with_defaults());
        }
        self.schema_registry.as_mut().unwrap()
    }

    /// Get schema status for a workspace
    pub fn get_schema_status(&self, workspace_id: &str) -> SchemaStatus {
        // Check if capture is in progress
        if self.schema_capturing.contains(workspace_id) {
            return SchemaStatus::Capturing;
        }

        // Check registry
        match &self.schema_registry {
            Some(registry) => get_schema_status(registry, workspace_id, 30),
            None => SchemaStatus::None,
        }
    }

    /// Get schema status for currently selected workspace
    pub fn selected_workspace_schema_status(&self) -> Option<SchemaStatus> {
        self.selected_workspaces
            .first()
            .map(|ws| self.get_schema_status(&ws.workspace_id))
    }

    /// Check if any selected workspace needs schema capture
    pub fn needs_schema_capture(&self) -> Vec<&Workspace> {
        self.selected_workspaces
            .iter()
            .filter(|ws| {
                let status = self.get_schema_status(&ws.workspace_id);
                matches!(status, SchemaStatus::None | SchemaStatus::Stale)
            })
            .collect()
    }

    /// Get all workspaces needing schema capture
    pub fn workspaces_needing_capture(&self) -> Vec<&Workspace> {
        self.available_workspaces
            .iter()
            .filter(|ws| {
                let status = self.get_schema_status(&ws.workspace_id);
                matches!(status, SchemaStatus::None | SchemaStatus::Stale)
            })
            .collect()
    }

    /// Mark a workspace as having schema capture in progress
    pub fn start_schema_capture(&mut self, workspace_id: &str) {
        self.schema_capturing.insert(workspace_id.to_string());
        self.schema_capture_errors.remove(workspace_id);
    }

    /// Mark schema capture as complete for a workspace
    pub fn complete_schema_capture(&mut self, workspace_id: &str) {
        self.schema_capturing.remove(workspace_id);
        self.schema_capture_errors.remove(workspace_id);
    }

    /// Mark schema capture as failed for a workspace
    pub fn fail_schema_capture(&mut self, workspace_id: &str, error: String) {
        self.schema_capturing.remove(workspace_id);
        self.schema_capture_errors.insert(workspace_id.to_string(), error);
    }

    /// Check if schema capture is in progress for any workspace
    pub fn is_schema_capturing(&self) -> bool {
        !self.schema_capturing.is_empty()
    }

    /// Get the count of workspaces being captured
    pub fn schema_capture_count(&self) -> usize {
        self.schema_capturing.len()
    }

    /// Save schema registry to disk
    pub fn save_schema_registry(&mut self) -> Result<()> {
        if let Some(ref mut registry) = self.schema_registry {
            registry.save_if_dirty()?;
        }
        Ok(())
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

    /// Get the pack authoring session (current state)
    pub fn pack_session(&self) -> &PackSession {
        self.state_graph.current_session()
    }

    /// Get mutable pack authoring session (current state)
    ///
    /// Note: Changes made here are not automatically committed.
    /// Call `commit_state` after modifications to create a new state.
    pub fn pack_session_mut(&mut self) -> &mut PackSession {
        self.state_graph.current_session_mut()
    }

    /// Start a new pack session (resets state graph)
    pub fn new_pack_session(&mut self, name: Option<String>) {
        let session = match name {
            Some(n) => PackSession::with_name(n),
            None => PackSession::new(),
        };
        self.state_graph.reset_with_session(session);
    }

    // === State Graph Methods ===

    /// Get the state graph
    pub fn state_graph(&self) -> &StateGraph {
        &self.state_graph
    }

    /// Get mutable state graph
    pub fn state_graph_mut(&mut self) -> &mut StateGraph {
        &mut self.state_graph
    }

    /// Get the current state ID
    pub fn current_state_id(&self) -> StateId {
        self.state_graph.current_id()
    }

    /// Commit current session changes as a new state
    pub fn commit_state(&mut self, command: impl Into<String>) -> StateId {
        self.state_graph.commit(command)
    }

    /// Revert to a previous state by ID
    pub fn revert_to_state(&mut self, state_id: StateId) -> bool {
        self.state_graph.revert_to(state_id).is_some()
    }

    /// Revert to the previous state
    pub fn revert_state(&mut self) -> bool {
        self.state_graph.revert().is_some()
    }

    /// Save a named checkpoint at the current state
    pub fn save_checkpoint(&mut self, name: impl Into<String>) {
        self.state_graph.save_checkpoint(name);
    }

    /// Restore a named checkpoint
    pub fn restore_checkpoint(&mut self, name: &str) -> bool {
        self.state_graph.restore_checkpoint(name).is_some()
    }

    /// Get the checkpoint name for the current state, if any
    pub fn current_checkpoint(&self) -> Option<&str> {
        self.state_graph.current_checkpoint()
    }

    /// List all checkpoints
    pub fn list_checkpoints(&self) -> Vec<(&str, StateId)> {
        self.state_graph.list_checkpoints()
    }

    /// Delete a checkpoint by name
    pub fn delete_checkpoint(&mut self, name: &str) -> bool {
        self.state_graph.delete_checkpoint(name).is_some()
    }

    /// Get the job registry
    pub fn job_registry(&self) -> &JobRegistry {
        &self.job_registry
    }

    /// Get mutable job registry
    #[allow(dead_code)]
    pub fn job_registry_mut(&mut self) -> &mut JobRegistry {
        &mut self.job_registry
    }

    /// Get execution history
    pub fn history(&self) -> &ExecutionHistory {
        &self.history
    }

    /// Get mutable execution history
    #[allow(dead_code)]
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
        let session = self.state_graph.current_session();
        let selected_ws = self.selected_workspaces.first();
        StatusSummary {
            initialized: self.initialized,
            discovering: self.discovering,
            discovery_error: self.discovery_error.clone(),
            workspace_count: self.available_workspaces.len(),
            selected_workspace: selected_ws.map(|w| w.name.clone()),
            selected_workspace_id: selected_ws.map(|w| w.workspace_id.clone()),
            selected_count: self.selected_workspaces.len(),
            loaded_pack: self.loaded_pack.as_ref().map(|p| p.pack.name.clone()),
            running_jobs: self.job_registry.running_count(),
            session_name: session.name.clone(),
            session_inputs: session.inputs.len(),
            session_steps: session.steps.len(),
            // State graph info
            state_id: self.state_graph.current_id().value(),
            current_checkpoint: self.state_graph.current_checkpoint().map(|s| s.to_string()),
            // Schema status
            schema_status: self.selected_workspace_schema_status(),
            schema_capturing_count: self.schema_capturing.len(),
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
#[allow(dead_code)]
pub struct StatusSummary {
    pub initialized: bool,
    pub discovering: bool,
    pub discovery_error: Option<String>,
    pub workspace_count: usize,
    pub selected_workspace: Option<String>,
    pub selected_workspace_id: Option<String>,
    pub selected_count: usize,
    pub loaded_pack: Option<String>,
    pub running_jobs: usize,
    pub session_name: Option<String>,
    pub session_inputs: usize,
    pub session_steps: usize,
    /// Current state ID in the exploring interpreter
    pub state_id: u64,
    /// Name of checkpoint at current state, if any
    pub current_checkpoint: Option<String>,
    /// Schema status for the selected workspace
    pub schema_status: Option<SchemaStatus>,
    /// Number of workspaces currently being schema captured
    pub schema_capturing_count: usize,
}

impl StatusSummary {
    /// Format as a status line
    #[allow(dead_code)]
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

        // Show schema status for selected workspace
        if let Some(status) = &self.schema_status {
            let icon = match status {
                SchemaStatus::Available => "\x1b[32m\u{25cf}\x1b[0m", // Green dot
                SchemaStatus::Capturing => "\x1b[33m\u{25cb}\x1b[0m", // Yellow circle
                SchemaStatus::Stale => "\x1b[33m\u{25cf}\x1b[0m",     // Yellow dot
                SchemaStatus::None => "\x1b[31m\u{25cb}\x1b[0m",      // Red circle
            };
            parts.push(format!("schema:{}", icon));
        }

        if let Some(pack) = &self.loaded_pack {
            parts.push(format!("pack:{}", pack));
        }

        // Show session info if there are inputs or steps
        if self.session_inputs > 0 || self.session_steps > 0 {
            parts.push(format!("{} steps", self.session_steps));
        }

        if self.running_jobs > 0 {
            parts.push(format!("jobs:{}", self.running_jobs));
        }

        // Show schema capture progress if any
        if self.schema_capturing_count > 0 {
            parts.push(format!("capturing:{}", self.schema_capturing_count));
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
        assert_eq!(summary.state_id, 0);
        assert_eq!(summary.as_status_line(), "not connected");
    }

    #[test]
    fn test_output_dir_default() {
        let ctx = ReplContext::new();
        assert!(ctx.output_dir().to_string_lossy().contains("kql-panopticon"));
    }

    #[test]
    fn test_state_graph_integration() {
        use crate::session::{InputDef, InputType};

        let mut ctx = ReplContext::new();
        assert_eq!(ctx.current_state_id().value(), 0);

        // Commit first (creates new state), then modify
        ctx.commit_state("input test");
        ctx.pack_session_mut().add_input(InputDef {
            name: "test".to_string(),
            input_type: InputType::String,
            description: None,
            required: true,
            default: None,
        });

        assert_eq!(ctx.current_state_id().value(), 1);

        // Revert
        assert!(ctx.revert_state());
        assert_eq!(ctx.current_state_id().value(), 0);
        assert!(ctx.pack_session().inputs.is_empty());
    }

    #[test]
    fn test_checkpoints() {
        let mut ctx = ReplContext::new();

        ctx.commit_state("cmd1");
        ctx.save_checkpoint("test-checkpoint");

        assert_eq!(ctx.current_checkpoint(), Some("test-checkpoint"));

        ctx.commit_state("cmd2");
        assert!(ctx.current_checkpoint().is_none());

        assert!(ctx.restore_checkpoint("test-checkpoint"));
        assert_eq!(ctx.current_state_id().value(), 1);
    }
}
