//! Theme selector state management

use crate::theme::ThemeCategory;
use std::collections::HashMap;
use tui_tree_widget::TreeItem;

#[derive(Debug, Clone)]
pub struct ThemeSelectorState {
    /// Themes grouped by category
    themes_by_category: HashMap<ThemeCategory, Vec<String>>,
    /// Current selection index in flat list
    current_index: usize,
    override_terminal_background: bool,
    /// Expanded category identifiers
    expanded: std::collections::HashSet<String>,
}

impl ThemeSelectorState {
    pub fn new(
        themes_by_category: HashMap<ThemeCategory, Vec<String>>,
        current_theme: &str,
        override_terminal_background: bool,
    ) -> Self {
        // Find current theme index in flat list
        let flat_themes = Self::build_flat_list(&themes_by_category);
        let current_index = flat_themes
            .iter()
            .position(|t| t == current_theme)
            .unwrap_or(0);

        // Start with all categories expanded
        let mut expanded = std::collections::HashSet::new();
        for category in ThemeCategory::all() {
            expanded.insert(category.id().to_string());
        }

        Self {
            themes_by_category,
            current_index,
            override_terminal_background,
            expanded,
        }
    }

    /// Build flat list of themes in consistent category order
    fn build_flat_list(themes_by_category: &HashMap<ThemeCategory, Vec<String>>) -> Vec<String> {
        let mut all_themes = Vec::new();
        for category in ThemeCategory::all() {
            if let Some(themes) = themes_by_category.get(&category) {
                all_themes.extend(themes.clone());
            }
        }
        all_themes
    }

    pub fn move_up(&mut self) {
        if self.current_index > 0 {
            self.current_index -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let flat_themes = Self::build_flat_list(&self.themes_by_category);
        if self.current_index < flat_themes.len().saturating_sub(1) {
            self.current_index += 1;
        }
    }

    pub fn selected_theme(&self) -> Option<String> {
        let flat_themes = Self::build_flat_list(&self.themes_by_category);
        flat_themes.get(self.current_index).cloned()
    }

    pub fn current_path(&self) -> Vec<String> {
        let flat_themes = Self::build_flat_list(&self.themes_by_category);
        if let Some(theme_name) = flat_themes.get(self.current_index) {
            // Find which category this theme belongs to and construct path
            for category in ThemeCategory::all() {
                if let Some(themes) = self.themes_by_category.get(&category) {
                    if themes.contains(theme_name) {
                        return vec![category.id().to_string(), theme_name.clone()];
                    }
                }
            }
        }
        vec![]
    }

    pub fn build_tree_items(&self) -> Vec<TreeItem<'static, String>> {
        let mut items = Vec::new();

        // Process categories in consistent order
        for category in ThemeCategory::all() {
            if let Some(themes) = self.themes_by_category.get(&category) {
                if !themes.is_empty() {
                    let children: Vec<TreeItem<'static, String>> = themes
                        .iter()
                        .map(|theme| TreeItem::new_leaf(theme.clone(), theme.clone()))
                        .collect();

                    items.push(
                        TreeItem::new(
                            category.id().to_string(),
                            category.to_string(),
                            children,
                        )
                        .expect("valid tree item"),
                    );
                }
            }
        }

        items
    }

    pub fn toggle_override_terminal_background(&mut self) {
        self.override_terminal_background = !self.override_terminal_background;
    }

    pub fn override_terminal_background(&self) -> bool {
        self.override_terminal_background
    }

    /// Get list of expanded category identifiers for the tree widget
    pub fn expanded_identifiers(&self) -> Vec<String> {
        self.expanded.iter().cloned().collect()
    }

    /// Toggle expand/collapse for a category (used with Right/Left arrow keys)
    pub fn toggle_expand(&mut self, category: ThemeCategory) {
        let id = category.id().to_string();
        if self.expanded.contains(&id) {
            self.expanded.remove(&id);
        } else {
            self.expanded.insert(id);
        }
    }

    /// Expand a category
    pub fn expand(&mut self, category: ThemeCategory) {
        self.expanded.insert(category.id().to_string());
    }

    /// Collapse a category
    pub fn collapse(&mut self, category: ThemeCategory) {
        self.expanded.remove(category.id());
    }
}
