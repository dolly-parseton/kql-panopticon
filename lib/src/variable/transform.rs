//! Transform types for pipe-based variable substitution.
//!
//! Transforms modify data flowing through a pipeline. They are chained
//! using the `|` operator: `{{step.column | first | eq('expected')}}`.
//!
//! # Transform Categories
//!
//! - **Step-level**: Operate on entire step results (`is_empty`, `length`, `any`, `all`, `filter`)
//! - **Column-level**: Operate on column values (`first`, `at`, `array`, `unique`, `str_join`, `for_each`)
//! - **Comparison**: Convert to boolean (`eq`, `neq`, `gt`, `gte`, `lt`, `lte`)
//! - **Formatting**: Control output format (`quote`)

use crate::error::{Error, Result};
use crate::pack::QuoteStyle;
use crate::variable::predicate::{ComparisonOp, Predicate, PredicateValue};
use regex::Regex;
use std::sync::LazyLock;

// Pre-compiled regexes for transform parsing
static RE_AT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^at\s*\(\s*(\d+)\s*\)$").expect("Invalid AT regex"));

static RE_STR_JOIN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^str_join\s*\(\s*['"]([^'"]*)['"]\s*\)$"#).expect("Invalid STR_JOIN regex")
});

static RE_QUOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^quote\s*\(\s*(\w+)\s*\)$").expect("Invalid QUOTE regex"));

static RE_ANY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^any\s*\(\s*(.+)\s*\)$").expect("Invalid ANY regex"));

static RE_ALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^all\s*\(\s*(.+)\s*\)$").expect("Invalid ALL regex"));

static RE_FILTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^filter\s*\(\s*(.+)\s*\)$").expect("Invalid FILTER regex"));

// Comparison transform regexes
static RE_EQ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^eq\s*\(\s*(.+)\s*\)$").expect("Invalid EQ regex"));

static RE_NEQ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^neq\s*\(\s*(.+)\s*\)$").expect("Invalid NEQ regex"));

static RE_GT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^gt\s*\(\s*(.+)\s*\)$").expect("Invalid GT regex"));

static RE_GTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^gte\s*\(\s*(.+)\s*\)$").expect("Invalid GTE regex"));

static RE_LT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^lt\s*\(\s*(.+)\s*\)$").expect("Invalid LT regex"));

static RE_LTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^lte\s*\(\s*(.+)\s*\)$").expect("Invalid LTE regex"));

/// Types flowing through a transform pipeline.
///
/// Each transform has an expected input type and produces an output type.
/// The type system ensures transforms are chained correctly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformType {
    /// Full step result (from `{{step}}`)
    Step,
    /// Column selection/series (from `{{step.column}}` or after `filter`)
    Column,
    /// Single value (from `first`, `at`, `str_join`)
    Scalar,
    /// Array of values (from `array`)
    Array,
    /// Boolean (from `is_empty`, comparisons, `any`, `all`)
    Boolean,
    /// Iterator values (from `for_each`)
    Iterator,
    /// Filtered step (from `filter`, can chain to `.column`)
    FilteredStep,
}

impl TransformType {
    /// Check if this type can be used in a KQL query context.
    pub fn valid_for_kql(&self) -> bool {
        matches!(self, TransformType::Scalar | TransformType::Array)
    }

    /// Check if this type can be used in an HTTP request context.
    pub fn valid_for_http(&self) -> bool {
        matches!(
            self,
            TransformType::Scalar | TransformType::Array | TransformType::Iterator
        )
    }

    /// Check if this type can be used in a condition context.
    pub fn valid_for_condition(&self) -> bool {
        matches!(self, TransformType::Boolean)
    }
}

/// All transform types.
///
/// Transforms are parsed from the pipe-separated parts of a variable reference.
#[derive(Debug, Clone, PartialEq)]
pub enum Transform {
    // ========== Step-level transforms ==========
    /// Check if step has no rows: `| is_empty`
    IsEmpty,

    /// Check if step has rows: `| is_not_empty`
    IsNotEmpty,

    /// Get row count: `| length`
    Length,

    /// Check if any row matches predicate: `| any(field > value)`
    Any(Predicate),

    /// Check if all rows match predicate: `| all(field == value)`
    All(Predicate),

    /// Filter rows by predicate: `| filter(field > value)`
    ///
    /// Returns a filtered step that can be chained with `.column`.
    Filter(Predicate),

    // ========== Column transforms ==========
    /// Get first row's value: `| first`
    First,

    /// Get Nth row's value (0-indexed): `| at(N)`
    At(usize),

    /// Get all values as array: `| array`
    Array,

    /// Deduplicate values: `| unique`
    Unique,

