//! Custom prompt for the shell REPL
//!
//! Shows context like current workspace, running jobs, etc.

use super::ShellContext;
use clap_repl::reedline::{Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus};
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Custom prompt that shows contextual information
pub struct PanopticonPrompt {
    context: Arc<RwLock<ShellContext>>,
}

impl PanopticonPrompt {
    pub fn new(context: Arc<RwLock<ShellContext>>) -> Self {
        Self { context }
    }

    /// Get prompt info synchronously (for reedline compatibility)
    fn get_prompt_info(&self) -> PromptInfo {
        // Try to get a read lock without blocking
        match self.context.try_read() {
            Ok(ctx) => {
                let workspace = ctx
                    .primary_workspace()
                    .map(|w| w.name.clone())
                    .unwrap_or_else(|| "no workspace".to_string());

                let running = ctx.running_job_count();
                let session = ctx.session_name.clone();
                let dirty = ctx.session_dirty;

                PromptInfo {
                    workspace,
                    running_jobs: running,
                    session,
                    dirty,
                }
            }
            Err(_) => PromptInfo::default(),
        }
    }
}

#[derive(Default)]
struct PromptInfo {
    workspace: String,
    running_jobs: usize,
    session: Option<String>,
    dirty: bool,
}

impl Prompt for PanopticonPrompt {
    fn render_prompt_left(&self) -> Cow<str> {
        let info = self.get_prompt_info();

        let running_indicator = if info.running_jobs > 0 {
            format!(" [{}⟳]", info.running_jobs)
        } else {
            String::new()
        };

        format!("panopticon{}>", running_indicator).into()
    }

    fn render_prompt_right(&self) -> Cow<str> {
        let info = self.get_prompt_info();

        let mut parts = Vec::new();

        // Session info
        if let Some(session) = &info.session {
            let dirty_marker = if info.dirty { "*" } else { "" };
            parts.push(format!("{}{}", session, dirty_marker));
        }

        // Workspace
        if !info.workspace.is_empty() && info.workspace != "no workspace" {
            parts.push(info.workspace);
        }

        if parts.is_empty() {
            Cow::Borrowed("")
        } else {
            Cow::Owned(parts.join(" | "))
        }
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<str> {
        Cow::Borrowed(" ")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<str> {
        Cow::Borrowed("... ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<str> {
        let prefix = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        Cow::Owned(format!(
            "({}reverse-search: {}) ",
            prefix, history_search.term
        ))
    }
}
