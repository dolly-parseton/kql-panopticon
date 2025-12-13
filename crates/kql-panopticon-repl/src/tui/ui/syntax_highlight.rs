//! Syntax highlighting for command input
//!
//! Tokenizes input and returns styled spans for rendering.
//! Uses shlex for tokenization and clap command structure for classification.

use crate::commands::ReplCommand;
use clap::CommandFactory;
use ratatui::style::{Color, Style};
use ratatui::text::Span;

/// Token types for syntax highlighting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Primary command (e.g., `query`, `workspace`, `run`)
    Command,
    /// Subcommand (e.g., `select`, `load`, `schema`)
    Subcommand,
    /// Flag (e.g., `--show`, `-q`)
    Flag,
    /// Argument value
    Argument,
    /// Quoted string
    String,
    /// Assignment operator (`=`)
    Operator,
    /// Whitespace between tokens
    Whitespace,
    /// Unknown/error token
    Unknown,
}

impl TokenKind {
    /// Get the style for this token kind
    pub fn style(self) -> Style {
        match self {
            TokenKind::Command => Style::default().fg(Color::Cyan),
            TokenKind::Subcommand => Style::default().fg(Color::Blue),
            TokenKind::Flag => Style::default().fg(Color::Yellow),
            TokenKind::Argument => Style::default().fg(Color::White),
            TokenKind::String => Style::default().fg(Color::Green),
            TokenKind::Operator => Style::default().fg(Color::Magenta),
            TokenKind::Whitespace => Style::default(),
            TokenKind::Unknown => Style::default().fg(Color::Red),
        }
    }
}

/// A highlighted token with its text and kind
#[derive(Debug, Clone)]
pub struct HighlightedToken {
    pub text: String,
    pub kind: TokenKind,
}

impl HighlightedToken {
    pub fn new(text: impl Into<String>, kind: TokenKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }

    /// Convert to a ratatui Span
    pub fn to_span(&self) -> Span<'_> {
        Span::styled(&self.text, self.kind.style())
    }
}

/// Highlight input text and return a list of tokens
pub fn highlight(input: &str) -> Vec<HighlightedToken> {
    if input.is_empty() {
        return vec![];
    }

    let mut tokens = Vec::new();
    let mut pos = 0;

    // Track parsing state
    let mut token_index = 0;
    let known_commands = get_known_commands();
    let mut current_command: Option<&str> = None;
    let mut subcommands: Vec<&'static str> = vec![];

    // Use shlex to tokenize
    let lexer = shlex::Shlex::new(input);
    let parsed_tokens: Vec<String> = lexer.collect();

    for token in &parsed_tokens {
        // Find this token in the original input (preserving whitespace)
        if let Some(start) = input[pos..].find(token.as_str()) {
            let actual_start = pos + start;

            // Add whitespace before token if any
            if actual_start > pos {
                tokens.push(HighlightedToken::new(
                    &input[pos..actual_start],
                    TokenKind::Whitespace,
                ));
            }

            // Classify and add the token
            let kind = classify_token(
                token,
                token_index,
                &known_commands,
                current_command,
                &subcommands,
            );

            // Track command context for subsequent tokens
            if token_index == 0 && kind == TokenKind::Command {
                current_command = known_commands.iter().find(|&&c| c == token.as_str()).copied();
                if let Some(cmd) = current_command {
                    subcommands = get_subcommands(cmd);
                }
            }

            // Check if this token is a subcommand for context
            if token_index == 1 && kind == TokenKind::Subcommand {
                // Could track subcommand context here for deeper nesting
            }

            // Handle quoted strings - check original input for quotes
            let original_text = if actual_start > 0 {
                // Check if preceded by quote in original
                let check_start = actual_start.saturating_sub(1);
                let after_end = (actual_start + token.len()).min(input.len());

                // Find the actual span including quotes
                find_quoted_span(input, actual_start, token)
            } else {
                None
            };

            let (token_text, token_kind) = if let Some((quoted_text, quoted_start)) = original_text {
                pos = quoted_start + quoted_text.len();
                (quoted_text, TokenKind::String)
            } else {
                pos = actual_start + token.len();
                (token.clone(), kind)
            };

            tokens.push(HighlightedToken::new(token_text, token_kind));
            token_index += 1;
        }
    }

    // Add any trailing whitespace
    if pos < input.len() {
        tokens.push(HighlightedToken::new(
            &input[pos..],
            TokenKind::Whitespace,
        ));
    }

    tokens
}

