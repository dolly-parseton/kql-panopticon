use crate::investigation_pack::InvestigationPack;
use ratatui::widgets::TableState;
use std::collections::HashMap;
use std::path::PathBuf;

/// Investigations tab state
#[derive(Debug, Clone)]
pub struct InvestigationsModel {
    /// List of available investigation packs with their file paths
    pub packs: Vec<InvestigationEntry>,
    /// Table state for scrolling the pack list
    pub table_state: TableState,
    /// Loading state
    pub loading: bool,
    /// Error message if pack loading failed
    pub error: Option<String>,
    /// Active investigation execution state
    pub active: Option<ActiveInvestigation>,
    /// Input collection state (when prompting for inputs before execution)
    pub input_collection: Option<InputCollectionState>,
}

/// An investigation pack entry in the browser
#[derive(Debug, Clone)]
pub struct InvestigationEntry {
    /// Full path to the pack file
    pub path: PathBuf,
    /// Loaded pack (lazy-loaded when selected)
    pub pack: Option<InvestigationPack>,
    /// Relative path from investigations directory (for display)
    pub relative_path: String,
    /// Load error if pack failed to parse
    pub load_error: Option<String>,
    /// Validation error if pack is invalid
    pub validation_error: Option<String>,
}

/// State for collecting input values before execution
#[derive(Debug, Clone)]
pub struct InputCollectionState {
    /// The pack being configured
    pub pack_path: PathBuf,
    /// Input values collected so far
    pub inputs: HashMap<String, String>,
    /// Currently focused input index
    pub current_input: usize,
    /// Current input text being edited
    pub current_value: String,
    /// List of input names in order
    pub input_names: Vec<String>,
}

/// State of an active investigation execution
#[derive(Debug, Clone)]
pub struct ActiveInvestigation {
    /// Pack name
    pub name: String,
    /// Pack path
    pub pack_path: PathBuf,
    /// Steps in execution order
    pub steps: Vec<StepProgress>,
    /// Per-workspace progress
    pub workspace_progress: HashMap<String, WorkspaceProgress>,
    /// Overall status
    pub status: InvestigationStatus,
    /// Output folder
    pub output_folder: Option<PathBuf>,
}

/// Progress for a single step
#[derive(Debug, Clone)]
pub struct StepProgress {
    pub name: String,
    pub depends_on: Vec<String>,
    pub status: StepState,
}

/// State of a step
#[derive(Debug, Clone, PartialEq)]
pub enum StepState {
    Pending,
    Running,
    Completed { rows: usize },
    Failed { error: String },
    Skipped,
}

/// Per-workspace progress
#[derive(Debug, Clone)]
pub struct WorkspaceProgress {
    pub name: String,
    pub current_step: Option<String>,
    pub completed_steps: usize,
    pub total_steps: usize,
    pub status: InvestigationStatus,
}

/// Overall investigation status
#[derive(Debug, Clone, PartialEq)]
pub enum InvestigationStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl InvestigationsModel {
    /// Create a new InvestigationsModel
    pub fn new() -> Self {
        Self {
            packs: Vec::new(),
            table_state: TableState::default(),
            loading: false,
            error: None,
            active: None,
            input_collection: None,
        }
    }

    /// Refresh the list of investigation packs from disk
    pub fn refresh(&mut self) {
        self.loading = true;
        self.error = None;

        match self.load_packs_from_library() {
            Ok(packs) => {
                self.packs = packs;
                // Set initial selection if we have packs
                if !self.packs.is_empty() && self.table_state.selected().is_none() {
                    self.table_state.select(Some(0));
                }
            }
            Err(e) => {
                self.error = Some(format!("Failed to load investigations: {}", e));
            }
        }

        self.loading = false;
    }

