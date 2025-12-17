//! Workspace selector modal state and logic

use kql_panopticon_core::Workspace;
use std::collections::{HashMap, HashSet};
use tui_tree_widget::TreeItem;

/// State for the workspace selector modal
#[derive(Debug)]
pub struct WorkspaceSelectorState {
    /// All available workspaces
    workspaces: Vec<Workspace>,
    /// Selected workspace IDs
    pub selected: HashSet<String>,
    /// Original selection (for cancel/revert)
    original_selected: HashSet<String>,
    /// Expanded subscription IDs
    expanded: HashSet<String>,
    /// Current cursor position (subscription index, workspace index within subscription)
    /// None for subscription row, Some for workspace row
    cursor: CursorPosition,
    /// Ordered list of subscription IDs (for consistent ordering)
    subscription_order: Vec<String>,
    /// Workspaces grouped by subscription
    by_subscription: HashMap<String, Vec<Workspace>>,
}

#[derive(Debug, Clone)]
enum CursorPosition {
    /// On a subscription header
    Subscription(usize),
    /// On a workspace within a subscription (sub_idx, ws_idx)
    Workspace(usize, usize),
}

impl WorkspaceSelectorState {
    /// Create a new workspace selector from a list of workspaces
    pub fn new(workspaces: Vec<Workspace>, previously_selected: HashSet<String>) -> Self {
        // Group by subscription
        let mut by_subscription: HashMap<String, Vec<Workspace>> = HashMap::new();
        for ws in &workspaces {
            by_subscription
                .entry(ws.subscription_name.clone())
                .or_default()
                .push(ws.clone());
        }

        // Sort subscriptions alphabetically
        let mut subscription_order: Vec<String> = by_subscription.keys().cloned().collect();
        subscription_order.sort();

        // Sort workspaces within each subscription
        for workspaces in by_subscription.values_mut() {
            workspaces.sort_by(|a, b| a.name.cmp(&b.name));
        }

        Self {
            workspaces,
            selected: previously_selected.clone(),
            original_selected: previously_selected,
            expanded: HashSet::new(), // All collapsed initially
            cursor: CursorPosition::Subscription(0),
            subscription_order,
            by_subscription,
        }
    }

    /// Move cursor up
    pub fn move_up(&mut self) {
        match &self.cursor {
            CursorPosition::Subscription(idx) => {
                if *idx > 0 {
                    // Check if previous subscription is expanded
                    let prev_sub = &self.subscription_order[idx - 1];
                    if self.expanded.contains(prev_sub) {
                        // Move to last workspace of previous subscription
                        let ws_count = self.by_subscription[prev_sub].len();
                        self.cursor = CursorPosition::Workspace(idx - 1, ws_count - 1);
                    } else {
                        self.cursor = CursorPosition::Subscription(idx - 1);
                    }
                }
            }
            CursorPosition::Workspace(sub_idx, ws_idx) => {
                if *ws_idx > 0 {
                    self.cursor = CursorPosition::Workspace(*sub_idx, ws_idx - 1);
                } else {
                    self.cursor = CursorPosition::Subscription(*sub_idx);
                }
            }
        }
    }

    /// Move cursor down
    pub fn move_down(&mut self) {
        match &self.cursor {
            CursorPosition::Subscription(idx) => {
                let sub_name = &self.subscription_order[*idx];
                if self.expanded.contains(sub_name) {
                    // Move into workspace list
                    self.cursor = CursorPosition::Workspace(*idx, 0);
                } else if *idx + 1 < self.subscription_order.len() {
                    self.cursor = CursorPosition::Subscription(idx + 1);
                }
            }
            CursorPosition::Workspace(sub_idx, ws_idx) => {
                let sub_name = &self.subscription_order[*sub_idx];
                let ws_count = self.by_subscription[sub_name].len();
                if *ws_idx + 1 < ws_count {
                    self.cursor = CursorPosition::Workspace(*sub_idx, ws_idx + 1);
                } else if *sub_idx + 1 < self.subscription_order.len() {
                    self.cursor = CursorPosition::Subscription(sub_idx + 1);
                }
            }
        }
    }

    /// Toggle expand/collapse for current subscription
    pub fn toggle_expand(&mut self) {
        let sub_idx = match &self.cursor {
            CursorPosition::Subscription(idx) => *idx,
            CursorPosition::Workspace(idx, _) => *idx,
        };
        let sub_name = &self.subscription_order[sub_idx];
        if self.expanded.contains(sub_name) {
            self.expanded.remove(sub_name);
            // Move cursor to subscription if on workspace
            if matches!(self.cursor, CursorPosition::Workspace(_, _)) {
                self.cursor = CursorPosition::Subscription(sub_idx);
            }
        } else {
            self.expanded.insert(sub_name.clone());
        }
    }