    /// Join values with separator: `| str_join(',')`
    StrJoin(String),

    /// Trigger iteration (HTTP only): `| for_each`
    ForEach,

    // ========== Comparison transforms ==========
    /// Equals: `| eq(value)`
    Eq(PredicateValue),

    /// Not equals: `| neq(value)`
    Neq(PredicateValue),

    /// Greater than: `| gt(value)`
    Gt(PredicateValue),

    /// Greater than or equal: `| gte(value)`
    Gte(PredicateValue),

    /// Less than: `| lt(value)`
    Lt(PredicateValue),

    /// Less than or equal: `| lte(value)`
    Lte(PredicateValue),

    // ========== Formatting transforms ==========
    /// Apply quote style: `| quote(single)`, `| quote(double)`, `| quote(verbatim)`
    Quote(QuoteStyle),
}

impl Transform {
    /// Parse a transform from a string.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// Transform::parse("first")           // First
    /// Transform::parse("at(5)")           // At(5)
    /// Transform::parse("any(score > 90)") // Any(Predicate { ... })
    /// Transform::parse("eq('test')")      // Eq(PredicateValue::String("test"))
    /// ```
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();

        // Simple transforms (no arguments)
        match s {
            "is_empty" => return Ok(Transform::IsEmpty),
            "is_not_empty" => return Ok(Transform::IsNotEmpty),
            "length" => return Ok(Transform::Length),
            "first" => return Ok(Transform::First),
            "array" => return Ok(Transform::Array),
            "unique" => return Ok(Transform::Unique),
            "for_each" => return Ok(Transform::ForEach),
            _ => {}
        }

        // at(N)
        if let Some(captures) = RE_AT.captures(s) {
            let index: usize = captures
                .get(1)
                .expect("regex group 1")
                .as_str()
                .parse()
                .map_err(|e| Error::variable(format!("Invalid index in at(): {}", e)))?;
            return Ok(Transform::At(index));
        }

        // str_join(sep)
        if let Some(captures) = RE_STR_JOIN.captures(s) {
            let sep = captures.get(1).expect("regex group 1").as_str();
            return Ok(Transform::StrJoin(sep.to_string()));
        }

        // quote(style)
        if let Some(captures) = RE_QUOTE.captures(s) {
            let style_str = captures.get(1).expect("regex group 1").as_str();
            let style = match style_str.to_lowercase().as_str() {
                "single" => QuoteStyle::Single,
                "double" => QuoteStyle::Double,
                "verbatim" => QuoteStyle::Verbatim,
                _ => {
                    return Err(Error::variable(format!(
                        "Unknown quote style '{}'. Expected single, double, or verbatim",
                        style_str
                    )))
                }
            };
            return Ok(Transform::Quote(style));
        }

        // any(predicate)
        if let Some(captures) = RE_ANY.captures(s) {
            let predicate_str = captures.get(1).expect("regex group 1").as_str();
            let predicate = Predicate::parse(predicate_str)?;
            return Ok(Transform::Any(predicate));
        }

        // all(predicate)
        if let Some(captures) = RE_ALL.captures(s) {
            let predicate_str = captures.get(1).expect("regex group 1").as_str();
            let predicate = Predicate::parse(predicate_str)?;
            return Ok(Transform::All(predicate));
        }

        // filter(predicate)
        if let Some(captures) = RE_FILTER.captures(s) {
            let predicate_str = captures.get(1).expect("regex group 1").as_str();
            let predicate = Predicate::parse(predicate_str)?;
            return Ok(Transform::Filter(predicate));
        }

        // Comparison transforms
        if let Some(captures) = RE_EQ.captures(s) {
            let value = PredicateValue::parse(captures.get(1).expect("regex group 1").as_str());
            return Ok(Transform::Eq(value));
        }

        if let Some(captures) = RE_NEQ.captures(s) {
            let value = PredicateValue::parse(captures.get(1).expect("regex group 1").as_str());
            return Ok(Transform::Neq(value));
        }

        if let Some(captures) = RE_GT.captures(s) {
            let value = PredicateValue::parse(captures.get(1).expect("regex group 1").as_str());
            return Ok(Transform::Gt(value));
        }

        if let Some(captures) = RE_GTE.captures(s) {
            let value = PredicateValue::parse(captures.get(1).expect("regex group 1").as_str());
            return Ok(Transform::Gte(value));
        }

        if let Some(captures) = RE_LT.captures(s) {
            let value = PredicateValue::parse(captures.get(1).expect("regex group 1").as_str());
            return Ok(Transform::Lt(value));
        }

        if let Some(captures) = RE_LTE.captures(s) {
            let value = PredicateValue::parse(captures.get(1).expect("regex group 1").as_str());
            return Ok(Transform::Lte(value));
        }

