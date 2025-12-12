//! Helper utilities for pack authoring commands

use crate::session::InputType;

/// Check if a string is a valid identifier
pub fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut chars = s.chars();

    // First character must be a letter or underscore
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }

    // Remaining characters must be alphanumeric or underscore
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Infer input type from a string value
pub fn infer_input_type(value: &str) -> InputType {
    // Check for boolean
    if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
        return InputType::Bool;
    }

    // Check for integer
    if value.parse::<i64>().is_ok() {
        return InputType::Int;
    }

    // Check for timespan (ISO 8601 duration)
    if value.starts_with('P') && value.len() > 1 {
        return InputType::Timespan;
    }

    // Check for datetime (ISO 8601)
    if value.contains('T') && value.contains('-') {
        return InputType::Datetime;
    }

    // Default to string
    InputType::String
}

/// Truncate a value for display in trace
pub fn truncate_value(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_identifier() {
        assert!(is_valid_identifier("foo"));
        assert!(is_valid_identifier("foo_bar"));
        assert!(is_valid_identifier("foo123"));
        assert!(is_valid_identifier("_foo"));

        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("123foo"));
        assert!(!is_valid_identifier("foo-bar"));
        assert!(!is_valid_identifier("foo bar"));
    }

    #[test]
    fn test_infer_input_type() {
        assert_eq!(infer_input_type("hello"), InputType::String);
        assert_eq!(infer_input_type("123"), InputType::Int);
        assert_eq!(infer_input_type("true"), InputType::Bool);
        assert_eq!(infer_input_type("P7D"), InputType::Timespan);
        assert_eq!(
            infer_input_type("2024-01-15T10:30:00Z"),
            InputType::Datetime
        );
    }

    #[test]
    fn test_truncate_value() {
        assert_eq!(truncate_value("short", 10), "short");
        assert_eq!(truncate_value("this is a longer string", 10), "this is...");
    }
}
