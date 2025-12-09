//! Condition evaluation for investigation packs
//!
//! This module provides condition evaluation used by verdict rules,
//! scoring indicators, and conditional step execution (`when` clauses).
//!
//! # Supported Syntax
//!
//! - Literals: `true`, `false`
//! - Boolean operators: `and`, `or`, `not`
//! - Length checks: `step_name.length == 0`, `step_name.length > 5`
//! - Field comparisons: `step_name.field == value`, `step_name.field > 10`
//! - Array indexing: `step_name[0].field == value`
//! - Predicates: `step_name.any(field == value)`, `step_name.all(field > 0)`
//! - Operators: `==`, `!=`, `>`, `<`, `>=`, `<=`

use log::warn;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

// Pre-compiled regex patterns for condition parsing
static RE_ANY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\.any\((.+)\)$").expect("Invalid ANY regex")
});
static RE_ALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\.all\((.+)\)$").expect("Invalid ALL regex")
});
static RE_LENGTH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\.length\s*(==|>|<|>=|<=)\s*(\d+)$").expect("Invalid LENGTH regex")
});
static RE_INDEXED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\[(\d+)\]\.(\w+)\s*(==|!=|>|<|>=|<=)\s*(.+)$").expect("Invalid INDEXED regex")
});
static RE_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\.(\w+)\s*(==|!=|>|<|>=|<=)\s*(.+)$").expect("Invalid FIELD regex")
});
static RE_ROW_CONDITION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\s*(==|!=|>|<|>=|<=)\s*(.+)$").expect("Invalid ROW_CONDITION regex")
});

/// Evaluate a condition expression against step results.
///
/// Returns `true` if the condition is satisfied, `false` otherwise.
/// Unknown or unparseable conditions log a warning and return `false`.
///
/// # Arguments
///
/// * `condition` - The condition expression string to evaluate
/// * `step_results` - Map of step names to their result rows
///
/// # Examples
///
/// ```ignore
/// // Check if any email has a high threat rate
/// evaluate_condition("email_details.any(ThreatRate > 50)", &results);
///
/// // Check if no click activity exists
/// evaluate_condition("click_activity.length == 0", &results);
///
/// // Combine conditions
/// evaluate_condition("email_details.AuthPassed == true and click_activity.length == 0", &results);
/// ```
pub fn evaluate_condition(
    condition: &str,
    step_results: &HashMap<String, Vec<serde_json::Value>>,
) -> bool {
    let condition = condition.trim();

    // Handle literal true/false
    if condition == "true" {
        return true;
    }
    if condition == "false" {
        return false;
    }

    // Handle "and" conditions
    if condition.contains(" and ") {
        return condition
            .split(" and ")
            .all(|part| evaluate_condition(part.trim(), step_results));
    }

    // Handle "or" conditions
    if condition.contains(" or ") {
        return condition
            .split(" or ")
            .any(|part| evaluate_condition(part.trim(), step_results));
    }

    // Handle "not" prefix
    if let Some(rest) = condition.strip_prefix("not ") {
        return !evaluate_condition(rest.trim(), step_results);
    }

    // Handle step_name.any(condition) - check if any row matches
    if let Some(captures) = RE_ANY.captures(condition) {
        let step_name = captures.get(1).expect("regex group 1").as_str();
        let inner_condition = captures.get(2).expect("regex group 2").as_str();

        return step_results
            .get(step_name)
            .map(|rows| rows.iter().any(|row| evaluate_row_condition(inner_condition, row)))
            .unwrap_or(false);
    }

    // Handle step_name.all(condition) - check if all rows match
    if let Some(captures) = RE_ALL.captures(condition) {
        let step_name = captures.get(1).expect("regex group 1").as_str();
        let inner_condition = captures.get(2).expect("regex group 2").as_str();

        return step_results
            .get(step_name)
            .map(|rows| !rows.is_empty() && rows.iter().all(|row| evaluate_row_condition(inner_condition, row)))
            .unwrap_or(false);
    }

    // Handle step_name.length == N
    if let Some(captures) = RE_LENGTH.captures(condition) {
        let step_name = captures.get(1).expect("regex group 1").as_str();
        let operator = captures.get(2).expect("regex group 2").as_str();
        let value: usize = captures.get(3).expect("regex group 3").as_str().parse().unwrap_or(0);

        let length = step_results.get(step_name).map(|r| r.len()).unwrap_or(0);

        return match operator {
            "==" => length == value,
            ">" => length > value,
            "<" => length < value,
            ">=" => length >= value,
            "<=" => length <= value,
            _ => false,
        };
    }

    // Handle direct step_name[0].field comparisons
    if let Some(captures) = RE_INDEXED.captures(condition) {
        let step_name = captures.get(1).expect("regex group 1").as_str();
        let index: usize = captures.get(2).expect("regex group 2").as_str().parse().unwrap_or(0);
        let field = captures.get(3).expect("regex group 3").as_str();
        let operator = captures.get(4).expect("regex group 4").as_str();
        let expected = captures.get(5).expect("regex group 5").as_str().trim();

        return step_results
            .get(step_name)
            .and_then(|rows| rows.get(index))
            .map(|row| compare_field(row, field, operator, expected))
            .unwrap_or(false);
    }

    // Fallback: try to evaluate as a simple row condition on first row
    // e.g., step_name.field == value (implicitly step_name[0].field)
    if let Some(captures) = RE_FIELD.captures(condition) {
        let step_name = captures.get(1).expect("regex group 1").as_str();
        let field = captures.get(2).expect("regex group 2").as_str();
        let operator = captures.get(3).expect("regex group 3").as_str();
        let expected = captures.get(4).expect("regex group 4").as_str().trim();

        return step_results
            .get(step_name)
            .and_then(|rows| rows.first())
            .map(|row| compare_field(row, field, operator, expected))
            .unwrap_or(false);
    }

    warn!("Could not parse condition: {}", condition);
    false
}

