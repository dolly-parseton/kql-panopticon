//! Custom tab completion for the REPL
//!
//! Provides context-aware completions for workspace names and other
//! dynamic values while delegating to clap for command structure.

use crate::commands::ReplCommand;
use crate::context::SharedContext;
use clap::{CommandFactory, Subcommand};
use reedline::{Completer, Span, Suggestion};
use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;

/// Custom completer with access to REPL context
pub struct PanopticonCompleter {
    ctx: SharedContext,
}

impl PanopticonCompleter {
    pub fn new(ctx: SharedContext) -> Self {
        Self { ctx }
    }

    /// Check if we're completing a workspace name and return suggestions
    fn complete_workspace(&self, line: &str, pos: usize) -> Option<Vec<Suggestion>> {
        // Match "workspace select " or "ws select "
        let trimmed = line.trim_start();
        let is_workspace_select = trimmed.starts_with("workspace select ")
            || trimmed.starts_with("ws select ");

        if !is_workspace_select {
            return None;
        }

        // Extract the partial workspace name being typed
        let partial = trimmed
            .strip_prefix("workspace select ")
            .or_else(|| trimmed.strip_prefix("ws select "))
            .unwrap_or("");

        // Skip if user is typing a flag
        if partial.starts_with('-') {
            return None;
        }

        // Try to get workspaces from context (non-blocking)
        let ctx_guard = self.ctx.try_read().ok()?;
        let workspaces = ctx_guard.available_workspaces();

        if workspaces.is_empty() {
            return Some(vec![Suggestion {
                value: "".to_string(),
                description: Some("Run 'workspace list' first to discover workspaces".to_string()),
                style: None,
                extra: None,
                span: Span::new(pos, pos),
                append_whitespace: false,
            }]);
        }

        // Calculate span for replacement
        let partial_trimmed = partial.trim();
        let span_start = pos - partial_trimmed.len();
        let span = Span::new(span_start, pos);

        // Filter workspaces matching the partial input
        let suggestions: Vec<Suggestion> = workspaces
            .iter()
            .filter(|ws| {
                partial_trimmed.is_empty()
                    || ws.name.to_lowercase().contains(&partial_trimmed.to_lowercase())
            })
            .map(|ws| Suggestion {
                value: ws.name.clone(),
                description: Some(format!("{} ({})", ws.subscription_name, ws.location)),
                style: None,
                extra: None,
                span,
                append_whitespace: true,
            })
            .collect();

        Some(suggestions)
    }

    /// Delegate to clap's completion for command structure
    fn complete_clap(&self, line: &str, pos: usize) -> Vec<Suggestion> {
        let cmd = ReplCommand::command();
        let mut cmd = clap_complete::dynamic::command::CompleteCommand::augment_subcommands(cmd);

        let args = shlex::Shlex::new(line);
        let mut args = std::iter::once("".to_owned())
            .chain(args)
            .map(OsString::from)
            .collect::<Vec<_>>();

        if line.ends_with(' ') {
            args.push(OsString::new());
        }

        let arg_index = args.len() - 1;
        let span = Span::new(pos - args[arg_index].len(), pos);

        let Ok(candidates) = clap_complete::dynamic::complete(
            &mut cmd,
            args,
            arg_index,
            PathBuf::from_str(".").ok().as_deref(),
        ) else {
            return vec![];
        };

        candidates
            .into_iter()
            .map(|c| Suggestion {
                value: c.get_content().to_string_lossy().into_owned(),
                description: c.get_help().map(|x| x.to_string()),
                style: None,
                extra: None,
                span,
                append_whitespace: true,
            })
            .collect()
    }
}

impl Completer for PanopticonCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        // First check for context-aware completions
        if let Some(suggestions) = self.complete_workspace(line, pos) {
            return suggestions;
        }

        // Fall back to clap completion
        self.complete_clap(line, pos)
    }
}
