use crate::workspace::Workspace;
use ratatui::widgets::TableState;

/// Workspace with selection state
#[derive(Debug, Clone)]
pub struct WorkspaceState {
    pub workspace: Workspace,
    pub selected: bool,
}

/// Workspaces tab state
#[derive(Debug, Clone)]
pub struct WorkspacesModel {
    /// List of workspaces with selection state
    pub workspaces: Vec<WorkspaceState>,
    /// Table state for scrolling
    pub table_state: TableState,
}

impl WorkspacesModel {
    /// Create a new WorkspacesModel
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
            table_state: TableState::default(),
        }
    }

    /// Load workspaces from a list
    pub fn load_workspaces(&mut self, workspaces: Vec<Workspace>) {
        self.workspaces = workspaces
            .into_iter()
            .map(|w| WorkspaceState {
                workspace: w,
                selected: true, // Default all selected
            })
            .collect();

        // Set initial selection to first workspace if any exist
        if !self.workspaces.is_empty() {
            self.table_state.select(Some(0));
        }
    }

    /// Get selected workspaces
    pub fn get_selected_workspaces(&self) -> Vec<Workspace> {
        self.workspaces
            .iter()
            .filter(|ws| ws.selected)
            .map(|ws| ws.workspace.clone())
            .collect()
    }

    /// Toggle selection for a workspace at index
    pub fn toggle_selection(&mut self, index: usize) {
        if let Some(ws) = self.workspaces.get_mut(index) {
            ws.selected = !ws.selected;
        }
    }

    /// Select all workspaces
    pub fn select_all(&mut self) {
        for ws in &mut self.workspaces {
            ws.selected = true;
        }
    }

    /// Deselect all workspaces
    pub fn select_none(&mut self) {
        for ws in &mut self.workspaces {
            ws.selected = false;
        }
    }

    /// Get the count of selected workspaces
    pub fn selected_count(&self) -> usize {
        self.workspaces.iter().filter(|w| w.selected).count()
    }
}

impl Default for WorkspacesModel {
    fn default() -> Self {
        Self::new()
    }
}