/// Evaluate a condition against a single row object.
///
/// Parses simple `field operator value` expressions and compares against
/// the row's field value.
fn evaluate_row_condition(condition: &str, row: &serde_json::Value) -> bool {
    if let Some(captures) = RE_ROW_CONDITION.captures(condition.trim()) {
        let field = captures.get(1).expect("regex group 1").as_str();
        let operator = captures.get(2).expect("regex group 2").as_str();
        let expected = captures.get(3).expect("regex group 3").as_str().trim();

        return compare_field(row, field, operator, expected);
    }

    false
}

/// Compare a field value against an expected value.
///
/// Handles type coercion for booleans, numbers, and strings.
/// Returns `false` if the field doesn't exist or types are incompatible.
fn compare_field(
    row: &serde_json::Value,
    field: &str,
    operator: &str,
    expected: &str,
) -> bool {
    let actual = match row.get(field) {
        Some(v) => v,
        None => return false,
    };

    // Parse expected value
    let expected_trimmed = expected.trim_matches(|c| c == '"' || c == '\'');

    // Handle boolean comparisons
    if expected_trimmed == "true" || expected_trimmed == "false" {
        let expected_bool = expected_trimmed == "true";
        if let Some(actual_bool) = actual.as_bool() {
            return match operator {
                "==" => actual_bool == expected_bool,
                "!=" => actual_bool != expected_bool,
                _ => false,
            };
        }
    }

    // Handle numeric comparisons
    if let Ok(expected_num) = expected_trimmed.parse::<f64>() {
        let actual_num = actual.as_f64().or_else(|| {
            actual.as_i64().map(|i| i as f64)
        }).or_else(|| {
            actual.as_str().and_then(|s| s.parse::<f64>().ok())
        });

        if let Some(actual_num) = actual_num {
            return match operator {
                "==" => (actual_num - expected_num).abs() < f64::EPSILON,
                "!=" => (actual_num - expected_num).abs() >= f64::EPSILON,
                ">" => actual_num > expected_num,
                "<" => actual_num < expected_num,
                ">=" => actual_num >= expected_num,
                "<=" => actual_num <= expected_num,
                _ => false,
            };
        }
    }

    // String comparison
    let actual_str = match actual {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        _ => actual.to_string(),
    };

    match operator {
        "==" => actual_str == expected_trimmed,
        "!=" => actual_str != expected_trimmed,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_step_results() -> HashMap<String, Vec<serde_json::Value>> {
        let mut results = HashMap::new();

        results.insert("email_details".to_string(), vec![
            serde_json::json!({
                "Subject": "Test Email",
                "ThreatRate": 5.0,
                "InboxRate": 95.0,
                "AuthPassed": true,
                "UrlCount": 10,
            }),
        ]);

        results.insert("click_activity".to_string(), vec![
            serde_json::json!({
                "HighRisk_UserClickedThrough": false,
                "TotalClicks": 0,
            }),
        ]);

        results
    }

    #[test]
    fn test_evaluate_simple_condition() {
        let results = create_test_step_results();

        // Test literal true/false
        assert!(evaluate_condition("true", &results));
        assert!(!evaluate_condition("false", &results));

        // Test field comparison
        assert!(evaluate_condition("email_details.InboxRate > 90", &results));
        assert!(!evaluate_condition("email_details.InboxRate < 90", &results));

        // Test boolean field
        assert!(evaluate_condition("email_details.AuthPassed == true", &results));
    }

    #[test]
    fn test_evaluate_and_or_conditions() {
        let results = create_test_step_results();

        // Test AND
        assert!(evaluate_condition(
            "email_details.InboxRate > 90 and email_details.AuthPassed == true",
            &results
        ));

        // Test OR
        assert!(evaluate_condition(
            "email_details.InboxRate < 50 or email_details.AuthPassed == true",
            &results
        ));
    }

    #[test]
    fn test_evaluate_length_condition() {
        let results = create_test_step_results();

        assert!(evaluate_condition("email_details.length == 1", &results));
        assert!(evaluate_condition("click_activity.length > 0", &results));
        assert!(evaluate_condition("nonexistent.length == 0", &results));
    }

    #[test]
    fn test_evaluate_any_condition() {
        let results = create_test_step_results();

        assert!(evaluate_condition("email_details.any(UrlCount > 5)", &results));
        assert!(!evaluate_condition("email_details.any(UrlCount > 20)", &results));
    }
}
