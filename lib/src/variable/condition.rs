//! Condition evaluation for pack execution
//!
//! Evaluates condition expressions against step results for:
//! - `when` clauses (conditional step execution)
//! - Verdict rules (investigation outcomes)
//! - Scoring indicators (risk scoring)
//!
//! See the [README](./README.md) for complete syntax documentation.

use crate::execution::result::ResultContext;
use regex::Regex;
use serde_json::Value as JsonValue;
use std::sync::LazyLock;
use tracing::warn;

// Pre-compiled regex patterns for condition parsing
static RE_ANY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\.any\((.+)\)$").expect("Invalid ANY regex")
});
static RE_ALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\.all\((.+)\)$").expect("Invalid ALL regex")
});
static RE_LENGTH: LazyLock<Regex> = LazyLock::new(|| {
    // Order matters: >= before >, <= before <
    Regex::new(r"^(\w+)\.length\s*(==|!=|>=|<=|>|<)\s*(\d+)$").expect("Invalid LENGTH regex")
});
static RE_INDEXED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\[(\d+)\]\.(\w+)\s*(==|!=|>=|<=|>|<)\s*(.+)$")
        .expect("Invalid INDEXED regex")
});
static RE_FIRST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\.first\.(\w+)\s*(==|!=|>=|<=|>|<)\s*(.+)$").expect("Invalid FIRST regex")
});
static RE_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\.(\w+)\s*(==|!=|>=|<=|>|<)\s*(.+)$").expect("Invalid FIELD regex")
});
static RE_ROW_CONDITION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\s*(==|!=|>=|<=|>|<)\s*(.+)$").expect("Invalid ROW_CONDITION regex")
});
static RE_EMPTY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\w+)\s+is\s+(empty|not\s+empty)$").expect("Invalid EMPTY regex")
});

/// Evaluate a condition expression against step results.
///
/// Returns `true` if the condition is satisfied, `false` otherwise.
/// Unknown or unparseable conditions log a warning and return `false`.
///
/// # Arguments
///
/// * `condition` - The condition expression string to evaluate
/// * `step_results` - Result context with file-backed results
///
/// # Supported Syntax
///
/// - Literals: `true`, `false`
/// - Empty checks: `step is empty`, `step is not empty`
/// - Length: `step.length == 0`, `step.length > 5`
/// - First row: `step.first.field == value` (aligns with `{{step.first.field}}` variable syntax)
/// - Field (first row): `step.field == value`
/// - Indexed: `step[0].field == value`
/// - Predicates: `step.any(field > 0)`, `step.all(field == true)`
/// - Boolean: `cond1 and cond2`, `cond1 or cond2`, `not cond`
/// - Operators: `==`, `!=`, `>`, `<`, `>=`, `<=`
pub fn evaluate_condition(condition: &str, step_results: &ResultContext) -> bool {
    let condition = condition.trim();

    // Handle literal true/false
    if condition == "true" {
        return true;
    }
    if condition == "false" {
        return false;
    }

    // Handle "and" conditions (check for " and " to avoid matching words containing "and")
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

    // Handle "step is empty" / "step is not empty"
    if let Some(captures) = RE_EMPTY.captures(condition) {
        let step_name = captures.get(1).expect("regex group 1").as_str();
        let is_not_empty = captures
            .get(2)
            .expect("regex group 2")
            .as_str()
            .contains("not");

        let has_rows = step_results
            .get(step_name)
            .and_then(|handle| handle.is_empty().ok())
            .map(|empty| !empty)
            .unwrap_or(false);

        return if is_not_empty { has_rows } else { !has_rows };
    }

    // Handle step_name.any(condition) - check if any row matches
    if let Some(captures) = RE_ANY.captures(condition) {
        let step_name = captures.get(1).expect("regex group 1").as_str();
        let inner_condition = captures.get(2).expect("regex group 2").as_str();

        return step_results
            .get(step_name)
            .and_then(|handle| handle.iter_rows().ok())
            .map(|iter| {
                iter.filter_map(|r| r.ok())
                    .any(|row| evaluate_row_condition(inner_condition, &row))
            })
            .unwrap_or(false);
    }

    // Handle step_name.all(condition) - check if all rows match
    if let Some(captures) = RE_ALL.captures(condition) {
        let step_name = captures.get(1).expect("regex group 1").as_str();
        let inner_condition = captures.get(2).expect("regex group 2").as_str();

        return step_results
            .get(step_name)
            .and_then(|handle| {
                let is_empty = handle.is_empty().ok()?;
                if is_empty {
                    return Some(false);
                }
                let iter = handle.iter_rows().ok()?;
                Some(
                    iter.filter_map(|r| r.ok())
                        .all(|row| evaluate_row_condition(inner_condition, &row)),
                )
            })
            .unwrap_or(false);
    }

    // Handle step_name.length == N
    if let Some(captures) = RE_LENGTH.captures(condition) {
        let step_name = captures.get(1).expect("regex group 1").as_str();
        let operator = captures.get(2).expect("regex group 2").as_str();
        let value: usize = captures
            .get(3)
            .expect("regex group 3")
            .as_str()
            .parse()
            .unwrap_or(0);

        let length = step_results.row_count(step_name).unwrap_or(0);

        return compare_numbers(length as f64, operator, value as f64);
    }

    // Handle step_name.first.field comparisons (aligns with {{step.first.field}} variable syntax)
    if let Some(captures) = RE_FIRST.captures(condition) {
        let step_name = captures.get(1).expect("regex group 1").as_str();
        let field = captures.get(2).expect("regex group 2").as_str();
        let operator = captures.get(3).expect("regex group 3").as_str();
        let expected = captures.get(4).expect("regex group 4").as_str().trim();

        return step_results
            .first_n(step_name, 1)
            .ok()
            .and_then(|rows| rows.into_iter().next())
            .map(|row| compare_field(&row, field, operator, expected))
            .unwrap_or(false);
    }

    // Handle step_name[N].field comparisons
    if let Some(captures) = RE_INDEXED.captures(condition) {
        let step_name = captures.get(1).expect("regex group 1").as_str();
        let index: usize = captures
            .get(2)
            .expect("regex group 2")
            .as_str()
            .parse()
            .unwrap_or(0);
        let field = captures.get(3).expect("regex group 3").as_str();
        let operator = captures.get(4).expect("regex group 4").as_str();
        let expected = captures.get(5).expect("regex group 5").as_str().trim();

        return step_results
            .first_n(step_name, index + 1)
            .ok()
            .and_then(|rows| rows.into_iter().nth(index))
            .map(|row| compare_field(&row, field, operator, expected))
            .unwrap_or(false);
    }

    // Handle step_name.field comparisons (implicitly first row)
    if let Some(captures) = RE_FIELD.captures(condition) {
        let step_name = captures.get(1).expect("regex group 1").as_str();
        let field = captures.get(2).expect("regex group 2").as_str();
        let operator = captures.get(3).expect("regex group 3").as_str();
        let expected = captures.get(4).expect("regex group 4").as_str().trim();

        return step_results
            .first_n(step_name, 1)
            .ok()
            .and_then(|rows| rows.into_iter().next())
            .map(|row| compare_field(&row, field, operator, expected))
            .unwrap_or(false);
    }

    warn!("Could not parse condition: {}", condition);
    false
}