    /// Load all investigation packs from the library directory
    fn load_packs_from_library(&self) -> crate::error::Result<Vec<InvestigationEntry>> {
        let pack_paths = InvestigationPack::list_library_packs()?;
        let library_root = InvestigationPack::get_library_path("")?;

        let mut entries = Vec::new();

        for path in pack_paths {
            // Compute relative path for display
            let relative_path = path
                .strip_prefix(&library_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            entries.push(InvestigationEntry {
                path: path.clone(),
                pack: None,
                relative_path,
                load_error: None,
                validation_error: None,
            });
        }

        // Sort by relative path
        entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        Ok(entries)
    }

    /// Get the currently selected pack entry
    pub fn get_selected_entry(&self) -> Option<&InvestigationEntry> {
        self.table_state.selected().and_then(|i| self.packs.get(i))
    }

    /// Get the currently selected pack entry (mutable)
    pub fn get_selected_entry_mut(&mut self) -> Option<&mut InvestigationEntry> {
        self.table_state
            .selected()
            .and_then(|i| self.packs.get_mut(i))
    }

    /// Load the pack data for the selected entry (lazy loading)
    pub fn load_selected_pack(&mut self) -> crate::error::Result<()> {
        if let Some(entry) = self.get_selected_entry_mut() {
            if entry.pack.is_none() && entry.load_error.is_none() {
                match InvestigationPack::load_from_file(&entry.path) {
                    Ok(pack) => {
                        // Validate the pack
                        if let Err(e) = pack.validate() {
                            entry.validation_error = Some(e.to_string());
                        }
                        entry.pack = Some(pack);
                    }
                    Err(e) => {
                        entry.load_error = Some(format!("Parse error: {}", e));
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }

    /// Navigate to the previous pack in the list
    pub fn previous(&mut self) {
        if self.packs.is_empty() {
            return;
        }

        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.packs.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    /// Navigate to the next pack in the list
    pub fn next(&mut self) {
        if self.packs.is_empty() {
            return;
        }

        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.packs.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    /// Get pack count
    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }

    /// Start input collection for the selected pack
    pub fn start_input_collection(&mut self) -> Option<()> {
        let entry = self.get_selected_entry()?;
        let pack = entry.pack.as_ref()?;

        if pack.inputs.is_empty() {
            return None; // No inputs needed
        }

        let input_names: Vec<String> = pack.inputs.iter().map(|i| i.name.clone()).collect();
        let mut inputs = HashMap::new();

        // Pre-populate with defaults
        for input in &pack.inputs {
            if let Some(default) = &input.default {
                inputs.insert(input.name.clone(), default.clone());
            }
        }

        self.input_collection = Some(InputCollectionState {
            pack_path: entry.path.clone(),
            inputs,
            current_input: 0,
            current_value: String::new(),
            input_names,
        });

        // Initialize current value from first input's default if available
        if let Some(state) = &mut self.input_collection {
            if let Some(first_name) = state.input_names.first() {
                if let Some(default) = state.inputs.get(first_name) {
                    state.current_value = default.clone();
                }
            }
        }

        Some(())
    }

    /// Move to next input in collection
    pub fn next_input(&mut self) {
        if let Some(state) = &mut self.input_collection {
            // Save current value
            if let Some(name) = state.input_names.get(state.current_input) {
                state.inputs.insert(name.clone(), state.current_value.clone());
            }

            // Move to next
            if state.current_input < state.input_names.len().saturating_sub(1) {
                state.current_input += 1;

                // Load value for new input
                if let Some(name) = state.input_names.get(state.current_input) {
                    state.current_value = state.inputs.get(name).cloned().unwrap_or_default();
                }
            }
        }
    }

    /// Move to previous input in collection
    pub fn prev_input(&mut self) {
        if let Some(state) = &mut self.input_collection {
            // Save current value
            if let Some(name) = state.input_names.get(state.current_input) {
                state.inputs.insert(name.clone(), state.current_value.clone());
            }

            // Move to previous
            if state.current_input > 0 {
                state.current_input -= 1;

                // Load value for new input
                if let Some(name) = state.input_names.get(state.current_input) {
                    state.current_value = state.inputs.get(name).cloned().unwrap_or_default();
                }
            }
        }
    }

    /// Finalize input collection and get the inputs
    pub fn finalize_inputs(&mut self) -> Option<HashMap<String, String>> {
        if let Some(mut state) = self.input_collection.take() {
            // Save current value
            if let Some(name) = state.input_names.get(state.current_input) {
                state.inputs.insert(name.clone(), state.current_value.clone());
            }
            Some(state.inputs)
        } else {
            None
        }
    }

    /// Cancel input collection
    pub fn cancel_input_collection(&mut self) {
        self.input_collection = None;
    }

    /// Check if currently collecting inputs
    pub fn is_collecting_inputs(&self) -> bool {
        self.input_collection.is_some()
    }
}

impl Default for InvestigationsModel {
    fn default() -> Self {
        Self::new()
    }
}

impl InvestigationEntry {
    /// Get the pack name (from metadata or filename)
    pub fn get_display_name(&self) -> String {
        if let Some(pack) = &self.pack {
            pack.name.clone()
        } else {
            // Use filename without extension as fallback
            self.path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        }
    }

    /// Get the pack description if available
    pub fn get_description(&self) -> Option<&str> {
        self.pack.as_ref()?.description.as_deref()
    }

    /// Get the number of steps in the pack
    pub fn get_step_count(&self) -> Option<usize> {
        self.pack.as_ref().map(|p| p.steps.len())
    }

    /// Get the number of inputs in the pack
    pub fn get_input_count(&self) -> Option<usize> {
        self.pack.as_ref().map(|p| p.inputs.len())
    }

    /// Check if pack has any errors
    pub fn has_error(&self) -> bool {
        self.load_error.is_some() || self.validation_error.is_some()
    }

    /// Get error message if any
    pub fn get_error(&self) -> Option<&str> {
        self.load_error
            .as_deref()
            .or(self.validation_error.as_deref())
    }
}
