//! Transform evaluation for pipeline execution.
//!
//! Evaluates pipelines against data to produce substitution results.
//!
//! # Example
//!
//! ```ignore
//! let pipeline = Pipeline::parse("step.column | first")?;
//! let ctx = EvaluationContext::new()
//!     .with_step_results(results);
//! let result = evaluate(&pipeline, &ctx)?;
//! ```

use crate::error::{Error, Result};
use crate::execution::result::{ResultContext, ResultHandle};
use crate::pack::{InputType, QuoteStyle};
use crate::variable::pipeline::Pipeline;
use crate::variable::predicate::{ComparisonOp, Predicate, PredicateValue};
use crate::variable::source::Source;
use crate::variable::transform::Transform;
use polars::prelude::*;
use serde_json::Value as JsonValue;
use std::borrow::Cow;
use std::collections::HashMap;

/// Result of evaluating a transform pipeline.
#[derive(Debug, Clone)]
pub enum TransformResult {
    /// Single string value.
    Scalar(String),

    /// Array of string values.
    Array(Vec<String>),

    /// Boolean value (for conditions).
    Boolean(bool),

    /// Iterator values for `for_each` expansion.
    ///
    /// Contains the values that will be iterated over.
    Iterator(Vec<String>),
}

impl TransformResult {
    /// Format the result as a string for substitution.
    ///
    /// - Scalar: returns the value as-is
    /// - Array: formats as quoted, comma-separated list
    /// - Boolean: returns "true" or "false"
    /// - Iterator: not directly formattable (use values directly)
    pub fn format(&self, quote_style: QuoteStyle) -> String {
        match self {
            TransformResult::Scalar(s) => s.clone(),
            TransformResult::Array(values) => quote_style.format_array(values),
            TransformResult::Boolean(b) => b.to_string(),
            TransformResult::Iterator(_) => {
                // Should not be directly formatted - use substitute_iter instead
                String::new()
            }
        }
    }

    /// Get as boolean if this is a Boolean result.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            TransformResult::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Get as scalar if this is a Scalar result.
    pub fn as_scalar(&self) -> Option<&str> {
        match self {
            TransformResult::Scalar(s) => Some(s),
            _ => None,
        }
    }

    /// Get as array if this is an Array result.
    pub fn as_array(&self) -> Option<&[String]> {
        match self {
            TransformResult::Array(v) => Some(v),
            _ => None,
        }
    }

    /// Get iterator values if this is an Iterator result.
    pub fn as_iterator(&self) -> Option<&[String]> {
        match self {
            TransformResult::Iterator(v) => Some(v),
            _ => None,
        }
    }

    /// Check if this result represents a for_each iteration.
    pub fn is_iterator(&self) -> bool {
        matches!(self, TransformResult::Iterator(_))
    }
}

/// Context for evaluating pipelines.
///
/// Contains all data sources: inputs, secrets, and step results.
///
/// Uses `Cow<'a, ResultContext>` to support both owned and borrowed step results:
/// - Owned: For incremental building during acquisition phase
/// - Borrowed: For zero-copy evaluation during processing/reporting phases
#[derive(Debug, Clone)]
pub struct EvaluationContext<'a> {
    /// User-provided input values.
    inputs: HashMap<String, String>,

    /// Input type definitions (string vs array).
    input_types: HashMap<String, InputType>,

    /// Environment secret values.
    secrets: HashMap<String, String>,

    /// Step results from execution.
    /// Uses Cow to support both owned and borrowed patterns.
    step_results: Cow<'a, ResultContext>,

    /// Default quote style for output.
    default_quote_style: QuoteStyle,
}

