//! Predicate parsing for row-level operations.
//!
//! Predicates are used inside `any()`, `all()`, and `filter()` transforms
//! to evaluate conditions against each row of step results.
//!
//! # Syntax
//!
//! ```text
//! field == value     # Equality
//! field != value     # Inequality
//! field > value      # Greater than
//! field >= value     # Greater than or equal
//! field < value      # Less than
//! field <= value     # Less than or equal
//! ```
//!
//! # Value Types
//!
//! - Boolean: `true`, `false`
//! - Number: `123`, `45.67`, `-10`
//! - String: `'quoted'` or `unquoted`

use crate::error::{Error, Result};
use regex::Regex;
use serde_json::Value as JsonValue;
use std::sync::LazyLock;

/// Pre-compiled regex for predicate parsing.
///
/// Matches: `field op value` where op is one of ==, !=, >=, <=, >, <
/// Order matters: >= before >, <= before <
static PREDICATE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(\w+)\s*(==|!=|>=|<=|>|<)\s*(.+?)\s*$").expect("Invalid predicate regex")
});

/// Comparison operators for predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    /// Equal: `==`
    Eq,
    /// Not equal: `!=`
    Neq,
    /// Greater than: `>`
    Gt,
    /// Greater than or equal: `>=`
    Gte,
    /// Less than: `<`
    Lt,
    /// Less than or equal: `<=`
    Lte,
}

impl ComparisonOp {
    /// Parse a comparison operator from a string.
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "==" => Ok(ComparisonOp::Eq),
            "!=" => Ok(ComparisonOp::Neq),
            ">" => Ok(ComparisonOp::Gt),
            ">=" => Ok(ComparisonOp::Gte),
            "<" => Ok(ComparisonOp::Lt),
            "<=" => Ok(ComparisonOp::Lte),
            _ => Err(Error::variable(format!(
                "Unknown comparison operator: '{}'",
                s
            ))),
        }
    }

    /// Apply the comparison to two numbers.
    pub fn compare_numbers(&self, left: f64, right: f64) -> bool {
        match self {
            ComparisonOp::Eq => (left - right).abs() < f64::EPSILON,
            ComparisonOp::Neq => (left - right).abs() >= f64::EPSILON,
            ComparisonOp::Gt => left > right,
            ComparisonOp::Gte => left >= right,
            ComparisonOp::Lt => left < right,
            ComparisonOp::Lte => left <= right,
        }
    }

    /// Apply the comparison to two strings.
    pub fn compare_strings(&self, left: &str, right: &str) -> bool {
        match self {
            ComparisonOp::Eq => left == right,
            ComparisonOp::Neq => left != right,
            // String comparisons for ordering use lexicographic comparison
            ComparisonOp::Gt => left > right,
            ComparisonOp::Gte => left >= right,
            ComparisonOp::Lt => left < right,
            ComparisonOp::Lte => left <= right,
        }
    }

    /// Apply the comparison to two booleans.
    pub fn compare_bools(&self, left: bool, right: bool) -> bool {
        match self {
            ComparisonOp::Eq => left == right,
            ComparisonOp::Neq => left != right,
            // Boolean comparisons for ordering don't make sense
            _ => false,
        }
    }
}

/// A value in a predicate comparison.
#[derive(Debug, Clone, PartialEq)]
pub enum PredicateValue {
    /// Boolean value: `true` or `false`
    Bool(bool),
    /// Numeric value: integers or floats
    Number(f64),
    /// String value: quoted or unquoted
    String(String),
}

impl PredicateValue {
    /// Parse a value from a string.
    ///
    /// Attempts to parse as boolean, then number, then string.
    pub fn parse(s: &str) -> Self {
        let s = s.trim();

        // Check for boolean
        if s.eq_ignore_ascii_case("true") {
            return PredicateValue::Bool(true);
        }
        if s.eq_ignore_ascii_case("false") {
            return PredicateValue::Bool(false);
        }

        // Check for number
        if let Ok(n) = s.parse::<f64>() {
            return PredicateValue::Number(n);
        }

        // Otherwise treat as string, removing quotes if present
        let unquoted = s
            .trim_start_matches('\'')
            .trim_end_matches('\'')
            .trim_start_matches('"')
            .trim_end_matches('"');

        PredicateValue::String(unquoted.to_string())
    }

