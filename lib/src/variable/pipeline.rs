//! Pipeline parsing for variable references.
//!
//! A pipeline represents a complete `{{...}}` variable reference with
//! source and transforms: `{{step.column | first | eq('value')}}`.
//!
//! # Syntax
//!
//! ```text
//! {{ source | transform | transform | ... }}
//! ```
//!
//! Where source is one of:
//! - `inputs.name` - User input
//! - `secrets.name` - Environment secret
//! - `step` - Step result (for step-level transforms)
//! - `step.column` - Column from step result
//!
//! And transforms are chained with `|`.

use crate::error::{Error, Result};
use crate::variable::source::Source;
use crate::variable::transform::{Transform, TransformType};
use regex::Regex;
use std::sync::LazyLock;

/// Regex to find all `{{...}}` patterns in text.
static VAR_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{([^}]+)\}\}").expect("Invalid variable pattern regex"));

/// Regex to parse filter().column syntax.
///
/// Matches: `filter(predicate).column`
static FILTER_COLUMN_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(filter\s*\([^)]+\))\.(\w+)$").expect("Invalid filter column regex")
});

/// A complete variable reference with source and transforms.
///
/// Parsed from `{{source | transform1 | transform2}}` syntax.
#[derive(Debug, Clone)]
pub struct Pipeline {
    /// Full match including braces: `{{step.column | first}}`
    pub full_match: String,

    /// Inner content without braces: `step.column | first`
    pub inner: String,

    /// Parsed source
    pub source: Source,

    /// Optional column selected after filter
    ///
    /// Set when using `{{step | filter(pred).column | ...}}` syntax.
    pub filter_column: Option<String>,

    /// Transform chain
    pub transforms: Vec<Transform>,
}

impl Pipeline {
    /// Parse a pipeline from the inner content (without braces).
    ///
    /// # Arguments
    ///
    /// * `inner` - Content between `{{` and `}}`, e.g., `step.column | first`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// Pipeline::parse("step.column | first")
    /// Pipeline::parse("step | is_empty")
    /// Pipeline::parse("inputs.user")
    /// Pipeline::parse("step | filter(score > 50).name | array")
    /// ```
    pub fn parse(inner: &str) -> Result<Self> {
        let inner = inner.trim();

        if inner.is_empty() {
            return Err(Error::variable("Empty variable reference"));
        }

        // Split by pipe, being careful not to split inside parentheses
        let parts = split_by_pipe(inner);

        if parts.is_empty() {
            return Err(Error::variable("Empty variable reference"));
        }

        // First part is the source
        let source_str = parts[0].trim();
        let source = Source::parse(source_str)?;

        // Rest are transforms
        let mut transforms = Vec::new();
        let mut filter_column = None;

        for part in parts.iter().skip(1) {
            let part = part.trim();

            // Check for filter().column syntax
            if let Some(captures) = FILTER_COLUMN_PATTERN.captures(part) {
                let filter_str = captures.get(1).expect("regex group 1").as_str();
                let column = captures.get(2).expect("regex group 2").as_str();

                let transform = Transform::parse(filter_str)?;
                transforms.push(transform);
                filter_column = Some(column.to_string());
            } else {
                let transform = Transform::parse(part)?;
                transforms.push(transform);
            }
        }

        let full_match = format!("{{{{{}}}}}", inner);

        Ok(Pipeline {
            full_match,
            inner: inner.to_string(),
            source,
            filter_column,
            transforms,
        })
    }

    /// Parse a pipeline from a full match including braces.
    pub fn parse_full(full_match: &str) -> Result<Self> {
        let inner = full_match
            .trim_start_matches("{{")
            .trim_end_matches("}}")
            .trim();

        let mut pipeline = Self::parse(inner)?;
        pipeline.full_match = full_match.to_string();
        Ok(pipeline)
    }

    /// Find all pipelines in a template string.
    ///
    /// Returns a vector of (full_match, Pipeline) pairs.
    pub fn find_all(text: &str) -> Result<Vec<Pipeline>> {
        let mut pipelines = Vec::new();

        for captures in VAR_PATTERN.captures_iter(text) {
            let full_match = captures.get(0).expect("regex match").as_str();
            let inner = captures.get(1).expect("regex group 1").as_str();

            let mut pipeline = Self::parse(inner)?;
            pipeline.full_match = full_match.to_string();
            pipelines.push(pipeline);
        }

        Ok(pipelines)
    }

    /// Check if any `{{...}}` patterns exist in the text.
    pub fn contains_vars(text: &str) -> bool {
        VAR_PATTERN.is_match(text)
    }