impl Default for EvaluationContext<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> EvaluationContext<'a> {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self {
            inputs: HashMap::new(),
            input_types: HashMap::new(),
            secrets: HashMap::new(),
            step_results: Cow::Owned(ResultContext::new()),
            default_quote_style: QuoteStyle::Single,
        }
    }

    /// Add an input value.
    pub fn with_input(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.inputs.insert(name.into(), value.into());
        self
    }

    /// Add input values from a map.
    pub fn with_inputs(mut self, inputs: HashMap<String, String>) -> Self {
        self.inputs.extend(inputs);
        self
    }

    /// Add input type definitions.
    pub fn with_input_types(mut self, types: HashMap<String, InputType>) -> Self {
        self.input_types.extend(types);
        self
    }

    /// Add a secret value.
    pub fn with_secret(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.secrets.insert(name.into(), value.into());
        self
    }

    /// Add secret values from a map.
    pub fn with_secrets(mut self, secrets: HashMap<String, String>) -> Self {
        self.secrets.extend(secrets);
        self
    }

    /// Set step results (owned).
    ///
    /// Use this when building context incrementally during acquisition.
    pub fn with_step_results(mut self, results: ResultContext) -> Self {
        self.step_results = Cow::Owned(results);
        self
    }

    /// Set step results (borrowed).
    ///
    /// Use this for zero-copy evaluation during processing/reporting phases.
    pub fn with_step_results_ref(mut self, results: &'a ResultContext) -> Self {
        self.step_results = Cow::Borrowed(results);
        self
    }

    /// Set default quote style.
    pub fn with_quote_style(mut self, style: QuoteStyle) -> Self {
        self.default_quote_style = style;
        self
    }

    /// Get an input value.
    pub fn get_input(&self, name: &str) -> Option<&str> {
        self.inputs.get(name).map(|s| s.as_str())
    }

    /// Get input type (default to String if not specified).
    pub fn get_input_type(&self, name: &str) -> InputType {
        self.input_types
            .get(name)
            .copied()
            .unwrap_or(InputType::String)
    }

    /// Get a secret value.
    pub fn get_secret(&self, name: &str) -> Option<&str> {
        self.secrets.get(name).map(|s| s.as_str())
    }

    /// Get step results.
    pub fn step_results(&self) -> &ResultContext {
        self.step_results.as_ref()
    }

    /// Get default quote style.
    pub fn default_quote_style(&self) -> QuoteStyle {
        self.default_quote_style
    }

    // ========== Mutable setters for incremental context building ==========

    /// Set input values directly.
    pub fn set_inputs(&mut self, inputs: HashMap<String, String>) {
        self.inputs = inputs;
    }

    /// Set input type definitions directly.
    pub fn set_input_types(&mut self, types: HashMap<String, InputType>) {
        self.input_types = types;
    }

    /// Register a step result.
    ///
    /// This will clone the ResultContext if it was borrowed.
    pub fn register_result(&mut self, step_name: impl Into<String>, handle: ResultHandle) {
        self.step_results.to_mut().insert(step_name.into(), handle);
    }

    /// Get mutable access to step results.
    ///
    /// This will clone the ResultContext if it was borrowed.
    pub fn step_results_mut(&mut self) -> &mut ResultContext {
        self.step_results.to_mut()
    }
}

/// Internal intermediate value during pipeline evaluation.
///
/// Uses Polars LazyFrame for step data to enable deferred execution
/// and query optimization. Only collects results at terminal transforms.
enum IntermediateValue {
    /// LazyFrame representing step rows (deferred execution).
    /// Used for step-level transforms: is_empty, length, any, all, filter.
    LazyRows(LazyFrame),

    /// LazyFrame with single column selected (deferred execution).
    /// Used for column transforms: first, at, array, unique, str_join.
    LazyColumn { frame: LazyFrame, column: String },

    /// Collected column values (post-terminal transform).
    /// From `array`, `unique | array`, or `for_each`.
    ColumnValues(Vec<String>),

    /// Single scalar value (already materialized).
    /// From inputs/secrets or terminal transforms like `first`, `str_join`.
    Scalar(String),

    /// Single integer value (from length).
    Integer(usize),

    /// Boolean value (from is_empty, any, all, comparisons).
    Boolean(bool),
}

// LazyFrame doesn't implement Debug, so we implement it manually
impl std::fmt::Debug for IntermediateValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntermediateValue::LazyRows(_) => write!(f, "LazyRows(<LazyFrame>)"),
            IntermediateValue::LazyColumn { column, .. } => {
                write!(f, "LazyColumn {{ column: {:?} }}", column)
            }
            IntermediateValue::ColumnValues(v) => f.debug_tuple("ColumnValues").field(v).finish(),
            IntermediateValue::Scalar(s) => f.debug_tuple("Scalar").field(s).finish(),
            IntermediateValue::Integer(n) => f.debug_tuple("Integer").field(n).finish(),
            IntermediateValue::Boolean(b) => f.debug_tuple("Boolean").field(b).finish(),
        }
    }
}