/// Evaluate a condition against a single row object.
///
/// Used internally for `any()` and `all()` predicates.
fn evaluate_row_condition(condition: &str, row: &JsonValue) -> bool {
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
fn compare_field(row: &JsonValue, field: &str, operator: &str, expected: &str) -> bool {
    let actual = match row.get(field) {
        Some(v) => v,
        None => return false,
    };

    // Strip quotes from expected value
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
        let actual_num = actual
            .as_f64()
            .or_else(|| actual.as_i64().map(|i| i as f64))
            .or_else(|| actual.as_str().and_then(|s| s.parse::<f64>().ok()));

        if let Some(actual_num) = actual_num {
            return compare_numbers(actual_num, operator, expected_num);
        }
    }

    // String comparison
    let actual_str = match actual {
        JsonValue::String(s) => s.clone(),
        JsonValue::Null => String::new(),
        _ => actual.to_string(),
    };

    match operator {
        "==" => actual_str == expected_trimmed,
        "!=" => actual_str != expected_trimmed,
        _ => false,
    }
}

/// Compare two numbers with the given operator
fn compare_numbers(actual: f64, operator: &str, expected: f64) -> bool {
    match operator {
        "==" => (actual - expected).abs() < f64::EPSILON,
        "!=" => (actual - expected).abs() >= f64::EPSILON,
        ">" => actual > expected,
        "<" => actual < expected,
        ">=" => actual >= expected,
        "<=" => actual <= expected,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::result::ResultWriter;

    /// Helper to create a step result handle with test data
    fn create_test_handle(
        dir: &std::path::Path,
        step_name: &str,
        rows: &[JsonValue],
    ) -> crate::execution::result::ResultHandle {
        let path = dir.join(format!("{}.jsonl", step_name));
        let mut writer = ResultWriter::new(&path, step_name).unwrap();
        writer.write_rows(rows).unwrap();
        writer.finish().unwrap()
    }

    fn create_test_results(temp_dir: &std::path::Path) -> ResultContext {
        let mut results = ResultContext::new();

        let users_handle = create_test_handle(
            temp_dir,
            "users",
            &[
                serde_json::json!({
                    "name": "Alice",
                    "score": 85,
                    "active": true,
                }),
                serde_json::json!({
                    "name": "Bob",
                    "score": 92,
                    "active": false,
                }),
            ],
        );
        results.insert("users", users_handle);

        let empty_handle = create_test_handle(temp_dir, "empty_step", &[]);
        results.insert("empty_step", empty_handle);

        results
    }

    #[test]
    fn test_literals() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());
        assert!(evaluate_condition("true", &results));
        assert!(!evaluate_condition("false", &results));
    }

    #[test]
    fn test_empty_syntax() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());

        // users has rows
        assert!(evaluate_condition("users is not empty", &results));
        assert!(!evaluate_condition("users is empty", &results));

        // empty_step has no rows
        assert!(evaluate_condition("empty_step is empty", &results));
        assert!(!evaluate_condition("empty_step is not empty", &results));

        // nonexistent step
        assert!(evaluate_condition("nonexistent is empty", &results));
        assert!(!evaluate_condition("nonexistent is not empty", &results));
    }

    #[test]
    fn test_length() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());

        assert!(evaluate_condition("users.length == 2", &results));
        assert!(evaluate_condition("users.length > 1", &results));
        assert!(evaluate_condition("users.length >= 2", &results));
        assert!(!evaluate_condition("users.length < 2", &results));
        assert!(evaluate_condition("empty_step.length == 0", &results));
        assert!(evaluate_condition("nonexistent.length == 0", &results));
    }

    #[test]
    fn test_first_syntax() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());

        // step.first.field syntax (aligns with {{step.first.field}})
        assert!(evaluate_condition("users.first.name == Alice", &results));
        assert!(evaluate_condition("users.first.score == 85", &results));
        assert!(evaluate_condition("users.first.active == true", &results));
        assert!(!evaluate_condition("users.first.name == Bob", &results));
    }

    #[test]
    fn test_field_implicit_first() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());

        // step.field implicitly uses first row
        assert!(evaluate_condition("users.name == Alice", &results));
        assert!(evaluate_condition("users.score > 80", &results));
    }

    #[test]
    fn test_indexed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());

        assert!(evaluate_condition("users[0].name == Alice", &results));
        assert!(evaluate_condition("users[1].name == Bob", &results));
        assert!(evaluate_condition("users[1].score == 92", &results));
        assert!(!evaluate_condition("users[2].name == Charlie", &results)); // Out of bounds
    }

    #[test]
    fn test_any_predicate() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());

        assert!(evaluate_condition("users.any(score > 90)", &results));
        assert!(evaluate_condition("users.any(active == true)", &results));
        assert!(!evaluate_condition("users.any(score > 100)", &results));
    }

    #[test]
    fn test_all_predicate() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());

        assert!(evaluate_condition("users.all(score > 80)", &results));
        assert!(!evaluate_condition("users.all(active == true)", &results));
        assert!(!evaluate_condition("empty_step.all(score > 0)", &results)); // Empty returns false
    }

    #[test]
    fn test_boolean_operators() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());

        // AND
        assert!(evaluate_condition(
            "users.length > 0 and users.first.name == Alice",
            &results
        ));
        assert!(!evaluate_condition(
            "users.length > 0 and users.first.name == Bob",
            &results
        ));

        // OR
        assert!(evaluate_condition(
            "users.first.name == Bob or users.first.name == Alice",
            &results
        ));
        assert!(!evaluate_condition(
            "users.first.name == Charlie or empty_step is not empty",
            &results
        ));

        // NOT
        assert!(evaluate_condition("not users is empty", &results));
        assert!(evaluate_condition("not users.first.name == Bob", &results));
    }

    #[test]
    fn test_combined_conditions() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());

        assert!(evaluate_condition(
            "users is not empty and users.any(score > 90)",
            &results
        ));

        assert!(evaluate_condition(
            "users.length == 2 and users.first.active == true",
            &results
        ));
    }

    #[test]
    fn test_string_with_quotes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());

        assert!(evaluate_condition("users.first.name == \"Alice\"", &results));
        assert!(evaluate_condition("users.first.name == 'Alice'", &results));
    }

    #[test]
    fn test_numeric_comparisons() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());

        assert!(evaluate_condition("users.first.score == 85", &results));
        assert!(evaluate_condition("users.first.score != 90", &results));
        assert!(evaluate_condition("users.first.score > 80", &results));
        assert!(evaluate_condition("users.first.score < 90", &results));
        assert!(evaluate_condition("users.first.score >= 85", &results));
        assert!(evaluate_condition("users.first.score <= 85", &results));
    }

    #[test]
    fn test_unknown_condition() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = create_test_results(temp_dir.path());

        // Unknown syntax returns false (with warning logged)
        assert!(!evaluate_condition("invalid syntax here", &results));
    }
}
