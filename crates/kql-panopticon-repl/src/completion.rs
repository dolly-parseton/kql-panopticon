//! Completion logic for the REPL
//!
//! Provides context-aware completions independent of the UI layer.
//! Used by both the TUI shell and legacy reedline REPL.

use crate::commands::ReplCommand;
use crate::context::ReplContext;
use clap::{CommandFactory, Subcommand};
use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;

/// A completion suggestion
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// The text to insert
    pub value: String,
    /// Optional description/help text
    pub description: Option<String>,
    /// Start position of the text being replaced
    pub replace_start: usize,
    /// End position of the text being replaced
    pub replace_end: usize,
}

impl Suggestion {
    pub fn new(value: impl Into<String>, replace_start: usize, replace_end: usize) -> Self {
        Self {
            value: value.into(),
            description: None,
            replace_start,
            replace_end,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Get completions for the given input at the cursor position
pub fn get_completions(input: &str, cursor: usize, ctx: &ReplContext) -> Vec<Suggestion> {
    // First check for context-aware completions
    if let Some(suggestions) = complete_workspace(input, cursor, ctx) {
        return suggestions;
    }

    if let Some(suggestions) = complete_steps_inputs(input, cursor, ctx) {
        return suggestions;
    }

    // Fall back to clap completion
    complete_clap(input, cursor)
}

/// Complete workspace names for `workspace select` command
fn complete_workspace(input: &str, cursor: usize, ctx: &ReplContext) -> Option<Vec<Suggestion>> {
    let trimmed = input.trim_start();

    // Match "workspace select " or "ws select "
    let partial = if trimmed.starts_with("workspace select ") {
        trimmed.strip_prefix("workspace select ")
    } else if trimmed.starts_with("ws select ") {
        trimmed.strip_prefix("ws select ")
    } else {
        None
    }?;

    // Skip if user is typing a flag
    if partial.starts_with('-') {
        return None;
    }

    let workspaces = ctx.available_workspaces();

    if workspaces.is_empty() {
        return Some(vec![Suggestion {
            value: String::new(),
            description: Some("Run 'workspace list' first to discover workspaces".to_string()),
            replace_start: cursor,
            replace_end: cursor,
        }]);
    }

    // Calculate span for replacement
    let partial_trimmed = partial.trim();
    let replace_start = cursor - partial_trimmed.len();

    // Filter workspaces matching the partial input
    let suggestions = workspaces
        .iter()
        .filter(|ws| {
            partial_trimmed.is_empty()
                || ws.name.to_lowercase().contains(&partial_trimmed.to_lowercase())
        })
        .map(|ws| {
            Suggestion::new(&ws.name, replace_start, cursor)
                .with_description(format!("{} ({})", ws.subscription_name, ws.location))
        })
        .collect();

    Some(suggestions)
}

/// Complete step and input names for relevant commands
fn complete_steps_inputs(input: &str, cursor: usize, ctx: &ReplContext) -> Option<Vec<Suggestion>> {
    let trimmed = input.trim_start();

    // Commands that take step names
    let step_commands = ["edit ", "remove ", "steps --show ", "sample "];

    for cmd in &step_commands {
        if trimmed.starts_with(cmd) {
            let partial = trimmed.strip_prefix(cmd).unwrap_or("");
            if partial.starts_with('-') {
                continue;
            }

            let partial_trimmed = partial.trim();
            let replace_start = cursor - partial_trimmed.len();

            let session = ctx.pack_session();

            // Get matching steps
            let mut suggestions: Vec<Suggestion> = session
                .steps
                .keys()
                .filter(|name| {
                    partial_trimmed.is_empty()
                        || name.to_lowercase().contains(&partial_trimmed.to_lowercase())
                })
                .map(|name| {
                    Suggestion::new(name, replace_start, cursor)
                        .with_description("step")
                })
                .collect();

            // For edit/remove, also include inputs
            if *cmd == "edit " || *cmd == "remove " {
                let input_suggestions: Vec<Suggestion> = session
                    .inputs
                    .keys()
                    .filter(|name| {
                        partial_trimmed.is_empty()
                            || name.to_lowercase().contains(&partial_trimmed.to_lowercase())
                    })
                    .map(|name| {
                        Suggestion::new(name, replace_start, cursor)
                            .with_description("input")
                    })
                    .collect();
                suggestions.extend(input_suggestions);
            }

            if !suggestions.is_empty() {
                return Some(suggestions);
            }
        }
    }

    None
}

/// Get completions from clap for command structure
fn complete_clap(input: &str, cursor: usize) -> Vec<Suggestion> {
    let cmd = ReplCommand::command();
    let mut cmd = clap_complete::dynamic::command::CompleteCommand::augment_subcommands(cmd);

    let args = shlex::Shlex::new(input);
    let mut args = std::iter::once(String::new())
        .chain(args)
        .map(OsString::from)
        .collect::<Vec<_>>();

    if input.ends_with(' ') {
        args.push(OsString::new());
    }

    let arg_index = args.len() - 1;
    let arg_len = args.get(arg_index).map(|s| s.len()).unwrap_or(0);
    let replace_start = cursor.saturating_sub(arg_len);

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
        .map(|c| {
            let value = c.get_content().to_string_lossy().into_owned();
            let description = c.get_help().map(|x| x.to_string());
            Suggestion {
                value,
                description,
                replace_start,
                replace_end: cursor,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggestion_new() {
        let s = Suggestion::new("test", 0, 4);
        assert_eq!(s.value, "test");
        assert_eq!(s.replace_start, 0);
        assert_eq!(s.replace_end, 4);
        assert!(s.description.is_none());
    }

    #[test]
    fn test_suggestion_with_description() {
        let s = Suggestion::new("test", 0, 4).with_description("A test suggestion");
        assert_eq!(s.description, Some("A test suggestion".to_string()));
    }
}
