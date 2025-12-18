//! Pack loader modal state and logic

use kql_panopticon::pack::Pack;
use std::path::PathBuf;
use tui_tree_widget::TreeItem;
use walkdir::WalkDir;

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

/// Represents a discovered pack (file or folder)
#[derive(Debug, Clone)]
struct PackFile {
    /// Display name
    name: String,
    /// Full path to the pack
    path: PathBuf,
    /// Whether this is a folder pack (contains pack.yaml)
    is_folder_pack: bool,
}

impl PackLoaderState {
    /// Create a new pack loader state with given directory paths
    /// Always includes ~/.kql-panopticon/packs, with additional directories being additive
    pub fn new(additional_dirs: Vec<String>) -> Self {
        // Always start with the default directory
        let mut dirs = vec!["~/.kql-panopticon/packs".to_string()];

        // Add any additional directories
        dirs.extend(additional_dirs);

        let mut categories = Vec::new();

        for dir in dirs {
            let packs = Self::discover_packs_from_directory(&dir);
            categories.push(PackCategory {
                pattern: dir.clone(),
                packs,
            });
        }

        Self {
            categories,
            cursor: CursorPosition::Category(0),
        }
    }

    /// Discover valid packs from a directory (recursively)
    ///
    /// Finds both:
    /// - Folder packs: directories containing `pack.yaml`
    /// - File packs: `.yaml`/`.yml` files that are valid packs
    ///
    /// All discovered packs are validated using `Pack::load()` and only valid ones are returned.
    fn discover_packs_from_directory(dir_path: &str) -> Vec<PackFile> {
        // Expand tilde
        let expanded_path = if dir_path.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                dir_path.replacen("~", &home.display().to_string(), 1)
            } else {
                dir_path.to_string()
            }
        } else {
            dir_path.to_string()
        };

        let root = PathBuf::from(&expanded_path);
        if !root.exists() {
            log::debug!("Pack directory does not exist: {}", expanded_path);
            return Vec::new();
        }

        if !root.is_dir() {
            log::warn!("Pack path is not a directory: {}", expanded_path);
            return Vec::new();
        }

        let mut packs = Vec::new();
        let mut folder_pack_roots: Vec<PathBuf> = Vec::new();

        // First pass: find all folder packs (directories with pack.yaml)
        for entry in WalkDir::new(&root).follow_links(true).into_iter() {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    log::debug!("Error walking directory: {}", e);
                    continue;
                }
            };

            let path = entry.path();

            // Check for folder pack (directory with pack.yaml)
            if path.is_dir() {
                let pack_yaml = path.join("pack.yaml");
                if pack_yaml.exists() {
                    folder_pack_roots.push(path.to_path_buf());
                }
            }
        }

        // Second pass: collect and validate packs
        for entry in WalkDir::new(&root).follow_links(true).into_iter() {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    log::debug!("Error walking directory: {}", e);
                    continue;
                }
            };

            let path = entry.path();

            // Check if this is a folder pack root
            if folder_pack_roots.contains(&path.to_path_buf()) {
                // Validate and add the folder pack
                match Pack::load(path) {
                    Ok(pack) => {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.display().to_string());
                        packs.push(PackFile {
                            name: format!("{}/", name), // Trailing slash indicates folder
                            path: path.to_path_buf(),
                            is_folder_pack: true,
                        });
                        log::debug!("Found valid folder pack: {} ({})", pack.name, path.display());
                    }
                    Err(e) => {
                        log::debug!("Invalid folder pack at {}: {}", path.display(), e);
                    }
                }
                continue;
            }

            // Skip files inside folder packs
            if folder_pack_roots
                .iter()
                .any(|root| path.starts_with(root) && path != root)
            {
                continue;
            }

            // Check for file pack (.yaml/.yml files)
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "yaml" || ext == "yml" {
                        // Try to load as a pack
                        match Pack::load(path) {
                            Ok(pack) => {
                                let name = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| path.display().to_string());
                                packs.push(PackFile {
                                    name,
                                    path: path.to_path_buf(),
                                    is_folder_pack: false,
                                });
                                log::debug!(
                                    "Found valid file pack: {} ({})",
                                    pack.name,
                                    path.display()
                                );
                            }
                            Err(e) => {
                                log::debug!("Invalid pack file at {}: {}", path.display(), e);
                            }
                        }
                    }
                }
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