    /// Compare this value against a JSON value using the given operator.
    pub fn compare(&self, op: ComparisonOp, json_value: &JsonValue) -> bool {
        match (self, json_value) {
            // Boolean comparison
            (PredicateValue::Bool(expected), JsonValue::Bool(actual)) => {
                op.compare_bools(*actual, *expected)
            }

            // Number comparison - handle both i64 and f64 JSON numbers
            (PredicateValue::Number(expected), JsonValue::Number(n)) => {
                if let Some(actual) = n.as_f64() {
                    op.compare_numbers(actual, *expected)
                } else {
                    false
                }
            }

            // String comparison
            (PredicateValue::String(expected), JsonValue::String(actual)) => {
                op.compare_strings(actual, expected)
            }

            // Try to coerce string JSON value to number for comparison
            (PredicateValue::Number(expected), JsonValue::String(s)) => {
                if let Ok(actual) = s.parse::<f64>() {
                    op.compare_numbers(actual, *expected)
                } else {
                    false
                }
            }

            // Try to coerce number JSON value to string for comparison
            (PredicateValue::String(expected), JsonValue::Number(n)) => {
                let actual = n.to_string();
                op.compare_strings(&actual, expected)
            }

            // Null handling - only equality/inequality make sense
            (PredicateValue::String(s), JsonValue::Null) if s.is_empty() || s == "null" => {
                matches!(op, ComparisonOp::Eq)
            }
            (_, JsonValue::Null) => matches!(op, ComparisonOp::Neq),

            // Type mismatch
            _ => false,
        }
    }
}

/// A predicate expression for row-level filtering.
///
/// Used inside `any()`, `all()`, and `filter()` transforms.
#[derive(Debug, Clone, PartialEq)]
pub struct Predicate {
    /// Field name to compare
    pub field: String,
    /// Comparison operator
    pub op: ComparisonOp,
    /// Value to compare against
    pub value: PredicateValue,
}

impl Predicate {
    /// Create a new predicate.
    pub fn new(field: impl Into<String>, op: ComparisonOp, value: PredicateValue) -> Self {
        Self {
            field: field.into(),
            op,
            value,
        }
    }

    /// Parse a predicate from a string.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// Predicate::parse("score > 90")
    /// Predicate::parse("active == true")
    /// Predicate::parse("name == 'alice'")
    /// ```
    pub fn parse(s: &str) -> Result<Self> {
        let captures = PREDICATE_REGEX.captures(s).ok_or_else(|| {
            Error::variable(format!(
                "Invalid predicate syntax: '{}'. Expected 'field op value'",
                s
            ))
        })?;

        let field = captures
            .get(1)
            .expect("regex group 1")
            .as_str()
            .to_string();
        let op_str = captures.get(2).expect("regex group 2").as_str();
        let value_str = captures.get(3).expect("regex group 3").as_str();

        // Reject values that start with = (e.g., from === being parsed as == followed by =)
        if value_str.starts_with('=') {
            return Err(Error::variable(format!(
                "Invalid predicate syntax: '{}'. Unrecognized operator",
                s
            )));
        }

        let op = ComparisonOp::parse(op_str)?;
        let value = PredicateValue::parse(value_str);

        Ok(Predicate { field, op, value })
    }