/// Evaluate a pipeline against a context.
///
/// Returns the result of applying all transforms to the source data.
pub fn evaluate(pipeline: &Pipeline, ctx: &EvaluationContext<'_>) -> Result<TransformResult> {
    // Resolve source to initial value
    let mut current = resolve_source(pipeline, ctx)?;

    // Apply each transform
    for transform in &pipeline.transforms {
        current = apply_transform(current, transform, pipeline, ctx)?;
    }

    // Convert final intermediate value to result
    intermediate_to_result(current, ctx.default_quote_style)
}

/// Resolve the source of a pipeline to an intermediate value.
fn resolve_source(pipeline: &Pipeline, ctx: &EvaluationContext<'_>) -> Result<IntermediateValue> {
    match &pipeline.source {
        Source::Input { name } => {
            let value = ctx.get_input(name).ok_or_else(|| {
                Error::variable(format!("Input '{}' not found", name))
            })?;

            // Inputs always start as scalars.
            // Use `| array` transform to split array-type inputs.
            Ok(IntermediateValue::Scalar(value.to_string()))
        }

        Source::Secret { name } => {
            let value = ctx.get_secret(name).ok_or_else(|| {
                Error::variable(format!("Secret '{}' not found", name))
            })?;
            Ok(IntermediateValue::Scalar(value.to_string()))
        }

        Source::Step { name } => {
            // Return LazyFrame for deferred execution
            match ctx.step_results().lazy_frame(name)? {
                Some(lf) => Ok(IntermediateValue::LazyRows(lf)),
                None => {
                    // Step doesn't exist or empty - return empty LazyFrame
                    Ok(IntermediateValue::LazyRows(empty_lazy_frame()))
                }
            }
        }

        Source::StepColumn { step, column } => {
            // Return LazyFrame with column info for deferred selection
            match ctx.step_results().lazy_frame(step)? {
                Some(lf) => Ok(IntermediateValue::LazyColumn {
                    frame: lf,
                    column: column.clone(),
                }),
                None => {
                    // Missing step - return empty values
                    Ok(IntermediateValue::ColumnValues(vec![]))
                }
            }
        }
    }
}

