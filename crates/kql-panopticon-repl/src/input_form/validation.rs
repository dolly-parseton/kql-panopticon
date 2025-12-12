//! Input type validation for the input form widget.
//!
//! Provides type-specific validation for input values as users type.

use crate::session::InputType;

/// Validation result for an input value
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// Value is valid
    Valid,
    /// Value is invalid with a specific error message
    Invalid(String),
    /// Value is empty (validity depends on whether input is required)
    Empty,
}

impl ValidationResult {
    /// Check if the result indicates a valid value
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid | ValidationResult::Empty)
    }

    /// Get the error message if invalid
    pub fn error_message(&self) -> Option<&str> {
        match self {
            ValidationResult::Invalid(msg) => Some(msg),
            _ => None,
        }
    }
}

/// Validate a value against an input type
pub fn validate_input_value(value: &str, input_type: &InputType) -> ValidationResult {
    let value = value.trim();

    if value.is_empty() {
        return ValidationResult::Empty;
    }

    match input_type {
        InputType::String => ValidationResult::Valid,
        InputType::Int => validate_int(value),
        InputType::Bool => validate_bool(value),
        InputType::Datetime => validate_datetime(value),
        InputType::Timespan => validate_timespan(value),
    }
}

/// Validate an integer value
fn validate_int(value: &str) -> ValidationResult {
    // Allow optional leading minus sign
    let to_parse = value.trim_start_matches('-');

    if to_parse.is_empty() {
        return ValidationResult::Invalid("Expected numeric value".to_string());
    }

    if to_parse.chars().all(|c| c.is_ascii_digit()) {
        // Check if value fits in i64
        if value.parse::<i64>().is_ok() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid("Number too large".to_string())
        }
    } else {
        ValidationResult::Invalid("Must be a numeric value".to_string())
    }
}

/// Validate a boolean value
fn validate_bool(value: &str) -> ValidationResult {
    let lower = value.to_lowercase();
    match lower.as_str() {
        "true" | "false" | "1" | "0" | "yes" | "no" => ValidationResult::Valid,
        _ => ValidationResult::Invalid("Must be true or false".to_string()),
    }
}

/// Validate a datetime value
///
/// Accepts:
/// - ISO 8601 format: 2024-01-15T10:30:00Z
/// - KQL datetime literal: datetime(2024-01-15T10:30:00Z)
/// - Date only: 2024-01-15
fn validate_datetime(value: &str) -> ValidationResult {
    // Strip KQL datetime() wrapper if present
    let inner = if value.starts_with("datetime(") && value.ends_with(')') {
        &value[9..value.len() - 1]
    } else {
        value
    };

    // Try parsing as ISO 8601
    if is_valid_iso_datetime(inner) {
        return ValidationResult::Valid;
    }

    // Try parsing as date only
    if is_valid_date(inner) {
        return ValidationResult::Valid;
    }

    ValidationResult::Invalid("Expected datetime format: 2024-01-15T10:30:00Z".to_string())
}

/// Check if a string is a valid ISO 8601 datetime
fn is_valid_iso_datetime(s: &str) -> bool {
    // Simple pattern check: YYYY-MM-DDTHH:MM:SS with optional Z or timezone
    // Full regex would be overkill for inline validation

    if s.len() < 19 {
        return false;
    }

    let chars: Vec<char> = s.chars().collect();

    // Check YYYY-MM-DD
    if chars.get(4) != Some(&'-') || chars.get(7) != Some(&'-') {
        return false;
    }

    // Check T separator
    if chars.get(10) != Some(&'T') && chars.get(10) != Some(&' ') {
        return false;
    }

    // Check HH:MM:SS
    if chars.get(13) != Some(&':') || chars.get(16) != Some(&':') {
        return false;
    }

    // Check numeric parts
    let date_part = &s[0..10];
    let time_part = &s[11..19];

    // Verify date components are numeric
    let date_nums: Vec<&str> = date_part.split('-').collect();
    if date_nums.len() != 3 {
        return false;
    }
    for part in &date_nums {
        if !part.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }

    // Verify time components are numeric
    let time_nums: Vec<&str> = time_part.split(':').collect();
    if time_nums.len() != 3 {
        return false;
    }
    for part in &time_nums {
        // Handle fractional seconds
        let num_part = part.split('.').next().unwrap_or(part);
        if !num_part.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }

    true
}

/// Check if a string is a valid date (YYYY-MM-DD)
fn is_valid_date(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }

    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return false;
    }

    parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

/// Validate a timespan value
///
/// Accepts:
/// - KQL timespan literals: 1d, 12h, 30m, 45s, 1h30m
/// - ISO 8601 duration: P7D, PT1H, P1DT12H
fn validate_timespan(value: &str) -> ValidationResult {
    // Check for ISO 8601 duration (starts with P)
    if value.starts_with('P') {
        if is_valid_iso_duration(value) {
            return ValidationResult::Valid;
        }
    }

    // Check for KQL timespan literal
    if is_valid_kql_timespan(value) {
        return ValidationResult::Valid;
    }

    ValidationResult::Invalid("Expected timespan: 1d, 12h, P7D".to_string())
}

