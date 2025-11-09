use crate::query_pack::PackQuery;
use tui_textarea::TextArea;

/// Query editor mode (Vim-style)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal, // Normal mode - navigation and commands
    Insert, // Insert mode - text editing
    Visual, // Visual mode - text selection
}

/// Pack context - tracks which query pack is currently loaded in the editor
#[derive(Debug, Clone)]
pub struct PackContext {
    /// Display name of the pack
    pub pack_name: String,
    /// Relative path from packs directory (for matching with PacksModel)
    pub pack_path: String,
    /// All queries in this pack
    pub queries: Vec<PackQuery>,
    /// Index of currently displayed query
    pub current_index: usize,
}

impl PackContext {
    /// Navigate to the next query in the pack
    pub fn next_query(&mut self) -> Option<&PackQuery> {
        if self.queries.is_empty() {
            return None;
        }
        self.current_index = (self.current_index + 1) % self.queries.len();
        Some(&self.queries[self.current_index])
    }

    /// Navigate to the previous query in the pack
    pub fn prev_query(&mut self) -> Option<&PackQuery> {
        if self.queries.is_empty() {
            return None;
        }
        if self.current_index == 0 {
            self.current_index = self.queries.len() - 1;
        } else {
            self.current_index -= 1;
        }
        Some(&self.queries[self.current_index])
    }

    /// Get the current query
    #[allow(dead_code)]
    pub fn current_query(&self) -> Option<&PackQuery> {
        self.queries.get(self.current_index)
    }

    /// Get display string for pack context (e.g., "Security Hunt (2/5)")
    pub fn display_string(&self) -> String {
        format!(
            "{} ({}/{})",
            self.pack_name,
            self.current_index + 1,
            self.queries.len()
        )
    }
}

/// Sort order for load panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadPanelSort {
    Status,       // Sort by job status
    Alphabetical, // Sort by job name
    Chronological, // Sort by creation time (order in list)
}

impl LoadPanelSort {
    /// Cycle to next sort order
    pub fn next(self) -> Self {
        match self {
            LoadPanelSort::Status => LoadPanelSort::Alphabetical,
            LoadPanelSort::Alphabetical => LoadPanelSort::Chronological,
            LoadPanelSort::Chronological => LoadPanelSort::Status,
        }
    }

    /// Get display name
    pub fn as_str(self) -> &'static str {
        match self {
            LoadPanelSort::Status => "Status",
            LoadPanelSort::Alphabetical => "Name",
            LoadPanelSort::Chronological => "Time",
        }
    }
}

/// Load panel state
#[derive(Debug, Clone)]
pub struct LoadPanelState {
    /// Selected job index (in the display/sorted list)
    pub selected: usize,
    /// Sort order
    pub sort: LoadPanelSort,
    /// Inverted sort order
    pub inverted: bool,
    /// Original query text (to restore on cancel)
    pub original_query: String,
    /// Cached sorted indices (maps display index -> original job index)
    pub sorted_indices: Vec<usize>,
}

/// Query tab state
pub struct QueryModel {
    /// Text area widget with full editor capabilities
    pub textarea: TextArea<'static>,
    /// Editor mode (Normal or Insert)
    pub mode: EditorMode,
    /// Job name input buffer (when executing)
    pub job_name_input: Option<String>,
    /// Load panel state (None = closed, Some = open)
    pub load_panel: Option<LoadPanelState>,
    /// Pack context (if query was loaded from a pack)
    pub pack_context: Option<PackContext>,
}

impl QueryModel {
    /// Create a new QueryModel
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(ratatui::style::Style::default());
        textarea.set_line_number_style(
            ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray),
        );

        Self {
            textarea,
            mode: EditorMode::Normal,
            job_name_input: None,
            load_panel: None,
            pack_context: None,
        }
    }

    /// Get the query text as a single string
    pub fn get_text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    /// Get a preview of the query (first N chars)
    pub fn get_preview(&self, max_len: usize) -> String {
        self.get_text().chars().take(max_len).collect()
    }

    /// Clear the query text
    pub fn clear(&mut self) {
        self.textarea = TextArea::default();
        self.textarea
            .set_cursor_line_style(ratatui::style::Style::default());
        self.textarea.set_line_number_style(
            ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray),
        );
    }

    /// Set query text from string
    pub fn set_text(&mut self, text: String) {
        let lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
        self.textarea = TextArea::from(lines);
        self.textarea
            .set_cursor_line_style(ratatui::style::Style::default());
        self.textarea.set_line_number_style(
            ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray),
        );
    }
}

impl Default for QueryModel {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadPanelState {
    /// Compute sorted indices based on current sort and inversion settings
    pub fn compute_sorted_indices(&self, jobs: &[crate::tui::model::jobs::JobState]) -> Vec<usize> {
        use crate::tui::model::jobs::JobStatus;

        let mut indices: Vec<usize> = (0..jobs.len()).collect();

        match self.sort {
            LoadPanelSort::Status => {
                indices.sort_by_key(|&idx| match jobs[idx].status {
                    JobStatus::Running => 0,
                    JobStatus::Queued => 1,
                    JobStatus::Failed => 2,
                    JobStatus::Completed => 3,
                });
            }
            LoadPanelSort::Alphabetical => {
                indices.sort_by(|&a, &b| jobs[a].workspace_name.cmp(&jobs[b].workspace_name));
            }
            LoadPanelSort::Chronological => {
                // Already in chronological order (no sorting needed)
            }
        }

        if self.inverted {
            indices.reverse();
        }

        indices
    }
}