    /// Evaluate the predicate against a JSON row.
    ///
    /// Returns `true` if the row matches the predicate, `false` otherwise.
    pub fn evaluate(&self, row: &JsonValue) -> bool {
        let field_value = match row.get(&self.field) {
            Some(v) => v,
            None => return false, // Missing field = no match
        };

        self.value.compare(self.op, field_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_comparison_op() {
        assert_eq!(ComparisonOp::parse("==").unwrap(), ComparisonOp::Eq);
        assert_eq!(ComparisonOp::parse("!=").unwrap(), ComparisonOp::Neq);
        assert_eq!(ComparisonOp::parse(">").unwrap(), ComparisonOp::Gt);
        assert_eq!(ComparisonOp::parse(">=").unwrap(), ComparisonOp::Gte);
        assert_eq!(ComparisonOp::parse("<").unwrap(), ComparisonOp::Lt);
        assert_eq!(ComparisonOp::parse("<=").unwrap(), ComparisonOp::Lte);
        assert!(ComparisonOp::parse("===").is_err());
    }

    #[test]
    fn test_parse_predicate_value() {
        assert_eq!(PredicateValue::parse("true"), PredicateValue::Bool(true));
        assert_eq!(PredicateValue::parse("false"), PredicateValue::Bool(false));
        assert_eq!(PredicateValue::parse("TRUE"), PredicateValue::Bool(true));

        assert_eq!(PredicateValue::parse("42"), PredicateValue::Number(42.0));
        assert_eq!(PredicateValue::parse("-10"), PredicateValue::Number(-10.0));
        assert_eq!(
            PredicateValue::parse("3.14"),
            PredicateValue::Number(3.14)
        );

        assert_eq!(
            PredicateValue::parse("hello"),
            PredicateValue::String("hello".to_string())
        );
        assert_eq!(
            PredicateValue::parse("'quoted'"),
            PredicateValue::String("quoted".to_string())
        );
        assert_eq!(
            PredicateValue::parse("\"double\""),
            PredicateValue::String("double".to_string())
        );
    }

    #[test]
    fn test_parse_predicate() {
        let p = Predicate::parse("score > 90").unwrap();
        assert_eq!(p.field, "score");
        assert_eq!(p.op, ComparisonOp::Gt);
        assert_eq!(p.value, PredicateValue::Number(90.0));

        let p = Predicate::parse("active == true").unwrap();
        assert_eq!(p.field, "active");
        assert_eq!(p.op, ComparisonOp::Eq);
        assert_eq!(p.value, PredicateValue::Bool(true));

        let p = Predicate::parse("name == 'alice'").unwrap();
        assert_eq!(p.field, "name");
        assert_eq!(p.op, ComparisonOp::Eq);
        assert_eq!(p.value, PredicateValue::String("alice".to_string()));
    }

    #[test]
    fn test_parse_predicate_with_spaces() {
        let p = Predicate::parse("  score   >   90  ").unwrap();
        assert_eq!(p.field, "score");
        assert_eq!(p.op, ComparisonOp::Gt);

        let p = Predicate::parse("field>=100").unwrap();
        assert_eq!(p.field, "field");
        assert_eq!(p.op, ComparisonOp::Gte);
    }

    #[test]
    fn test_parse_predicate_error() {
        assert!(Predicate::parse("").is_err());
        assert!(Predicate::parse("score").is_err());
        assert!(Predicate::parse("> 90").is_err());
        assert!(Predicate::parse("score === 90").is_err());
    }

    #[test]
    fn test_evaluate_number_comparison() {
        let row = json!({"score": 85, "count": 10});

        let p = Predicate::parse("score > 80").unwrap();
        assert!(p.evaluate(&row));

        let p = Predicate::parse("score > 90").unwrap();
        assert!(!p.evaluate(&row));

        let p = Predicate::parse("score >= 85").unwrap();
        assert!(p.evaluate(&row));

        let p = Predicate::parse("score == 85").unwrap();
        assert!(p.evaluate(&row));

        let p = Predicate::parse("score != 90").unwrap();
        assert!(p.evaluate(&row));
    }

    #[test]
    fn test_evaluate_string_comparison() {
        let row = json!({"name": "alice", "status": "active"});

        let p = Predicate::parse("name == alice").unwrap();
        assert!(p.evaluate(&row));

        let p = Predicate::parse("name == 'alice'").unwrap();
        assert!(p.evaluate(&row));

        let p = Predicate::parse("name != bob").unwrap();
        assert!(p.evaluate(&row));

        let p = Predicate::parse("status == active").unwrap();
        assert!(p.evaluate(&row));
    }

    #[test]
    fn test_evaluate_bool_comparison() {
        let row = json!({"active": true, "deleted": false});

        let p = Predicate::parse("active == true").unwrap();
        assert!(p.evaluate(&row));

        let p = Predicate::parse("active != false").unwrap();
        assert!(p.evaluate(&row));

        let p = Predicate::parse("deleted == false").unwrap();
        assert!(p.evaluate(&row));
    }

    #[test]
    fn test_evaluate_missing_field() {
        let row = json!({"name": "alice"});

        let p = Predicate::parse("score > 90").unwrap();
        assert!(!p.evaluate(&row)); // Missing field = no match
    }

    #[test]
    fn test_evaluate_type_coercion() {
        // String to number coercion
        let row = json!({"score": "85"});
        let p = Predicate::parse("score > 80").unwrap();
        assert!(p.evaluate(&row));

        // Number to string coercion
        let row = json!({"code": 200});
        let p = Predicate::parse("code == 200").unwrap();
        assert!(p.evaluate(&row));
    }

    #[test]
    fn test_evaluate_null() {
        let row = json!({"value": null});

        let p = Predicate::parse("value == null").unwrap();
        assert!(p.evaluate(&row));

        let p = Predicate::parse("value != null").unwrap();
        assert!(!p.evaluate(&row));
    }

    #[test]
    fn test_number_comparisons() {
        let op = ComparisonOp::Eq;
        assert!(op.compare_numbers(1.0, 1.0));
        assert!(!op.compare_numbers(1.0, 2.0));

        let op = ComparisonOp::Gt;
        assert!(op.compare_numbers(2.0, 1.0));
        assert!(!op.compare_numbers(1.0, 2.0));

        let op = ComparisonOp::Lte;
        assert!(op.compare_numbers(1.0, 1.0));
        assert!(op.compare_numbers(1.0, 2.0));
        assert!(!op.compare_numbers(2.0, 1.0));
    }
}