    /// Expand current subscription (right arrow)
    pub fn expand(&mut self) {
        if let CursorPosition::Subscription(idx) = &self.cursor {
            let sub_name = &self.subscription_order[*idx];
            self.expanded.insert(sub_name.clone());
        }
    }

    /// Collapse current subscription (left arrow)
    pub fn collapse(&mut self) {
        let sub_idx = match &self.cursor {
            CursorPosition::Subscription(idx) => *idx,
            CursorPosition::Workspace(idx, _) => *idx,
        };
        let sub_name = &self.subscription_order[sub_idx];
        self.expanded.remove(sub_name);
        self.cursor = CursorPosition::Subscription(sub_idx);
    }

    /// Toggle selection of current item
    pub fn toggle_selection(&mut self) {
        match &self.cursor {
            CursorPosition::Subscription(idx) => {
                // Toggle expand instead of selection
                self.toggle_expand();
            }
            CursorPosition::Workspace(sub_idx, ws_idx) => {
                let sub_name = &self.subscription_order[*sub_idx];
                let ws = &self.by_subscription[sub_name][*ws_idx];
                if self.selected.contains(&ws.workspace_id) {
                    self.selected.remove(&ws.workspace_id);
                } else {
                    self.selected.insert(ws.workspace_id.clone());
                }
            }
        }
    }

    /// Select all workspaces
    pub fn select_all(&mut self) {
        for ws in &self.workspaces {
            self.selected.insert(ws.workspace_id.clone());
        }
    }

    /// Deselect all workspaces
    pub fn select_none(&mut self) {
        self.selected.clear();
    }

    /// Cancel and revert to original selection
    pub fn cancel(&mut self) {
        self.selected = self.original_selected.clone();
    }

    /// Get selected workspaces
    pub fn get_selected_workspaces(&self) -> Vec<&Workspace> {
        self.workspaces
            .iter()
            .filter(|ws| self.selected.contains(&ws.workspace_id))
            .collect()
    }

    /// Build tree items for rendering
    pub fn build_tree_items(&self) -> Vec<TreeItem<'static, String>> {
        self.subscription_order
            .iter()
            .enumerate()
            .map(|(sub_idx, sub_name)| {
                let workspaces = &self.by_subscription[sub_name];
                let selected_count = workspaces
                    .iter()
                    .filter(|ws| self.selected.contains(&ws.workspace_id))
                    .count();

                let header = format!(
                    "{} ({}/{})",
                    sub_name,
                    selected_count,
                    workspaces.len()
                );

                let children: Vec<TreeItem<'static, String>> = workspaces
                    .iter()
                    .enumerate()
                    .map(|(ws_idx, ws)| {
                        let checkbox = if self.selected.contains(&ws.workspace_id) {
                            "[x]"
                        } else {
                            "[ ]"
                        };
                        let text = format!("{} {}", checkbox, ws.name);
                        TreeItem::new_leaf(format!("ws_{}_{}", sub_idx, ws_idx), text)
                    })
                    .collect();

                TreeItem::new(format!("sub_{}", sub_idx), header, children)
                    .expect("valid tree item")
            })
            .collect()
    }

    /// Get the full path to the currently highlighted item for the tree widget
    /// Returns vec of identifiers from root to current item
    pub fn current_path(&self) -> Vec<String> {
        match &self.cursor {
            CursorPosition::Subscription(idx) => vec![format!("sub_{}", idx)],
            CursorPosition::Workspace(sub_idx, ws_idx) => vec![
                format!("sub_{}", sub_idx),
                format!("ws_{}_{}", sub_idx, ws_idx),
            ],
        }
    }

    /// Get list of expanded identifiers for the tree widget
    pub fn expanded_identifiers(&self) -> Vec<String> {
        self.subscription_order
            .iter()
            .enumerate()
            .filter(|(_, name)| self.expanded.contains(*name))
            .map(|(idx, _)| format!("sub_{}", idx))
            .collect()
    }

    /// Check if there are any workspaces
    pub fn is_empty(&self) -> bool {
        self.workspaces.is_empty()
    }

    /// Get total workspace count
    pub fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    /// Get selected count
    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }
}