/// Apply a single transform to an intermediate value.
fn apply_transform(
    input: IntermediateValue,
    transform: &Transform,
    pipeline: &Pipeline,
    _ctx: &EvaluationContext<'_>,
) -> Result<IntermediateValue> {
    match transform {
        // ========== Step-level transforms ==========
        Transform::IsEmpty => {
            let is_empty = match input {
                IntermediateValue::LazyRows(lf) => {
                    let df = lf.collect().map_err(polars_to_error)?;
                    df.height() == 0
                }
                IntermediateValue::LazyColumn { frame, column } => {
                    // Count non-null values in column
                    let df = frame
                        .select([col(&column)])
                        .collect()
                        .map_err(polars_to_error)?;
                    df.height() == 0
                }
                IntermediateValue::ColumnValues(vals) => vals.is_empty(),
                _ => return Err(Error::variable("is_empty requires step or column input")),
            };
            Ok(IntermediateValue::Boolean(is_empty))
        }

        Transform::IsNotEmpty => {
            let is_not_empty = match input {
                IntermediateValue::LazyRows(lf) => {
                    let df = lf.collect().map_err(polars_to_error)?;
                    df.height() > 0
                }
                IntermediateValue::LazyColumn { frame, column } => {
                    let df = frame
                        .select([col(&column)])
                        .collect()
                        .map_err(polars_to_error)?;
                    df.height() > 0
                }
                IntermediateValue::ColumnValues(vals) => !vals.is_empty(),
                _ => return Err(Error::variable("is_not_empty requires step or column input")),
            };
            Ok(IntermediateValue::Boolean(is_not_empty))
        }

        Transform::Length => {
            let len = match input {
                IntermediateValue::LazyRows(lf) => {
                    let df = lf.collect().map_err(polars_to_error)?;
                    df.height()
                }
                IntermediateValue::LazyColumn { frame, column } => {
                    let df = frame
                        .select([col(&column)])
                        .collect()
                        .map_err(polars_to_error)?;
                    df.height()
                }
                IntermediateValue::ColumnValues(vals) => vals.len(),
                _ => return Err(Error::variable("length requires step or column input")),
            };
            Ok(IntermediateValue::Integer(len))
        }

        Transform::Any(predicate) => {
            match input {
                IntermediateValue::LazyRows(lf) => {
                    let expr = predicate_to_polars_expr(predicate)?;
                    let df = lf.filter(expr).collect().map_err(polars_to_error)?;
                    Ok(IntermediateValue::Boolean(df.height() > 0))
                }
                _ => Err(Error::variable("any() requires step input")),
            }
        }

        Transform::All(predicate) => {
            match input {
                IntermediateValue::LazyRows(lf) => {
                    // First check if empty (all() on empty returns false)
                    let total_df = lf.clone().collect().map_err(polars_to_error)?;
                    if total_df.height() == 0 {
                        return Ok(IntermediateValue::Boolean(false));
                    }

                    // all() = total count == matching count
                    let expr = predicate_to_polars_expr(predicate)?;
                    let match_df = lf.filter(expr).collect().map_err(polars_to_error)?;
                    Ok(IntermediateValue::Boolean(match_df.height() == total_df.height()))
                }
                _ => Err(Error::variable("all() requires step input")),
            }
        }

        Transform::Filter(predicate) => {
            match input {
                IntermediateValue::LazyRows(lf) => {
                    let expr = predicate_to_polars_expr(predicate)?;
                    let filtered = lf.filter(expr);

                    // Check if we need to extract a column (filter().column syntax)
                    if let Some(column) = &pipeline.filter_column {
                        Ok(IntermediateValue::LazyColumn {
                            frame: filtered,
                            column: column.clone(),
                        })
                    } else {
                        Ok(IntermediateValue::LazyRows(filtered))
                    }
                }
                _ => Err(Error::variable("filter() requires step input")),
            }
        }

        // ========== Column transforms ==========
        Transform::First => {
            match input {
                IntermediateValue::LazyColumn { frame, column } => {
                    let df = frame
                        .select([col(&column)])
                        .limit(1)
                        .collect()
                        .map_err(polars_to_error)?;
                    let value = extract_first_string(&df, &column)?;
                    Ok(IntermediateValue::Scalar(value))
                }
                IntermediateValue::ColumnValues(vals) => {
                    let first = vals.into_iter().next().unwrap_or_default();
                    Ok(IntermediateValue::Scalar(first))
                }
                IntermediateValue::LazyRows(_) => {
                    Err(Error::variable("first requires column input, not step"))
                }
                _ => Err(Error::variable("first requires column input")),
            }
        }

        Transform::At(index) => {
            match input {
                IntermediateValue::LazyColumn { frame, column } => {
                    let df = frame
                        .select([col(&column)])
                        .slice(*index as i64, 1)
                        .collect()
                        .map_err(polars_to_error)?;
                    let value = extract_first_string(&df, &column)?;
                    Ok(IntermediateValue::Scalar(value))
                }
                IntermediateValue::ColumnValues(vals) => {
                    let value = vals.into_iter().nth(*index).unwrap_or_default();
                    Ok(IntermediateValue::Scalar(value))
                }
                _ => Err(Error::variable("at() requires column input")),
            }
        }

        Transform::Array => {
            match input {
                IntermediateValue::LazyColumn { frame, column } => {
                    let values = collect_column_as_strings(frame, &column)?;
                    Ok(IntermediateValue::ColumnValues(values))
                }
                IntermediateValue::ColumnValues(vals) => {
                    Ok(IntermediateValue::ColumnValues(vals))
                }
                IntermediateValue::Scalar(val) => {
                    // For scalars (from inputs), check if this is an array-type input
                    if let Source::Input { name } = &pipeline.source {
                        let input_type = _ctx.get_input_type(name);
                        match input_type {
                            InputType::Array => {
                                // Split comma-separated values for array-type inputs
                                let values: Vec<String> = val
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                Ok(IntermediateValue::ColumnValues(values))
                            }
                            InputType::String => {
                                Err(Error::variable(format!(
                                    "Cannot use '| array' transform on string-type input '{}'",
                                    name
                                )))
                            }
                        }
                    } else {
                        Err(Error::variable("array requires column or array input"))
                    }
                }
                _ => Err(Error::variable("array requires column or array input")),
            }
        }

        Transform::Unique => {
            match input {
                IntermediateValue::LazyColumn { frame, column } => {
                    // Use Polars unique() - preserves first occurrence order with unique_stable
                    let unique_frame = frame.select([col(&column).unique_stable()]);
                    Ok(IntermediateValue::LazyColumn {
                        frame: unique_frame,
                        column,
                    })
                }
                IntermediateValue::ColumnValues(vals) => {
                    // Fallback for already-collected values
                    let mut seen = std::collections::HashSet::new();
                    let unique: Vec<String> = vals
                        .into_iter()
                        .filter(|v| seen.insert(v.clone()))
                        .collect();
                    Ok(IntermediateValue::ColumnValues(unique))
                }
                _ => Err(Error::variable("unique requires column input")),
            }
        }

        Transform::StrJoin(sep) => {
            match input {
                IntermediateValue::LazyColumn { frame, column } => {
                    let values = collect_column_as_strings(frame, &column)?;
                    let joined = values.join(sep);
                    Ok(IntermediateValue::Scalar(joined))
                }
                IntermediateValue::ColumnValues(vals) => {
                    let joined = vals.join(sep);
                    Ok(IntermediateValue::Scalar(joined))
                }
                _ => Err(Error::variable("str_join requires column input")),
            }
        }

        Transform::ForEach => {
            match input {
                IntermediateValue::LazyColumn { frame, column } => {
                    let values = collect_column_as_strings(frame, &column)?;
                    Ok(IntermediateValue::ColumnValues(values))
                }
                IntermediateValue::ColumnValues(vals) => {
                    Ok(IntermediateValue::ColumnValues(vals))
                }
                IntermediateValue::Scalar(val) => {
                    // For scalars (from inputs), check if this is an array-type input
                    if let Source::Input { name } = &pipeline.source {
                        let input_type = _ctx.get_input_type(name);
                        match input_type {
                            InputType::Array => {
                                // Split comma-separated values for array-type inputs
                                let values: Vec<String> = val
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                Ok(IntermediateValue::ColumnValues(values))
                            }
                            InputType::String => {
                                Err(Error::variable(format!(
                                    "Cannot use '| for_each' transform on string-type input '{}'",
                                    name
                                )))
                            }
                        }
                    } else {
                        Err(Error::variable("for_each requires column or array input"))
                    }
                }
                _ => Err(Error::variable("for_each requires column or array input")),
            }
        }

        // ========== Comparison transforms ==========
        Transform::Eq(expected) => {
            apply_comparison(input, expected, |a, b| a == b)
        }

        Transform::Neq(expected) => {
            apply_comparison(input, expected, |a, b| a != b)
        }

        Transform::Gt(expected) => {
            apply_comparison(input, expected, |a, b| a > b)
        }

        Transform::Gte(expected) => {
            apply_comparison(input, expected, |a, b| a >= b)
        }

        Transform::Lt(expected) => {
            apply_comparison(input, expected, |a, b| a < b)
        }

        Transform::Lte(expected) => {
            apply_comparison(input, expected, |a, b| a <= b)
        }

        // ========== Formatting ==========
        Transform::Quote(_style) => {
            // Quote style is applied at final formatting, just pass through
            Ok(input)
        }
    }
}

