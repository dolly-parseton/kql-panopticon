//! SubstitutionBuilder for variable substitution.
//!
//! The main entry point for variable substitution in templates.
//! Handles both simple substitution and `for_each` iteration.
//!
//! # Example
//!
//! ```ignore
//! // Simple substitution
//! let builder = SubstitutionBuilder::new(query, &ctx)?;
//! let resolved = builder.substitute()?;
//!
//! // With for_each iteration
//! let builder = SubstitutionBuilder::new(url, &ctx)?;
//! if builder.has_for_each() {
//!     for resolved in builder.substitute_iter()? {
//!         // Use each resolved URL
//!     }
//! }
//! ```

use crate::error::{Error, Result};
use crate::pack::QuoteStyle;
use crate::variable::evaluation::{evaluate, EvaluationContext, TransformResult};
use crate::variable::pipeline::Pipeline;
use crate::variable::transform::TransformType;

/// Context type for validation.
///
/// Different contexts have different requirements for valid output types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextType {
    /// KQL query - allows Scalar and Array outputs.
    KqlQuery,
    /// HTTP request - allows Scalar, Array, and Iterator (for_each).
    HttpRequest,
    /// Condition (when clause) - requires Boolean output.
    Condition,
    /// Scoring indicator condition - requires Boolean output.
    ScoringCondition,
}

impl ContextType {
    /// Check if an output type is valid for this context.
    pub fn is_valid_output(&self, output_type: TransformType) -> bool {
        match self {
            ContextType::KqlQuery => output_type.valid_for_kql(),
            ContextType::HttpRequest => output_type.valid_for_http(),
            ContextType::Condition | ContextType::ScoringCondition => {
                output_type.valid_for_condition()
            }
        }
    }

    /// Get a human-readable name for this context.
    pub fn name(&self) -> &'static str {
        match self {
            ContextType::KqlQuery => "KQL query",
            ContextType::HttpRequest => "HTTP request",
            ContextType::Condition => "condition",
            ContextType::ScoringCondition => "scoring condition",
        }
    }
}

/// Builder for variable substitution.
///
/// Parses template text, validates pipelines, and performs substitution.
pub struct SubstitutionBuilder<'a, 'ctx> {
    /// Original template text.
    text: &'a str,
    /// Parsed pipelines from the template.
    pipelines: Vec<Pipeline>,
    /// Evaluation context.
    context: &'a EvaluationContext<'ctx>,
}

impl<'a, 'ctx> SubstitutionBuilder<'a, 'ctx> {
    /// Create a new builder for a template.
    ///
    /// Parses all `{{...}}` references in the template.
    pub fn new(text: &'a str, context: &'a EvaluationContext<'ctx>) -> Result<Self> {
        let pipelines = Pipeline::find_all(text)?;
        Ok(Self {
            text,
            pipelines,
            context,
        })
    }

    /// Check if any pipeline contains `for_each`.
    pub fn has_for_each(&self) -> bool {
        self.pipelines.iter().any(|p| p.has_for_each())
    }

    /// Get the pipeline with `for_each`, if any.
    fn for_each_pipeline(&self) -> Option<&Pipeline> {
        self.pipelines.iter().find(|p| p.has_for_each())
    }

    /// Get all step dependencies from all pipelines.
    pub fn step_dependencies(&self) -> Vec<&str> {
        let mut deps = Vec::new();
        for pipeline in &self.pipelines {
            deps.extend(pipeline.step_dependencies());
        }
        deps.sort();
        deps.dedup();
        deps
    }

    /// Validate all pipelines for a given context.
    ///
    /// Returns errors if any pipeline produces an invalid output type
    /// for the context.
    pub fn validate(&self, context_type: ContextType) -> Result<()> {
        // Check for multiple for_each (not allowed)
        let for_each_count = self.pipelines.iter().filter(|p| p.has_for_each()).count();
        if for_each_count > 1 {
            return Err(Error::variable(
                "Only one for_each allowed per step",
            ));
        }

        // Check for_each in KQL (not allowed)
        if context_type == ContextType::KqlQuery && self.has_for_each() {
            return Err(Error::variable(
                "for_each not supported in KQL steps. Use | array instead, or move to an HTTP step.",
            ));
        }

        // Validate each pipeline's output type
        for pipeline in &self.pipelines {
            let output_type = pipeline.validate()?;

            // Special case: for_each produces Iterator which is valid for HTTP
            if pipeline.has_for_each() && context_type == ContextType::HttpRequest {
                continue;
            }

            if !context_type.is_valid_output(output_type) {
                return Err(Error::variable(format!(
                    "Pipeline '{}' produces {:?} which is not valid for {}",
                    pipeline.inner,
                    output_type,
                    context_type.name()
                )));
            }
        }

        Ok(())
    }