    /// Check if this pipeline contains a `for_each` transform.
    pub fn has_for_each(&self) -> bool {
        self.transforms.iter().any(|t| t.is_for_each())
    }

    /// Get all step dependencies from this pipeline.
    pub fn step_dependencies(&self) -> Vec<&str> {
        let mut deps = Vec::new();

        if let Some(step) = self.source.step_dependency() {
            deps.push(step);
        }

        deps
    }

    /// Validate the transform chain and return the final output type.
    ///
    /// This checks that each transform receives the expected input type
    /// from the previous transform or source.
    pub fn validate(&self) -> Result<TransformType> {
        // Determine initial type from source
        let mut current_type = match &self.source {
            Source::Input { .. } | Source::Secret { .. } => {
                // Inputs and secrets start as scalars (or arrays if typed)
                // For now, treat as Column since they need transforms
                TransformType::Column
            }
            Source::Step { .. } => TransformType::Step,
            Source::StepColumn { .. } => TransformType::Column,
        };

        // If we have a filter column, we need to transition through FilteredStep -> Column
        let mut filter_column_consumed = false;

        for (i, transform) in self.transforms.iter().enumerate() {
            let expected_input = transform.input_type();

            // Special handling for filter column syntax
            if matches!(current_type, TransformType::FilteredStep) {
                if self.filter_column.is_some() && !filter_column_consumed {
                    // After filter with column selection, we're now at Column type
                    current_type = TransformType::Column;
                    filter_column_consumed = true;
                }
            }

            // Step-level predicates (any/all/filter) can accept Step or FilteredStep
            let type_compatible = match (current_type, expected_input) {
                (t, e) if t == e => true,
                // FilteredStep can be treated as Step for further step-level ops
                (TransformType::FilteredStep, TransformType::Step) => true,
                // Array is compatible with Column (both are collections of values)
                (TransformType::Array, TransformType::Column) => true,
                _ => false,
            };

            if !type_compatible {
                return Err(Error::variable(format!(
                    "Transform '{}' expects {:?} input but received {:?} at position {}",
                    self.inner,
                    expected_input,
                    current_type,
                    i + 1
                )));
            }

            current_type = transform.output_type();
        }

        // Handle case where filter_column was specified but no transforms followed
        if matches!(current_type, TransformType::FilteredStep) && self.filter_column.is_some() {
            current_type = TransformType::Column;
        }

        // Check for bare column reference without terminal transform
        if matches!(current_type, TransformType::Column) && self.transforms.is_empty() {
            // Special case: inputs and secrets without transforms are OK
            if self.source.is_input() || self.source.is_secret() {
                return Ok(TransformType::Scalar);
            }

            return Err(Error::variable(format!(
                "Column reference '{}' requires a transform (first, array, etc.)",
                self.inner
            )));
        }

        Ok(current_type)
    }

    /// Get the final output type of this pipeline.
    ///
    /// Returns `None` if validation fails.
    pub fn output_type(&self) -> Option<TransformType> {
        self.validate().ok()
    }
}

