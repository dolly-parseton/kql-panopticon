//! Dynamic prompt that reads from shared REPL context
//!
//! This allows the prompt to update in real-time as background
//! tasks complete, without waiting for user input.
//!
//! Prompt format (exploring interpreter pattern):
//! ```
//! panopticon #<state_id> [checkpoint] (workspace) >
//! ```

use crate::context::SharedContext;
use kql_panopticon_core::schema::SchemaStatus;
use reedline::{Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus};
use std::borrow::Cow;

/// Dynamic prompt that reads state from SharedContext
pub struct DynamicPrompt {
    ctx: SharedContext,
}

impl DynamicPrompt {
    pub fn new(ctx: SharedContext) -> Self {
        Self { ctx }
    }

    fn get_left_prompt(&self) -> String {
        // Try to read context (non-blocking)
        let Ok(ctx) = self.ctx.try_read() else {
            return "panopticon #0".to_string();
        };

        let summary = ctx.status_summary();

        // Build prompt: panopticon #N [checkpoint] (workspace) <schema_icon>
        let mut prompt = format!("panopticon #{}", summary.state_id);

        // Add checkpoint name if at a checkpoint
        if let Some(checkpoint) = &summary.current_checkpoint {
            prompt.push_str(&format!(" [{}]", checkpoint));
        }

        // Add workspace if selected
        if let Some(ws) = &summary.selected_workspace {
            if summary.selected_count > 1 {
                prompt.push_str(&format!(" ({}+{})", ws, summary.selected_count - 1));
            } else {
                prompt.push_str(&format!(" ({})", ws));
            }

            // Add schema status indicator for selected workspace
            // ● = green (available), ○ = yellow (capturing), ◐ = yellow (stale), ○ = red (none)
            if let Some(status) = &summary.schema_status {
                let icon = match status {
                    SchemaStatus::Available => "\x1b[32m●\x1b[0m",  // Green filled
                    SchemaStatus::Capturing => "\x1b[33m◐\x1b[0m",  // Yellow half
                    SchemaStatus::Stale => "\x1b[33m●\x1b[0m",       // Yellow filled
                    SchemaStatus::None => "\x1b[31m○\x1b[0m",        // Red empty
                };
                prompt.push(' ');
                prompt.push_str(icon);
                prompt.push(' ');
            }
        }

        prompt
    }

    fn get_right_prompt(&self) -> String {
        // Try to read context (non-blocking)
        let Ok(ctx) = self.ctx.try_read() else {
            return String::new();
        };

        let summary = ctx.status_summary();
        let mut parts = Vec::new();

        if summary.discovering {
            parts.push("discovering...".to_string());
        } else if let Some(err) = &summary.discovery_error {
            let truncated = if err.len() > 20 {
                format!("{}...", &err[..17])
            } else {
                err.clone()
            };
            parts.push(format!("! {}", truncated));
        } else if summary.initialized {
            parts.push(format!("{} ws", summary.workspace_count));
        }

        // Show schema capture progress
        if summary.schema_capturing_count > 0 {
            parts.push(format!("schema: {} capturing", summary.schema_capturing_count));
        }

        if summary.running_jobs > 0 {
            parts.push(format!("{} jobs", summary.running_jobs));
        }

        if let Some(pack) = &summary.loaded_pack {
            parts.push(format!("pack:{}", pack));
        }

        if parts.is_empty() {
            String::new()
        } else {
            format!("[{}]", parts.join(" | "))
        }
    }
}

impl Prompt for DynamicPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Owned(self.get_left_prompt())
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Owned(self.get_right_prompt())
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("> ")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed(". ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
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
