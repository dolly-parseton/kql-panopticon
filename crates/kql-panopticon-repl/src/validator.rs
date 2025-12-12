//! Line continuation validator for multi-line input
//!
//! Detects trailing backslashes and signals reedline to continue
//! reading input until a complete command is entered.

use reedline::{ValidationResult, Validator};

/// Validator that detects line continuation with trailing backslash
///
/// When input ends with `\` (ignoring trailing whitespace), the validator
/// signals that more input is needed. This allows multi-line queries like:
///
/// ```text
/// query events = "SecurityEvent \
///     | where EventID == 4625 \
///     | project Account, IpAddress"
/// ```
#[derive(Debug, Clone, Default)]
pub struct LineContinuationValidator;

impl LineContinuationValidator {
    pub fn new() -> Self {
        Self
    }

    /// Check if input needs continuation (ends with unescaped backslash)
    fn needs_continuation(input: &str) -> bool {
        let trimmed = input.trim_end();

        // Check for trailing backslash that's not escaped (\\)
        if trimmed.ends_with('\\') {
            // Count consecutive backslashes at the end
            let backslash_count = trimmed
                .chars()
                .rev()
                .take_while(|&c| c == '\\')
                .count();

            // Odd number of backslashes means line continuation
            // Even number means escaped backslash
            backslash_count % 2 == 1
        } else {
            false
        }
    }
}

impl Validator for LineContinuationValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        if Self::needs_continuation(line) {
            ValidationResult::Incomplete
        } else {
            ValidationResult::Complete
        }
    }
}

/// Process a multi-line input by joining continuation lines
///
/// Removes trailing backslashes and joins lines, preserving the
/// content after the backslash on each line.
pub fn join_continuation_lines(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            // Check if this is a line continuation (backslash followed by newline)
            match chars.peek() {
                Some('\n') => {
                    // Skip the backslash and newline, add a space for separation
                    chars.next();
                    // Skip leading whitespace on the next line (but preserve some structure)
                    while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                        chars.next();
                    }
                    // Add a single space to separate the joined content
                    if !result.ends_with(' ') && !result.is_empty() {
                        result.push(' ');
                    }
                }
                Some('\r') => {
                    // Handle Windows-style line endings
                    chars.next(); // skip \r
                    if chars.peek() == Some(&'\n') {
                        chars.next(); // skip \n
                    }
                    while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                        chars.next();
                    }
                    if !result.ends_with(' ') && !result.is_empty() {
                        result.push(' ');
                    }
                }
                _ => {
                    // Not a line continuation, keep the backslash
                    result.push(c);
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_continuation_trailing_backslash() {
        assert!(LineContinuationValidator::needs_continuation("query x = \"test \\"));
        assert!(LineContinuationValidator::needs_continuation("test\\"));
        assert!(LineContinuationValidator::needs_continuation("test\\  ")); // trailing spaces
    }

    #[test]
    fn test_needs_continuation_no_backslash() {
        assert!(!LineContinuationValidator::needs_continuation("query x = \"test\""));
        assert!(!LineContinuationValidator::needs_continuation("test"));
        assert!(!LineContinuationValidator::needs_continuation(""));
    }

    #[test]
    fn test_needs_continuation_escaped_backslash() {
        // Double backslash is escaped, not continuation
        assert!(!LineContinuationValidator::needs_continuation("test\\\\"));
        // Triple backslash: \\ (escaped) + \ (continuation)
        assert!(LineContinuationValidator::needs_continuation("test\\\\\\"));
    }

    #[test]
    fn test_join_simple() {
        let input = "query x = \"test \\\n| where A == 1\"";
        let result = join_continuation_lines(input);
        assert_eq!(result, "query x = \"test | where A == 1\"");
    }

    #[test]
    fn test_join_multiple_lines() {
        let input = "query x = \"test \\\n| where A == 1 \\\n| take 10\"";
        let result = join_continuation_lines(input);
        assert_eq!(result, "query x = \"test | where A == 1 | take 10\"");
    }

    #[test]
    fn test_join_preserves_non_continuation_backslash() {
        let input = "query x = \"path\\\\file\"";
        let result = join_continuation_lines(input);
        assert_eq!(result, "query x = \"path\\\\file\"");
    }

    #[test]
    fn test_join_with_indentation() {
        let input = "query x = \"test \\\n    | where A == 1\"";
        let result = join_continuation_lines(input);
        assert_eq!(result, "query x = \"test | where A == 1\"");
    }

    #[test]
    fn test_join_windows_line_endings() {
        let input = "query x = \"test \\\r\n| where A == 1\"";
        let result = join_continuation_lines(input);
        assert_eq!(result, "query x = \"test | where A == 1\"");
    }
}