/// Find a quoted span in the original input
fn find_quoted_span(input: &str, token_start: usize, token: &str) -> Option<(String, usize)> {
    // Check for preceding quote
    if token_start > 0 {
        let prev_char = input.chars().nth(token_start - 1);
        if prev_char == Some('"') || prev_char == Some('\'') {
            let quote = prev_char.unwrap();
            let span_start = token_start - 1;

            // Find closing quote
            let after_token = token_start + token.len();
            if after_token < input.len() {
                let next_char = input.chars().nth(after_token);
                if next_char == Some(quote) {
                    let span_end = after_token + 1;
                    return Some((input[span_start..span_end].to_string(), span_start));
                }
            }

            // No closing quote yet - include what we have
            return Some((input[span_start..after_token].to_string(), span_start));
        }
    }
    None
}

/// Classify a token based on its position and content
fn classify_token(
    token: &str,
    index: usize,
    known_commands: &[&str],
    current_command: Option<&str>,
    subcommands: &[&'static str],
) -> TokenKind {
    // Flags always highlighted as flags
    if token.starts_with('-') {
        return TokenKind::Flag;
    }

    // Assignment operator
    if token == "=" {
        return TokenKind::Operator;
    }

    // First token is a command
    if index == 0 {
        if known_commands.contains(&token) {
            return TokenKind::Command;
        }
        return TokenKind::Unknown;
    }

    // Second token might be a subcommand
    if index == 1 && current_command.is_some() {
        if subcommands.contains(&token) {
            return TokenKind::Subcommand;
        }
    }

    // Check if it looks like a quoted value (shlex removes quotes)
    // This is handled separately in highlight() by checking original input

    TokenKind::Argument
}

/// Get list of known top-level commands from clap
fn get_known_commands() -> Vec<&'static str> {
    vec![
        "input", "in",
        "query",
        "inputs",
        "steps",
        "remove", "rm",
        "info",
        "new",
        "revert", "undo",
        "checkpoint", "cp",
        "trace",
        "workspace", "ws",
        "pack",
        "run",
        "sample",
        "validate",
        "jobs",
        "results",
        "history",
        "peek",
        "config",
        "status",
        "clear",
        "help",
        "exit", "quit", "q",
    ]
}

/// Get subcommands for a given command
fn get_subcommands(command: &str) -> Vec<&'static str> {
    match command {
        "workspace" | "ws" => vec!["list", "select", "schema"],
        "pack" => vec!["load", "save", "export"],
        "checkpoint" | "cp" => vec!["save", "restore", "list", "delete"],
        "jobs" => vec!["list", "cancel", "status"],
        _ => vec![],
    }
}

/// Convert highlighted tokens to spans for ratatui rendering
pub fn to_spans(tokens: &[HighlightedToken]) -> Vec<Span<'_>> {
    tokens.iter().map(|t| t.to_span()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_simple_command() {
        let tokens = highlight("status");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Command);
    }

    #[test]
    fn test_highlight_command_with_subcommand() {
        let tokens = highlight("workspace select");
        assert_eq!(tokens.len(), 3); // command, whitespace, subcommand
        assert_eq!(tokens[0].kind, TokenKind::Command);
        assert_eq!(tokens[1].kind, TokenKind::Whitespace);
        assert_eq!(tokens[2].kind, TokenKind::Subcommand);
    }

    #[test]
    fn test_highlight_command_with_flag() {
        let tokens = highlight("steps --show");
        assert_eq!(tokens[0].kind, TokenKind::Command);
        assert_eq!(tokens[2].kind, TokenKind::Flag);
    }

    #[test]
    fn test_highlight_unknown_command() {
        let tokens = highlight("foobar");
        assert_eq!(tokens[0].kind, TokenKind::Unknown);
    }

    #[test]
    fn test_highlight_empty() {
        let tokens = highlight("");
        assert!(tokens.is_empty());
    }
}