/// Split a string by `|` but not inside parentheses.
///
/// This handles cases like `filter(a | b)` where the `|` inside
/// parentheses should not be treated as a pipe separator.
fn split_by_pipe(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut paren_depth: i32 = 0;

    for (i, c) in s.char_indices() {
        match c {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '|' if paren_depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }

    // Add the last part
    parts.push(&s[start..]);

    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_source() {
        let p = Pipeline::parse("inputs.user").unwrap();
        assert!(matches!(p.source, Source::Input { .. }));
        assert!(p.transforms.is_empty());
    }

    #[test]
    fn test_parse_step_with_transform() {
        let p = Pipeline::parse("step | is_empty").unwrap();
        assert!(matches!(p.source, Source::Step { .. }));
        assert_eq!(p.transforms.len(), 1);
        assert!(matches!(p.transforms[0], Transform::IsEmpty));
    }

    #[test]
    fn test_parse_column_with_transform() {
        let p = Pipeline::parse("step.column | first").unwrap();
        assert!(matches!(p.source, Source::StepColumn { .. }));
        assert_eq!(p.transforms.len(), 1);
        assert!(matches!(p.transforms[0], Transform::First));
    }

    #[test]
    fn test_parse_chained_transforms() {
        let p = Pipeline::parse("step.column | unique | array").unwrap();
        assert_eq!(p.transforms.len(), 2);
        assert!(matches!(p.transforms[0], Transform::Unique));
        assert!(matches!(p.transforms[1], Transform::Array));
    }

    #[test]
    fn test_parse_with_comparison() {
        let p = Pipeline::parse("step | length | gt(5)").unwrap();
        assert_eq!(p.transforms.len(), 2);
        assert!(matches!(p.transforms[0], Transform::Length));
        assert!(matches!(p.transforms[1], Transform::Gt(_)));
    }

    #[test]
    fn test_parse_filter_with_column() {
        let p = Pipeline::parse("step | filter(score > 50).name | array").unwrap();
        assert!(matches!(p.source, Source::Step { .. }));
        assert_eq!(p.filter_column, Some("name".to_string()));
        assert_eq!(p.transforms.len(), 2);
        assert!(matches!(p.transforms[0], Transform::Filter(_)));
        assert!(matches!(p.transforms[1], Transform::Array));
    }

    #[test]
    fn test_parse_full_match() {
        let p = Pipeline::parse_full("{{step.column | first}}").unwrap();
        assert_eq!(p.full_match, "{{step.column | first}}");
        assert!(matches!(p.source, Source::StepColumn { .. }));
    }

    #[test]
    fn test_find_all() {
        let text = "Hello {{inputs.name}} and {{step.value | first}}!";
        let pipelines = Pipeline::find_all(text).unwrap();
        assert_eq!(pipelines.len(), 2);
        assert!(matches!(pipelines[0].source, Source::Input { .. }));
        assert!(matches!(pipelines[1].source, Source::StepColumn { .. }));
    }

    #[test]
    fn test_contains_vars() {
        assert!(Pipeline::contains_vars("Hello {{name}}"));
        assert!(Pipeline::contains_vars("{{a}} and {{b}}"));
        assert!(!Pipeline::contains_vars("No variables here"));
        assert!(!Pipeline::contains_vars("Single { brace }"));
    }

    #[test]
    fn test_has_for_each() {
        let p = Pipeline::parse("step.column | for_each").unwrap();
        assert!(p.has_for_each());

        let p = Pipeline::parse("step.column | array").unwrap();
        assert!(!p.has_for_each());
    }

    #[test]
    fn test_step_dependencies() {
        let p = Pipeline::parse("suspicious_ips.IPAddress | array").unwrap();
        assert_eq!(p.step_dependencies(), vec!["suspicious_ips"]);

        let p = Pipeline::parse("inputs.user").unwrap();
        assert!(p.step_dependencies().is_empty());
    }

    #[test]
    fn test_validate_step_level() {
        let p = Pipeline::parse("step | is_empty").unwrap();
        assert_eq!(p.validate().unwrap(), TransformType::Boolean);

        let p = Pipeline::parse("step | length").unwrap();
        assert_eq!(p.validate().unwrap(), TransformType::Scalar);

        let p = Pipeline::parse("step | length | gt(5)").unwrap();
        assert_eq!(p.validate().unwrap(), TransformType::Boolean);
    }

    #[test]
    fn test_validate_column_level() {
        let p = Pipeline::parse("step.column | first").unwrap();
        assert_eq!(p.validate().unwrap(), TransformType::Scalar);

        let p = Pipeline::parse("step.column | array").unwrap();
        assert_eq!(p.validate().unwrap(), TransformType::Array);

        let p = Pipeline::parse("step.column | for_each").unwrap();
        assert_eq!(p.validate().unwrap(), TransformType::Iterator);
    }

    #[test]
    fn test_validate_input_without_transform() {
        // Inputs without transforms are OK (treated as scalar)
        let p = Pipeline::parse("inputs.user").unwrap();
        assert_eq!(p.validate().unwrap(), TransformType::Scalar);

        let p = Pipeline::parse("secrets.key").unwrap();
        assert_eq!(p.validate().unwrap(), TransformType::Scalar);
    }

    #[test]
    fn test_validate_column_requires_transform() {
        // Column reference without transform is an error
        let p = Pipeline::parse("step.column").unwrap();
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_split_by_pipe() {
        let parts = split_by_pipe("a | b | c");
        assert_eq!(parts, vec!["a ", " b ", " c"]);

        // Pipe inside parentheses should not split
        let parts = split_by_pipe("filter(a | b) | array");
        assert_eq!(parts, vec!["filter(a | b) ", " array"]);

        // Nested parentheses
        let parts = split_by_pipe("filter((a | b)) | first");
        assert_eq!(parts, vec!["filter((a | b)) ", " first"]);
    }

    #[test]
    fn test_validate_filter_column_chain() {
        let p = Pipeline::parse("step | filter(score > 50).name | array").unwrap();
        assert_eq!(p.validate().unwrap(), TransformType::Array);
    }
}