        Err(Error::variable(format!("Unknown transform: '{}'", s)))
    }

    /// Get the expected input type for this transform.
    pub fn input_type(&self) -> TransformType {
        match self {
            // Step-level transforms expect Step or FilteredStep
            Transform::IsEmpty
            | Transform::IsNotEmpty
            | Transform::Length
            | Transform::Any(_)
            | Transform::All(_)
            | Transform::Filter(_) => TransformType::Step,

            // Column transforms expect Column
            Transform::First
            | Transform::At(_)
            | Transform::Array
            | Transform::Unique
            | Transform::StrJoin(_)
            | Transform::ForEach => TransformType::Column,

            // Comparison transforms expect Scalar or Length output
            Transform::Eq(_)
            | Transform::Neq(_)
            | Transform::Gt(_)
            | Transform::Gte(_)
            | Transform::Lt(_)
            | Transform::Lte(_) => TransformType::Scalar,

            // Quote expects Array
            Transform::Quote(_) => TransformType::Array,
        }
    }

    /// Get the output type of this transform.
    pub fn output_type(&self) -> TransformType {
        match self {
            // Boolean outputs
            Transform::IsEmpty
            | Transform::IsNotEmpty
            | Transform::Any(_)
            | Transform::All(_)
            | Transform::Eq(_)
            | Transform::Neq(_)
            | Transform::Gt(_)
            | Transform::Gte(_)
            | Transform::Lt(_)
            | Transform::Lte(_) => TransformType::Boolean,

            // Scalar outputs
            Transform::Length => TransformType::Scalar, // Actually integer, treated as scalar
            Transform::First | Transform::At(_) | Transform::StrJoin(_) => TransformType::Scalar,

            // Array outputs
            Transform::Array | Transform::Unique | Transform::Quote(_) => TransformType::Array,

            // Iterator output
            Transform::ForEach => TransformType::Iterator,

            // Filtered step (can chain to column)
            Transform::Filter(_) => TransformType::FilteredStep,
        }
    }

    /// Check if this transform produces a boolean result.
    pub fn is_boolean(&self) -> bool {
        self.output_type() == TransformType::Boolean
    }

    /// Check if this transform is for_each.
    pub fn is_for_each(&self) -> bool {
        matches!(self, Transform::ForEach)
    }

    /// Get the comparison operator if this is a comparison transform.
    pub fn comparison_op(&self) -> Option<ComparisonOp> {
        match self {
            Transform::Eq(_) => Some(ComparisonOp::Eq),
            Transform::Neq(_) => Some(ComparisonOp::Neq),
            Transform::Gt(_) => Some(ComparisonOp::Gt),
            Transform::Gte(_) => Some(ComparisonOp::Gte),
            Transform::Lt(_) => Some(ComparisonOp::Lt),
            Transform::Lte(_) => Some(ComparisonOp::Lte),
            _ => None,
        }
    }

    /// Get the comparison value if this is a comparison transform.
    pub fn comparison_value(&self) -> Option<&PredicateValue> {
        match self {
            Transform::Eq(v)
            | Transform::Neq(v)
            | Transform::Gt(v)
            | Transform::Gte(v)
            | Transform::Lt(v)
            | Transform::Lte(v) => Some(v),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_transforms() {
        assert_eq!(Transform::parse("is_empty").unwrap(), Transform::IsEmpty);
        assert_eq!(
            Transform::parse("is_not_empty").unwrap(),
            Transform::IsNotEmpty
        );
        assert_eq!(Transform::parse("length").unwrap(), Transform::Length);
        assert_eq!(Transform::parse("first").unwrap(), Transform::First);
        assert_eq!(Transform::parse("array").unwrap(), Transform::Array);
        assert_eq!(Transform::parse("unique").unwrap(), Transform::Unique);
        assert_eq!(Transform::parse("for_each").unwrap(), Transform::ForEach);
    }

    #[test]
    fn test_parse_at() {
        assert_eq!(Transform::parse("at(0)").unwrap(), Transform::At(0));
        assert_eq!(Transform::parse("at(5)").unwrap(), Transform::At(5));
        assert_eq!(Transform::parse("at( 10 )").unwrap(), Transform::At(10));
    }

    #[test]
    fn test_parse_str_join() {
        assert_eq!(
            Transform::parse("str_join(',')").unwrap(),
            Transform::StrJoin(",".to_string())
        );
        assert_eq!(
            Transform::parse("str_join(\", \")").unwrap(),
            Transform::StrJoin(", ".to_string())
        );
        assert_eq!(
            Transform::parse("str_join('')").unwrap(),
            Transform::StrJoin("".to_string())
        );
    }

    #[test]
    fn test_parse_quote() {
        assert_eq!(
            Transform::parse("quote(single)").unwrap(),
            Transform::Quote(QuoteStyle::Single)
        );
        assert_eq!(
            Transform::parse("quote(double)").unwrap(),
            Transform::Quote(QuoteStyle::Double)
        );
        assert_eq!(
            Transform::parse("quote(verbatim)").unwrap(),
            Transform::Quote(QuoteStyle::Verbatim)
        );
        assert_eq!(
            Transform::parse("quote(SINGLE)").unwrap(),
            Transform::Quote(QuoteStyle::Single)
        );
    }

    #[test]
    fn test_parse_any_all_filter() {
        let t = Transform::parse("any(score > 90)").unwrap();
        assert!(matches!(t, Transform::Any(_)));

        let t = Transform::parse("all(active == true)").unwrap();
        assert!(matches!(t, Transform::All(_)));

        let t = Transform::parse("filter(count >= 5)").unwrap();
        assert!(matches!(t, Transform::Filter(_)));
    }

    #[test]
    fn test_parse_comparison_transforms() {
        let t = Transform::parse("eq(42)").unwrap();
        assert!(matches!(t, Transform::Eq(PredicateValue::Number(_))));

        let t = Transform::parse("neq('test')").unwrap();
        assert!(matches!(t, Transform::Neq(PredicateValue::String(_))));

        let t = Transform::parse("gt(100)").unwrap();
        assert!(matches!(t, Transform::Gt(_)));

        let t = Transform::parse("gte(0)").unwrap();
        assert!(matches!(t, Transform::Gte(_)));

        let t = Transform::parse("lt(50)").unwrap();
        assert!(matches!(t, Transform::Lt(_)));

        let t = Transform::parse("lte(10)").unwrap();
        assert!(matches!(t, Transform::Lte(_)));
    }

    #[test]
    fn test_parse_with_whitespace() {
        assert_eq!(
            Transform::parse("  first  ").unwrap(),
            Transform::First
        );
        assert_eq!(
            Transform::parse("at( 5 )").unwrap(),
            Transform::At(5)
        );
    }

    #[test]
    fn test_parse_unknown() {
        assert!(Transform::parse("unknown").is_err());
        assert!(Transform::parse("foo(bar)").is_err());
    }

    #[test]
    fn test_input_output_types() {
        // Step-level transforms
        assert_eq!(Transform::IsEmpty.input_type(), TransformType::Step);
        assert_eq!(Transform::IsEmpty.output_type(), TransformType::Boolean);

        assert_eq!(Transform::Length.input_type(), TransformType::Step);
        assert_eq!(Transform::Length.output_type(), TransformType::Scalar);

        // Column transforms
        assert_eq!(Transform::First.input_type(), TransformType::Column);
        assert_eq!(Transform::First.output_type(), TransformType::Scalar);

        assert_eq!(Transform::Array.input_type(), TransformType::Column);
        assert_eq!(Transform::Array.output_type(), TransformType::Array);

        assert_eq!(Transform::ForEach.input_type(), TransformType::Column);
        assert_eq!(Transform::ForEach.output_type(), TransformType::Iterator);

        // Comparison transforms
        let eq = Transform::Eq(PredicateValue::Number(5.0));
        assert_eq!(eq.input_type(), TransformType::Scalar);
        assert_eq!(eq.output_type(), TransformType::Boolean);
    }

    #[test]
    fn test_transform_type_validity() {
        assert!(TransformType::Scalar.valid_for_kql());
        assert!(TransformType::Array.valid_for_kql());
        assert!(!TransformType::Boolean.valid_for_kql());
        assert!(!TransformType::Iterator.valid_for_kql());

        assert!(TransformType::Scalar.valid_for_http());
        assert!(TransformType::Array.valid_for_http());
        assert!(TransformType::Iterator.valid_for_http());
        assert!(!TransformType::Boolean.valid_for_http());

        assert!(TransformType::Boolean.valid_for_condition());
        assert!(!TransformType::Scalar.valid_for_condition());
    }

    #[test]
    fn test_is_for_each() {
        assert!(Transform::ForEach.is_for_each());
        assert!(!Transform::First.is_for_each());
        assert!(!Transform::Array.is_for_each());
    }

    #[test]
    fn test_is_boolean() {
        assert!(Transform::IsEmpty.is_boolean());
        assert!(Transform::IsNotEmpty.is_boolean());
        assert!(Transform::Eq(PredicateValue::Number(1.0)).is_boolean());
        assert!(!Transform::First.is_boolean());
        assert!(!Transform::Length.is_boolean());
    }
}