/// Apply a comparison transform.
fn apply_comparison<F>(
    input: IntermediateValue,
    expected: &crate::variable::predicate::PredicateValue,
    cmp: F,
) -> Result<IntermediateValue>
where
    F: Fn(f64, f64) -> bool,
{
    use crate::variable::predicate::PredicateValue;

    match input {
        IntermediateValue::Integer(n) => {
            let result = match expected {
                PredicateValue::Number(expected_n) => cmp(n as f64, *expected_n),
                PredicateValue::String(s) => {
                    if let Ok(expected_n) = s.parse::<f64>() {
                        cmp(n as f64, expected_n)
                    } else {
                        false
                    }
                }
                _ => false,
            };
            Ok(IntermediateValue::Boolean(result))
        }

        IntermediateValue::Scalar(s) => {
            let result = match expected {
                PredicateValue::String(expected_s) => {
                    // For scalars, use string comparison for eq/neq
                    s == *expected_s
                }
                PredicateValue::Number(expected_n) => {
                    if let Ok(n) = s.parse::<f64>() {
                        cmp(n, *expected_n)
                    } else {
                        false
                    }
                }
                PredicateValue::Bool(expected_b) => {
                    let actual = s.eq_ignore_ascii_case("true");
                    actual == *expected_b
                }
            };
            Ok(IntermediateValue::Boolean(result))
        }

        _ => Err(Error::variable("Comparison requires scalar or integer input")),
    }
}

