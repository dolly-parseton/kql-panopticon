//! Pack loader modal state and logic

use std::path::PathBuf;
use tui_tree_widget::TreeItem;

/// State for the pack loader modal
#[derive(Debug)]
pub struct PackLoaderState {
    /// Pack files grouped by pattern/category
    categories: Vec<PackCategory>,
    /// Current cursor position (category index, pack index within category)
    /// None for pack index means cursor is on the category header
    cursor: CursorPosition,
}

/// Represents a category of packs from a glob pattern
#[derive(Debug, Clone)]
struct PackCategory {
    /// Original glob pattern (display name)
    pattern: String,
    /// Pack files found by this pattern
    packs: Vec<PackFile>,
}

/// Cursor position in the tree
#[derive(Debug, Clone)]
enum CursorPosition {
    /// On a category header
    Category(usize),
    /// On a pack within a category (category_idx, pack_idx)
    Pack(usize, usize),
}

/// Represents a discovered pack file
#[derive(Debug, Clone)]
struct PackFile {
    /// File name
    name: String,
    /// Full path to the file
    path: PathBuf,
}

impl PackLoaderState {
    /// Create a new pack loader state with given glob patterns
    /// Always includes ~/.kql-panopticon/packs/*.yaml, with additional patterns being additive
    pub fn new(additional_patterns: Vec<String>) -> Self {
        // Always start with the default pattern
        let mut patterns = vec!["~/.kql-panopticon/packs/*.yaml".to_string()];

        // Add any additional patterns
        patterns.extend(additional_patterns);

        let mut categories = Vec::new();

        for pattern in patterns {
            let packs = Self::discover_packs_from_glob(&pattern);
            categories.push(PackCategory {
                pattern: pattern.clone(),
                packs,
            });
        }

        Self {
            categories,
            cursor: CursorPosition::Category(0),
        }
    }

    /// Discover pack files matching the given glob pattern
    fn discover_packs_from_glob(pattern: &str) -> Vec<PackFile> {
        // Expand tilde
        let expanded_pattern = if pattern.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                pattern.replacen("~", &home.display().to_string(), 1)
            } else {
                pattern.to_string()
            }
        } else {
            pattern.to_string()
        };

        let mut packs = Vec::new();

        // Use glob to find matching files
        match glob::glob(&expanded_pattern) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(path) => {
                            // Filter for YAML files
                            if path.is_file() {
                                if let Some(ext) = path.extension() {
                                    if ext == "yaml" || ext == "yml" {
                                        if let Some(name) = path.file_name() {
                                            packs.push(PackFile {
                                                name: name.to_string_lossy().to_string(),
                                                path: path.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Glob entry error: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("Glob pattern error for '{}': {}", expanded_pattern, e);
            }
        }

        // Sort alphabetically
        packs.sort_by(|a, b| a.name.cmp(&b.name));

        packs
    }

    /// Move cursor up
    pub fn move_up(&mut self) {
        match &self.cursor {
            CursorPosition::Category(idx) => {
                if *idx > 0 {
                    self.cursor = CursorPosition::Category(idx - 1);
                }
            }
            CursorPosition::Pack(cat_idx, pack_idx) => {
                if *pack_idx > 0 {
                    self.cursor = CursorPosition::Pack(*cat_idx, pack_idx - 1);
                } else {
                    // Move to category header
                    self.cursor = CursorPosition::Category(*cat_idx);
                }
            }
        }
    }

    /// Move cursor down
    pub fn move_down(&mut self) {
        match &self.cursor {
            CursorPosition::Category(idx) => {
                if let Some(category) = self.categories.get(*idx) {
                    if !category.packs.is_empty() {
                        // Move into first pack of this category
                        self.cursor = CursorPosition::Pack(*idx, 0);
                    } else if *idx + 1 < self.categories.len() {
                        // Move to next category if current is empty
                        self.cursor = CursorPosition::Category(idx + 1);
                    }
                }
            }
            CursorPosition::Pack(cat_idx, pack_idx) => {
                if let Some(category) = self.categories.get(*cat_idx) {
                    if *pack_idx + 1 < category.packs.len() {
                        // Move to next pack in same category
                        self.cursor = CursorPosition::Pack(*cat_idx, pack_idx + 1);
                    } else if *cat_idx + 1 < self.categories.len() {
                        // Move to next category
                        self.cursor = CursorPosition::Category(cat_idx + 1);
                    }
                }
            }
        }
    }

    /// Get the currently selected pack path
    pub fn selected_pack_path(&self) -> Option<PathBuf> {
        match &self.cursor {
            CursorPosition::Category(_) => None,
            CursorPosition::Pack(cat_idx, pack_idx) => {
                self.categories
                    .get(*cat_idx)
                    .and_then(|cat| cat.packs.get(*pack_idx))
                    .map(|pack| pack.path.clone())
            }
        }
    }

    /// Build tree items for rendering
    pub fn build_tree_items(&self) -> Vec<TreeItem<'static, String>> {
        self.categories
            .iter()
            .enumerate()
            .map(|(cat_idx, category)| {
                let children: Vec<TreeItem<'static, String>> = category
                    .packs
                    .iter()
                    .enumerate()
                    .map(|(pack_idx, pack)| {
                        TreeItem::new_leaf(
                            format!("pack_{}_{}", cat_idx, pack_idx),
                            pack.name.clone(),
                        )
                    })
                    .collect();

                if children.is_empty() {
                    TreeItem::new_leaf(
                        format!("cat_{}", cat_idx),
                        format!("{} (no packs found)", category.pattern),
                    )
                } else {
                    TreeItem::new(
                        format!("cat_{}", cat_idx),
                        category.pattern.clone(),
                        children,
                    )
                    .expect("valid tree item")
                }
            })
            .collect()
    }

    /// Get the full path to the currently highlighted item for the tree widget
    pub fn current_path(&self) -> Vec<String> {
        match &self.cursor {
            CursorPosition::Category(idx) => vec![format!("cat_{}", idx)],
            CursorPosition::Pack(cat_idx, pack_idx) => {
                vec![format!("cat_{}", cat_idx), format!("pack_{}_{}", cat_idx, pack_idx)]
            }
        }
    }

    /// Get list of expanded identifiers (all categories are expanded)
    pub fn expanded_identifiers(&self) -> Vec<String> {
        (0..self.categories.len())
            .map(|idx| format!("cat_{}", idx))
            .collect()
    }

    /// Check if there are any packs across all categories
    pub fn is_empty(&self) -> bool {
        self.categories.iter().all(|cat| cat.packs.is_empty())
    }

    /// Get total pack count across all categories
    pub fn pack_count(&self) -> usize {
        self.categories.iter().map(|cat| cat.packs.len()).sum()
    }
}
