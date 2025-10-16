use ratatui::widgets::ListState;

/// Settings state
#[derive(Debug, Clone)]
pub struct SettingsModel {
    /// Output folder path
    pub output_folder: String,
    /// Query timeout in seconds
    pub query_timeout_secs: u64,
    /// Number of retries for failed queries
    pub retry_count: u32,
    /// Validation interval in seconds
    pub validation_interval_secs: u64,
    /// Export results as CSV files
    pub export_csv: bool,
    /// Export results as JSON files
    pub export_json: bool,
    /// Parse nested dynamic fields into JSON objects (only for JSON export)
    pub parse_dynamics: bool,
    /// Currently selected setting index (0-6)
    pub selected_index: usize,
    /// List state for scrolling
    pub list_state: ListState,
    /// If currently editing, stores the input buffer
    pub editing: Option<String>,
}

impl SettingsModel {
    /// Create a new SettingsModel with default values
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            output_folder: "./output".to_string(),
            query_timeout_secs: 30,
            retry_count: 0,
            validation_interval_secs: 300,
            export_csv: true,     // CSV enabled by default
            export_json: false,   // JSON disabled by default
            parse_dynamics: true, // Parse dynamics enabled by default
            selected_index: 0,
            list_state,
            editing: None,
        }
    }

    /// Get the currently selected setting's value as a string
    pub fn get_selected_value(&self) -> String {
        match self.selected_index {
            0 => self.output_folder.clone(),
            1 => self.query_timeout_secs.to_string(),
            2 => self.retry_count.to_string(),
            3 => self.validation_interval_secs.to_string(),
            4 => if self.export_csv {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
            5 => if self.export_json {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
            6 => if self.parse_dynamics {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
            _ => String::new(),
        }
    }

    /// Check if the selected setting is a toggle (boolean)
    pub fn is_selected_toggle(&self) -> bool {
        matches!(self.selected_index, 4..=6)
    }

    /// Get the currently selected setting's name
    pub fn get_selected_name(&self) -> &'static str {
        match self.selected_index {
            0 => "Output Folder",
            1 => "Query Timeout (secs)",
            2 => "Retry Count",
            3 => "Validation Interval (secs)",
            4 => "Export CSV",
            5 => "Export JSON",
            6 => "Parse Dynamics (JSON only)",
            _ => "Unknown Setting",
        }
    }

    /// Get all settings as display strings
    pub fn get_all_settings(&self) -> Vec<String> {
        vec![
            format!("Output Folder: {}", self.output_folder),
            format!("Query Timeout (secs): {}", self.query_timeout_secs),
            format!("Retry Count: {}", self.retry_count),
            format!(
                "Validation Interval (secs): {}",
                self.validation_interval_secs
            ),
            format!(
                "Export CSV: {}",
                if self.export_csv { "[X]" } else { "[ ]" }
            ),
            format!(
                "Export JSON: {}",
                if self.export_json { "[X]" } else { "[ ]" }
            ),
            format!(
                "Parse Dynamics (JSON only): {}",
                if self.parse_dynamics { "[X]" } else { "[ ]" }
            ),
        ]
    }

    /// Toggle a boolean setting
    pub fn toggle_selected(&mut self) {
        match self.selected_index {
            4 => self.export_csv = !self.export_csv,
            5 => self.export_json = !self.export_json,
            6 => self.parse_dynamics = !self.parse_dynamics,
            _ => {}
        }
    }

    /// Attempt to save the edited value
    /// Returns Ok(()) if successful, Err(msg) if validation fails
    pub fn save_edit(&mut self, value: String) -> Result<(), String> {
        match self.selected_index {
            0 => {
                self.output_folder = value;
                Ok(())
            }
            1 => match value.parse::<u64>() {
                Ok(val) => {
                    self.query_timeout_secs = val;
                    Ok(())
                }
                Err(_) => Err("Invalid number format".to_string()),
            },
            2 => match value.parse::<u32>() {
                Ok(val) => {
                    self.retry_count = val;
                    Ok(())
                }
                Err(_) => Err("Invalid number format".to_string()),
            },
            3 => match value.parse::<u64>() {
                Ok(val) => {
                    self.validation_interval_secs = val;
                    Ok(())
                }
                Err(_) => Err("Invalid number format".to_string()),
            },
            4..=6 => {
                // Toggle settings - should use toggle_selected() instead
                Err("Use Space to toggle this setting".to_string())
            }
            _ => Err("Invalid setting index".to_string()),
        }
    }
}

impl Default for SettingsModel {
    fn default() -> Self {
        Self::new()
    }
}
