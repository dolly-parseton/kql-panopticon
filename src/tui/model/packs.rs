use crate::query_pack::QueryPack;
use ratatui::widgets::TableState;
use std::path::PathBuf;

/// Query Packs tab state
#[derive(Debug, Clone)]
pub struct PacksModel {
    /// List of available query packs with their file paths
    pub packs: Vec<PackEntry>,
    /// Table state for scrolling
    pub table_state: TableState,
    /// Loading state
    pub loading: bool,
    /// Error message if pack loading failed
    pub error: Option<String>,
}

/// A query pack entry in the browser
#[derive(Debug, Clone)]
pub struct PackEntry {
    /// Full path to the pack file
    pub path: PathBuf,
    /// Loaded pack (lazy-loaded when selected)
    pub pack: Option<QueryPack>,
    /// Relative path from packs directory (for display)
    pub relative_path: String,
    /// Load error if pack failed to parse
    pub load_error: Option<String>,
}

impl PacksModel {
    /// Create a new PacksModel
    pub fn new() -> Self {
        Self {
            packs: Vec::new(),
            table_state: TableState::default(),
            loading: false,
            error: None,
        }
    }

    /// Refresh the list of packs from disk
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
                self.error = Some(format!("Failed to load packs: {}", e));
            }
        }

        self.loading = false;
    }

    /// Load all packs from the library directory
    fn load_packs_from_library(&self) -> crate::error::Result<Vec<PackEntry>> {
        let pack_paths = QueryPack::list_library_packs()?;
        let library_root = QueryPack::get_library_path("")?;

        let mut entries = Vec::new();

        for path in pack_paths {
            // Compute relative path for display
            let relative_path = path
                .strip_prefix(&library_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            entries.push(PackEntry {
                path: path.clone(),
                pack: None, // Lazy load when needed
                relative_path,
                load_error: None,
            });
        }

        // Sort by relative path
        entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        Ok(entries)
    }

    /// Get the currently selected pack entry
    pub fn get_selected_entry(&self) -> Option<&PackEntry> {
        self.table_state.selected().and_then(|i| self.packs.get(i))
    }

    /// Get the currently selected pack entry (mutable)
    pub fn get_selected_entry_mut(&mut self) -> Option<&mut PackEntry> {
        self.table_state.selected().and_then(|i| self.packs.get_mut(i))
    }

    /// Load the pack data for the selected entry (lazy loading)
    pub fn load_selected_pack(&mut self) -> crate::error::Result<()> {
        if let Some(entry) = self.get_selected_entry_mut() {
            if entry.pack.is_none() && entry.load_error.is_none() {
                match QueryPack::load_from_file(&entry.path) {
                    Ok(pack) => {
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
}

impl Default for PacksModel {
    fn default() -> Self {
        Self::new()
    }
}

impl PackEntry {
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
    #[allow(dead_code)]
    pub fn get_description(&self) -> Option<&str> {
        self.pack.as_ref()?.description.as_deref()
    }

    /// Get the number of queries in the pack
    pub fn get_query_count(&self) -> Option<usize> {
        self.pack.as_ref().map(|p| p.get_queries().len())
    }
}