    /// Substitute all variables in the template.
    ///
    /// Returns an error if any pipeline contains `for_each`.
    /// Use `substitute_iter` for templates with `for_each`.
    pub fn substitute(&self) -> Result<String> {
        if self.has_for_each() {
            return Err(Error::variable(
                "Template contains for_each. Use substitute_iter() instead.",
            ));
        }

        self.substitute_with_value(None)
    }

    /// Substitute with a specific value for the `for_each` pipeline.
    ///
    /// Used internally by `substitute_iter`.
    fn substitute_with_value(&self, for_each_value: Option<&str>) -> Result<String> {
        let mut result = self.text.to_string();
        let quote_style = self.context.default_quote_style();

        for pipeline in &self.pipelines {
            let formatted = if pipeline.has_for_each() {
                // Use the provided for_each value
                match for_each_value {
                    Some(v) => quote_style.format_value(v),
                    None => {
                        // Evaluate to get the array, but don't iterate
                        let eval_result = evaluate(pipeline, self.context)?;
                        eval_result.format(quote_style)
                    }
                }
            } else {
                let eval_result = evaluate(pipeline, self.context)?;
                eval_result.format(quote_style)
            };

            result = result.replace(&pipeline.full_match, &formatted);
        }

        Ok(result)
    }

    /// Get an iterator for `for_each` substitution.
    ///
    /// Returns an iterator that yields one substituted string per
    /// value in the `for_each` source.
    pub fn substitute_iter(&self) -> Result<SubstitutionIterator> {
        let for_each_pipeline = self.for_each_pipeline().ok_or_else(|| {
            Error::variable("No for_each found in template")
        })?;

        // Evaluate the for_each pipeline to get values
        let eval_result = evaluate(for_each_pipeline, self.context)?;

        let values = match eval_result {
            TransformResult::Array(vals) | TransformResult::Iterator(vals) => vals,
            _ => {
                return Err(Error::variable(
                    "for_each pipeline did not produce array values",
                ))
            }
        };

        // Pre-compute substitutions for non-for_each pipelines
        let quote_style = self.context.default_quote_style();
        let mut other_substitutions = Vec::new();

        for pipeline in &self.pipelines {
            if !pipeline.has_for_each() {
                let eval_result = evaluate(pipeline, self.context)?;
                let formatted = eval_result.format(quote_style);
                other_substitutions.push((pipeline.full_match.clone(), formatted));
            }
        }

        Ok(SubstitutionIterator {
            template: self.text.to_string(),
            for_each_match: for_each_pipeline.full_match.clone(),
            other_substitutions,
            values,
            index: 0,
            quote_style,
        })
    }

    /// Substitute and return a boolean result.
    ///
    /// Used for condition evaluation. The template should produce a boolean.
    pub fn substitute_bool(&self) -> Result<bool> {
        // For conditions, we need to handle compound expressions like "{{a}} and {{b}}"
        let substituted = self.substitute_condition()?;

        // Parse the result as a boolean expression
        evaluate_boolean_expression(&substituted)
    }

    /// Substitute for condition context.
    ///
    /// Evaluates all pipelines and returns the string result.
    fn substitute_condition(&self) -> Result<String> {
        let mut result = self.text.to_string();
        let quote_style = self.context.default_quote_style();

        for pipeline in &self.pipelines {
            let eval_result = evaluate(pipeline, self.context)?;

            // For conditions, format booleans as "true"/"false"
            let formatted = match &eval_result {
                TransformResult::Boolean(b) => b.to_string(),
                _ => eval_result.format(quote_style),
            };

            result = result.replace(&pipeline.full_match, &formatted);
        }

        Ok(result)
    }
}

/// Iterator for `for_each` substitution.
pub struct SubstitutionIterator {
    /// Template text with placeholder for for_each value
    template: String,
    /// Full match string to replace
    for_each_match: String,
    /// Other pipeline substitutions already applied
    other_substitutions: Vec<(String, String)>,
    /// Values to iterate over
    values: Vec<String>,
    /// Current index
    index: usize,
    /// Quote style
    quote_style: QuoteStyle,
}

impl Iterator for SubstitutionIterator {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.values.len() {
            return None;
        }

        let value = &self.values[self.index];
        self.index += 1;

        // Start with template
        let mut result = self.template.clone();