/// Check if a string is a valid ISO 8601 duration
fn is_valid_iso_duration(s: &str) -> bool {
    // P[n]Y[n]M[n]DT[n]H[n]M[n]S format
    if !s.starts_with('P') {
        return false;
    }

    let rest = &s[1..];
    if rest.is_empty() {
        return false;
    }

    // Split on T if present
    let (date_part, time_part) = if let Some(t_pos) = rest.find('T') {
        (&rest[..t_pos], Some(&rest[t_pos + 1..]))
    } else {
        (rest, None)
    };

    // Validate date part (Y, M, D, W designators)
    if !date_part.is_empty() && !is_valid_duration_part(date_part, &['Y', 'M', 'D', 'W']) {
        return false;
    }

    // Validate time part (H, M, S designators)
    if let Some(tp) = time_part {
        if tp.is_empty() || !is_valid_duration_part(tp, &['H', 'M', 'S']) {
            return false;
        }
    }

    true
}

/// Check if a duration part contains valid number+designator pairs
fn is_valid_duration_part(s: &str, designators: &[char]) -> bool {
    let mut num_buf = String::new();
    let mut found_any = false;

    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' {
            num_buf.push(c);
        } else if designators.contains(&c) {
            if num_buf.is_empty() {
                return false;
            }
            num_buf.clear();
            found_any = true;
        } else {
            return false;
        }
    }

    // Must have consumed all numbers
    num_buf.is_empty() && found_any
}

/// Check if a string is a valid KQL timespan literal
fn is_valid_kql_timespan(s: &str) -> bool {
    // KQL accepts: 1d, 12h, 30m, 45s, 100ms, 1000us, 1h30m, etc.
    // Also accepts negative: -1d

    let s = s.trim_start_matches('-');
    if s.is_empty() {
        return false;
    }

    let mut chars = s.chars().peekable();
    let mut found_unit = false;

    while chars.peek().is_some() {
        // Expect numeric value
        let mut has_digit = false;
        while chars.peek().map_or(false, |c| c.is_ascii_digit() || *c == '.') {
            chars.next();
            has_digit = true;
        }

        if !has_digit {
            return false;
        }

        // Expect unit (d, h, m, s, ms, us, tick)
        let unit: String = chars
            .by_ref()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();

        match unit.as_str() {
            "d" | "h" | "m" | "s" | "ms" | "us" | "tick" => {
                found_unit = true;
            }
            "" => return false,
            _ => return false,
        }
    }

    found_unit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_string() {
        assert_eq!(
            validate_input_value("anything", &InputType::String),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("", &InputType::String),
            ValidationResult::Empty
        );
    }

    #[test]
    fn test_validate_int() {
        assert_eq!(
            validate_input_value("42", &InputType::Int),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("-123", &InputType::Int),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("0", &InputType::Int),
            ValidationResult::Valid
        );
        assert!(matches!(
            validate_input_value("abc", &InputType::Int),
            ValidationResult::Invalid(_)
        ));
        assert!(matches!(
            validate_input_value("12.5", &InputType::Int),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn test_validate_bool() {
        assert_eq!(
            validate_input_value("true", &InputType::Bool),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("false", &InputType::Bool),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("TRUE", &InputType::Bool),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("1", &InputType::Bool),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("yes", &InputType::Bool),
            ValidationResult::Valid
        );
        assert!(matches!(
            validate_input_value("maybe", &InputType::Bool),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn test_validate_datetime() {
        assert_eq!(
            validate_input_value("2024-01-15T10:30:00Z", &InputType::Datetime),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("2024-01-15T10:30:00", &InputType::Datetime),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("datetime(2024-01-15T10:30:00Z)", &InputType::Datetime),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("2024-01-15", &InputType::Datetime),
            ValidationResult::Valid
        );
        assert!(matches!(
            validate_input_value("not-a-date", &InputType::Datetime),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn test_validate_timespan() {
        // KQL literals
        assert_eq!(
            validate_input_value("1d", &InputType::Timespan),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("12h", &InputType::Timespan),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("30m", &InputType::Timespan),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("1h30m", &InputType::Timespan),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("-1d", &InputType::Timespan),
            ValidationResult::Valid
        );

        // ISO 8601 durations
        assert_eq!(
            validate_input_value("P7D", &InputType::Timespan),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("PT1H", &InputType::Timespan),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_input_value("P1DT12H", &InputType::Timespan),
            ValidationResult::Valid
        );

        assert!(matches!(
            validate_input_value("not-a-timespan", &InputType::Timespan),
            ValidationResult::Invalid(_)
        ));
    }
}