/// Extract column values from JSON rows.
#[allow(dead_code)]
fn extract_column_from_rows(rows: &[JsonValue], column: &str) -> Vec<String> {
    rows.iter()
        .filter_map(|row| row.get(column))
        .map(json_value_to_string)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Convert a JSON value to a string.
#[allow(dead_code)]
fn json_value_to_string(value: &JsonValue) -> String {
    match value {
        JsonValue::String(s) => s.clone(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Null => String::new(),
        _ => value.to_string(),
    }
}

/// Convert an intermediate value to a final result.
fn intermediate_to_result(
    value: IntermediateValue,
    _quote_style: QuoteStyle,
) -> Result<TransformResult> {
    match value {
        IntermediateValue::Scalar(s) => Ok(TransformResult::Scalar(s)),
        IntermediateValue::ColumnValues(values) => {
            Ok(TransformResult::Array(values))
        }
        IntermediateValue::Integer(n) => Ok(TransformResult::Scalar(n.to_string())),
        IntermediateValue::Boolean(b) => Ok(TransformResult::Boolean(b)),
        IntermediateValue::LazyRows(_) => {
            Err(Error::variable("Step reference requires a transform"))
        }
        IntermediateValue::LazyColumn { column, .. } => {
            Err(Error::variable(format!(
                "Column '{}' reference requires a transform (first, array, etc.)",
                column
            )))
        }
    }
}

// ========== Polars Helper Functions ==========

/// Convert a Polars error to our error type.
#[allow(dead_code)]
fn polars_to_error(e: PolarsError) -> Error {
    Error::variable(format!("Polars error: {}", e))
}

/// Collect a column from a LazyFrame as Vec<String>.
///
/// Filters out null values and empty strings.
#[allow(dead_code)]
fn collect_column_as_strings(lf: LazyFrame, column: &str) -> Result<Vec<String>> {
    let df = lf
        .select([col(column)])
        .collect()
        .map_err(polars_to_error)?;

    let polars_col = df
        .column(column)
        .map_err(|_| Error::variable(format!("Column '{}' not found", column)))?;

    // Convert Column to Series for iteration
    let series = polars_col.as_materialized_series();
    let mut values = Vec::with_capacity(series.len());

    for i in 0..series.len() {
        match series.get(i) {
            Ok(AnyValue::Null) => continue,
            Ok(AnyValue::String(s)) => {
                if !s.is_empty() {
                    values.push(s.to_string());
                }
            }
            Ok(other) => {
                let s = format!("{}", other);
                if !s.is_empty() {
                    values.push(s);
                }
            }
            Err(_) => continue,
        }
    }

    Ok(values)
}

/// Extract the first value from a single-column DataFrame as a String.
///
/// Returns empty string if the DataFrame is empty or the value is null.
#[allow(dead_code)]
fn extract_first_string(df: &DataFrame, column: &str) -> Result<String> {
    if df.height() == 0 {
        return Ok(String::new());
    }

    let polars_col = df
        .column(column)
        .map_err(|_| Error::variable(format!("Column '{}' not found", column)))?;

    let series = polars_col.as_materialized_series();

    match series.get(0) {
        Ok(AnyValue::Null) => Ok(String::new()),
        Ok(AnyValue::String(s)) => Ok(s.to_string()),
        Ok(other) => Ok(format!("{}", other)),
        Err(_) => Ok(String::new()),
    }
}

/// Convert a Predicate to a Polars filter expression.
///
/// Translates our predicate syntax (field op value) into Polars Expr.
#[allow(dead_code)]
fn predicate_to_polars_expr(predicate: &Predicate) -> Result<Expr> {
    let field_expr = col(&predicate.field);

    let value_expr: Expr = match &predicate.value {
        PredicateValue::Bool(b) => lit(*b),
        PredicateValue::Number(n) => lit(*n),
        PredicateValue::String(s) => lit(s.clone()),
    };

    let expr = match predicate.op {
        ComparisonOp::Eq => field_expr.eq(value_expr),
        ComparisonOp::Neq => field_expr.neq(value_expr),
        ComparisonOp::Gt => field_expr.gt(value_expr),
        ComparisonOp::Gte => field_expr.gt_eq(value_expr),
        ComparisonOp::Lt => field_expr.lt(value_expr),
        ComparisonOp::Lte => field_expr.lt_eq(value_expr),
    };

    Ok(expr)
}

/// Create an empty LazyFrame for cases where step doesn't exist.
#[allow(dead_code)]
fn empty_lazy_frame() -> LazyFrame {
    DataFrame::empty().lazy()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::result::ResultWriter;
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("kql-panopticon-test")
            .join("evaluation")
            .join(name)
    }

    fn create_test_results(path: &PathBuf, rows: &[JsonValue]) -> ResultContext {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }

        let mut writer = ResultWriter::new(path, "test_step").unwrap();
        writer.write_rows(rows).unwrap();
        let handle = writer.finish().unwrap();

        let mut ctx = ResultContext::new();
        ctx.insert("test_step", handle);
        ctx
    }

    #[test]
    fn test_evaluate_input() {
        let pipeline = Pipeline::parse("inputs.user").unwrap();
        let ctx = EvaluationContext::new().with_input("user", "alice");

        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_scalar(), Some("alice"));
    }

    #[test]
    fn test_evaluate_secret() {
        let pipeline = Pipeline::parse("secrets.api_key").unwrap();
        let ctx = EvaluationContext::new().with_secret("api_key", "secret123");

        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_scalar(), Some("secret123"));
    }

    #[test]
    fn test_evaluate_step_is_empty() {
        let path = temp_path("is_empty.jsonl");
        let results = create_test_results(&path, &[]);

        let pipeline = Pipeline::parse("test_step | is_empty").unwrap();
        let ctx = EvaluationContext::new().with_step_results(results);

        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_bool(), Some(true));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_evaluate_step_is_not_empty() {
        let path = temp_path("is_not_empty.jsonl");
        let results = create_test_results(
            &path,
            &[serde_json::json!({"name": "alice"})],
        );

        let pipeline = Pipeline::parse("test_step | is_not_empty").unwrap();
        let ctx = EvaluationContext::new().with_step_results(results);

        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_bool(), Some(true));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_evaluate_step_length() {
        let path = temp_path("length.jsonl");
        let results = create_test_results(
            &path,
            &[
                serde_json::json!({"x": 1}),
                serde_json::json!({"x": 2}),
                serde_json::json!({"x": 3}),
            ],
        );

        let pipeline = Pipeline::parse("test_step | length").unwrap();
        let ctx = EvaluationContext::new().with_step_results(results);

        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_scalar(), Some("3"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_evaluate_column_first() {
        let path = temp_path("first.jsonl");
        let results = create_test_results(
            &path,
            &[
                serde_json::json!({"name": "alice"}),
                serde_json::json!({"name": "bob"}),
            ],
        );

        let pipeline = Pipeline::parse("test_step.name | first").unwrap();
        let ctx = EvaluationContext::new().with_step_results(results);

        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_scalar(), Some("alice"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_evaluate_column_array() {
        let path = temp_path("array.jsonl");
        let results = create_test_results(
            &path,
            &[
                serde_json::json!({"name": "alice"}),
                serde_json::json!({"name": "bob"}),
            ],
        );

        let pipeline = Pipeline::parse("test_step.name | array").unwrap();
        let ctx = EvaluationContext::new().with_step_results(results);

        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_array(), Some(&["alice".to_string(), "bob".to_string()][..]));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_evaluate_column_unique() {
        let path = temp_path("unique.jsonl");
        let results = create_test_results(
            &path,
            &[
                serde_json::json!({"name": "alice"}),
                serde_json::json!({"name": "bob"}),
                serde_json::json!({"name": "alice"}),
            ],
        );

        let pipeline = Pipeline::parse("test_step.name | unique | array").unwrap();
        let ctx = EvaluationContext::new().with_step_results(results);

        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_array(), Some(&["alice".to_string(), "bob".to_string()][..]));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_evaluate_any_predicate() {
        let path = temp_path("any.jsonl");
        let results = create_test_results(
            &path,
            &[
                serde_json::json!({"score": 85}),
                serde_json::json!({"score": 92}),
                serde_json::json!({"score": 78}),
            ],
        );

        let pipeline = Pipeline::parse("test_step | any(score > 90)").unwrap();
        let ctx = EvaluationContext::new().with_step_results(results);

        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_bool(), Some(true));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_evaluate_all_predicate() {
        let path = temp_path("all.jsonl");
        let results = create_test_results(
            &path,
            &[
                serde_json::json!({"score": 85}),
                serde_json::json!({"score": 92}),
                serde_json::json!({"score": 78}),
            ],
        );

        let pipeline = Pipeline::parse("test_step | all(score > 70)").unwrap();
        let ctx = EvaluationContext::new().with_step_results(results);

        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_bool(), Some(true));

        let pipeline = Pipeline::parse("test_step | all(score > 80)").unwrap();
        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_bool(), Some(false));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_evaluate_length_comparison() {
        let path = temp_path("length_cmp.jsonl");
        let results = create_test_results(
            &path,
            &[
                serde_json::json!({"x": 1}),
                serde_json::json!({"x": 2}),
                serde_json::json!({"x": 3}),
            ],
        );

        let pipeline = Pipeline::parse("test_step | length | gt(2)").unwrap();
        let ctx = EvaluationContext::new().with_step_results(results);

        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_bool(), Some(true));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_evaluate_format_array() {
        let path = temp_path("format_array.jsonl");
        let results = create_test_results(
            &path,
            &[
                serde_json::json!({"name": "alice"}),
                serde_json::json!({"name": "bob"}),
            ],
        );

        let pipeline = Pipeline::parse("test_step.name | array").unwrap();
        let ctx = EvaluationContext::new().with_step_results(results);

        let result = evaluate(&pipeline, &ctx).unwrap();
        let formatted = result.format(QuoteStyle::Single);
        assert_eq!(formatted, "'alice','bob'");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_evaluate_str_join() {
        let path = temp_path("str_join.jsonl");
        let results = create_test_results(
            &path,
            &[
                serde_json::json!({"name": "alice"}),
                serde_json::json!({"name": "bob"}),
            ],
        );

        let pipeline = Pipeline::parse("test_step.name | str_join(', ')").unwrap();
        let ctx = EvaluationContext::new().with_step_results(results);

        let result = evaluate(&pipeline, &ctx).unwrap();
        assert_eq!(result.as_scalar(), Some("alice, bob"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_evaluate_input_array() {
        let pipeline = Pipeline::parse("inputs.targets | array").unwrap();
        let mut input_types = HashMap::new();
        input_types.insert("targets".to_string(), InputType::Array);

        let ctx = EvaluationContext::new()
            .with_input("targets", "10.0.0.1, 10.0.0.2, 10.0.0.3")
            .with_input_types(input_types);

        let result = evaluate(&pipeline, &ctx).unwrap();
        let formatted = result.format(QuoteStyle::Single);
        assert_eq!(formatted, "'10.0.0.1','10.0.0.2','10.0.0.3'");
    }

    #[test]
    fn test_evaluate_input_array_on_string_type_error() {
        let pipeline = Pipeline::parse("inputs.user | array").unwrap();
        let mut input_types = HashMap::new();
        input_types.insert("user".to_string(), InputType::String);

        let ctx = EvaluationContext::new()
            .with_input("user", "alice")
            .with_input_types(input_types);

        let result = evaluate(&pipeline, &ctx);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Cannot use '| array' transform on string-type input"));
    }

    #[test]
    fn test_evaluate_input_scalar_default() {
        // Even array-type inputs start as scalars without explicit transform
        let pipeline = Pipeline::parse("inputs.targets").unwrap();
        let mut input_types = HashMap::new();
        input_types.insert("targets".to_string(), InputType::Array);

        let ctx = EvaluationContext::new()
            .with_input("targets", "10.0.0.1, 10.0.0.2")
            .with_input_types(input_types);

        let result = evaluate(&pipeline, &ctx).unwrap();
        // Returns raw value as scalar, not split into array
        assert_eq!(result.as_scalar(), Some("10.0.0.1, 10.0.0.2"));
    }
}