        // Apply other substitutions
        for (full_match, replacement) in &self.other_substitutions {
            result = result.replace(full_match, replacement);
        }

        // Apply for_each value
        let formatted_value = self.quote_style.format_value(value);
        result = result.replace(&self.for_each_match, &formatted_value);

        Some(Ok(result))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.values.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for SubstitutionIterator {
    fn len(&self) -> usize {
        self.values.len() - self.index
    }
}

/// Evaluate a boolean expression after variable substitution.
///
/// Handles compound expressions with `and`, `or`, `not`.
fn evaluate_boolean_expression(expr: &str) -> Result<bool> {
    let expr = expr.trim();

    // Handle literal values
    if expr.eq_ignore_ascii_case("true") {
        return Ok(true);
    }
    if expr.eq_ignore_ascii_case("false") {
        return Ok(false);
    }

    // Handle "and" (split and check all)
    if expr.contains(" and ") {
        return Ok(expr
            .split(" and ")
            .all(|part| evaluate_boolean_expression(part.trim()).unwrap_or(false)));
    }

    // Handle "or" (split and check any)
    if expr.contains(" or ") {
        return Ok(expr
            .split(" or ")
            .any(|part| evaluate_boolean_expression(part.trim()).unwrap_or(false)));
    }

    // Handle "not" prefix
    if let Some(rest) = expr.strip_prefix("not ") {
        return Ok(!evaluate_boolean_expression(rest.trim())?);
    }

    // Unknown expression - treat as false with warning
    Ok(false)
}

/// Convenience function for simple substitution.
///
/// Creates a builder and performs substitution in one step.
pub fn substitute(text: &str, context: &EvaluationContext<'_>) -> Result<String> {
    let builder = SubstitutionBuilder::new(text, context)?;
    builder.substitute()
}

/// Convenience function for condition evaluation.
///
/// Creates a builder and evaluates as boolean.
pub fn evaluate_condition(condition: &str, context: &EvaluationContext<'_>) -> Result<bool> {
    let builder = SubstitutionBuilder::new(condition, context)?;
    builder.substitute_bool()
}

/// Check if text contains any variable references.
pub fn contains_vars(text: &str) -> bool {
    Pipeline::contains_vars(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::result::{ResultContext, ResultWriter};
    use crate::pack::InputType;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("kql-panopticon-test")
            .join("builder")
            .join(name)
    }

    fn create_test_results(path: &PathBuf, rows: &[serde_json::Value]) -> ResultContext {
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
    fn test_simple_substitution() {
        let ctx = EvaluationContext::new()
            .with_input("user", "alice")
            .with_secret("key", "secret123");

        let result = substitute("User: {{inputs.user}}, Key: {{secrets.key}}", &ctx).unwrap();
        assert_eq!(result, "User: alice, Key: secret123");
    }

    #[test]
    fn test_step_substitution() {
        let path = temp_path("step_sub.jsonl");
        let results = create_test_results(
            &path,
            &[
                serde_json::json!({"name": "alice"}),
                serde_json::json!({"name": "bob"}),
            ],
        );

        let ctx = EvaluationContext::new().with_step_results(results);

        let result = substitute("First: {{test_step.name | first}}", &ctx).unwrap();
        assert_eq!(result, "First: alice");

        let result = substitute("All: {{test_step.name | array}}", &ctx).unwrap();
        assert_eq!(result, "All: 'alice','bob'");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_validate_kql_context() {
        let ctx = EvaluationContext::new().with_input("user", "alice");

        // Scalar is valid for KQL
        let builder = SubstitutionBuilder::new("{{inputs.user}}", &ctx).unwrap();
        assert!(builder.validate(ContextType::KqlQuery).is_ok());

        // for_each is not valid for KQL
        let path = temp_path("validate_kql.jsonl");
        let results = create_test_results(&path, &[serde_json::json!({"x": 1})]);
        let ctx = EvaluationContext::new().with_step_results(results);

        let builder = SubstitutionBuilder::new("{{test_step.x | for_each}}", &ctx).unwrap();
        assert!(builder.validate(ContextType::KqlQuery).is_err());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_validate_http_context() {
        let path = temp_path("validate_http.jsonl");
        let results = create_test_results(&path, &[serde_json::json!({"ip": "10.0.0.1"})]);
        let ctx = EvaluationContext::new().with_step_results(results);

        // for_each is valid for HTTP
        let builder = SubstitutionBuilder::new("{{test_step.ip | for_each}}", &ctx).unwrap();
        assert!(builder.validate(ContextType::HttpRequest).is_ok());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_validate_condition_context() {
        let path = temp_path("validate_cond.jsonl");
        let results = create_test_results(&path, &[serde_json::json!({"x": 1})]);
        let ctx = EvaluationContext::new().with_step_results(results);

        // Boolean is valid for condition
        let builder = SubstitutionBuilder::new("{{test_step | is_not_empty}}", &ctx).unwrap();
        assert!(builder.validate(ContextType::Condition).is_ok());

        // Scalar is not valid for condition
        let builder = SubstitutionBuilder::new("{{test_step.x | first}}", &ctx).unwrap();
        assert!(builder.validate(ContextType::Condition).is_err());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_for_each_iteration() {
        let path = temp_path("foreach.jsonl");
        let results = create_test_results(
            &path,
            &[
                serde_json::json!({"ip": "10.0.0.1"}),
                serde_json::json!({"ip": "10.0.0.2"}),
                serde_json::json!({"ip": "10.0.0.3"}),
            ],
        );

        let ctx = EvaluationContext::new().with_step_results(results);

        let builder = SubstitutionBuilder::new(
            "https://api.example.com/check?ip={{test_step.ip | for_each}}",
            &ctx,
        )
        .unwrap();

        assert!(builder.has_for_each());

        let urls: Vec<String> = builder
            .substitute_iter()
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://api.example.com/check?ip='10.0.0.1'");
        assert_eq!(urls[1], "https://api.example.com/check?ip='10.0.0.2'");
        assert_eq!(urls[2], "https://api.example.com/check?ip='10.0.0.3'");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_evaluate_condition() {
        let path = temp_path("eval_cond.jsonl");
        let results = create_test_results(
            &path,
            &[
                serde_json::json!({"score": 85}),
                serde_json::json!({"score": 92}),
            ],
        );

        let ctx = EvaluationContext::new().with_step_results(results);

        // Simple condition
        assert!(evaluate_condition("{{test_step | is_not_empty}}", &ctx).unwrap());
        assert!(!evaluate_condition("{{test_step | is_empty}}", &ctx).unwrap());

        // Predicate condition
        assert!(evaluate_condition("{{test_step | any(score > 90)}}", &ctx).unwrap());
        assert!(!evaluate_condition("{{test_step | all(score > 90)}}", &ctx).unwrap());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_compound_condition() {
        let path = temp_path("compound.jsonl");
        let results = create_test_results(
            &path,
            &[serde_json::json!({"x": 1})],
        );

        let ctx = EvaluationContext::new().with_step_results(results);

        // And condition
        assert!(evaluate_condition(
            "{{test_step | is_not_empty}} and {{test_step | length | gt(0)}}",
            &ctx
        )
        .unwrap());

        // Or condition
        assert!(evaluate_condition(
            "{{test_step | is_empty}} or {{test_step | is_not_empty}}",
            &ctx
        )
        .unwrap());

        // Not condition
        assert!(evaluate_condition("not {{test_step | is_empty}}", &ctx).unwrap());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_multiple_for_each_error() {
        let path = temp_path("multi_foreach.jsonl");
        let results = create_test_results(&path, &[serde_json::json!({"x": 1, "y": 2})]);
        let ctx = EvaluationContext::new().with_step_results(results);

        let builder = SubstitutionBuilder::new(
            "{{test_step.x | for_each}} and {{test_step.y | for_each}}",
            &ctx,
        )
        .unwrap();

        assert!(builder.validate(ContextType::HttpRequest).is_err());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_contains_vars() {
        assert!(contains_vars("Hello {{name}}"));
        assert!(!contains_vars("No variables"));
    }

    #[test]
    fn test_step_dependencies() {
        let ctx = EvaluationContext::new();

        let builder = SubstitutionBuilder::new(
            "{{step1.a | first}} and {{step2.b | array}}",
            &ctx,
        )
        .unwrap();

        let deps = builder.step_dependencies();
        assert!(deps.contains(&"step1"));
        assert!(deps.contains(&"step2"));
    }

    #[test]
    fn test_input_array_substitution() {
        let mut input_types = HashMap::new();
        input_types.insert("targets".to_string(), InputType::Array);

        let ctx = EvaluationContext::new()
            .with_input("targets", "10.0.0.1, 10.0.0.2")
            .with_input_types(input_types);

        let result = substitute("let ips = dynamic([{{inputs.targets | array}}]);", &ctx).unwrap();
        assert_eq!(result, "let ips = dynamic(['10.0.0.1','10.0.0.2']);");
    }
}
