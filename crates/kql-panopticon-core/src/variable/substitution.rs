//! Variable substitution engine
//!
//! Replaces variable references with actual values from step results,
//! inputs, secrets, and foreach iteration context.

use super::{VarRef, VarRefType};
use crate::error::{Error, Result};
use crate::execution::result::ResultContext;
use crate::pack::{ExampleValue, InputType, QuoteStyle};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Context for variable substitution during execution
#[derive(Debug, Default, Clone)]
pub struct SubstitutionContext {
    /// User-provided inputs
    pub inputs: HashMap<String, String>,

    /// Input types (for array vs string handling)
    pub input_types: HashMap<String, InputType>,

    /// Step results (file-backed via handles)
    pub step_results: ResultContext,

    /// Current foreach row (alias -> row data)
    pub foreach_row: Option<(String, JsonValue)>,

    /// Secrets (resolved from environment)
    pub secrets: HashMap<String, String>,

    /// Default quote style for values
    pub default_quote_style: QuoteStyle,
}

impl SubstitutionContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an input value
    pub fn with_input(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.inputs.insert(name.into(), value.into());
        self
    }

    /// Add an input value with its type
    pub fn with_typed_input(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
        input_type: InputType,
    ) -> Self {
        let name = name.into();
        self.inputs.insert(name.clone(), value.into());
        self.input_types.insert(name, input_type);
        self
    }

    /// Set input type for an existing input
    pub fn with_input_type(mut self, name: impl Into<String>, input_type: InputType) -> Self {
        self.input_types.insert(name.into(), input_type);
        self
    }

    /// Add step result handle
    pub fn with_step_handle(
        mut self,
        step: impl Into<String>,
        handle: crate::execution::result::ResultHandle,
    ) -> Self {
        self.step_results.insert(step.into(), handle);
        self
    }

    /// Set step results context
    pub fn with_step_results_context(mut self, context: ResultContext) -> Self {
        self.step_results = context;
        self
    }

    /// Set foreach row context
    pub fn with_foreach_row(mut self, alias: impl Into<String>, row: JsonValue) -> Self {
        self.foreach_row = Some((alias.into(), row));
        self
    }

    /// Add a secret
    pub fn with_secret(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.secrets.insert(name.into(), value.into());
        self
    }

    /// Set default quote style
    pub fn with_quote_style(mut self, style: QuoteStyle) -> Self {
        self.default_quote_style = style;
        self
    }

    /// Resolve a variable reference to its value
    pub fn resolve(&self, var_ref: &VarRef) -> Result<String> {
        self.resolve_with_quote_style(var_ref, self.default_quote_style)
    }

    /// Resolve with specific quote style
    pub fn resolve_with_quote_style(
        &self,
        var_ref: &VarRef,
        quote_style: QuoteStyle,
    ) -> Result<String> {
        match &var_ref.ref_type {
            VarRefType::Input { name } => {
                let value = self
                    .inputs
                    .get(name)
                    .ok_or_else(|| Error::variable(format!("Input '{}' not found", name)))?;

                // Check if this input is typed as an array
                let input_type = self.input_types.get(name).copied().unwrap_or_default();
                match input_type {
                    InputType::String => Ok(value.clone()),
                    InputType::Array => {
                        // Split comma-separated values and format with quotes
                        let values: Vec<String> = value
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        Ok(quote_style.format_array(&values))
                    }
                }
            }

            VarRefType::Secret { name } => self
                .secrets
                .get(name)
                .cloned()
                .ok_or_else(|| Error::variable(format!("Secret '{}' not found", name))),

            VarRefType::StepArray { step, column } => {
                let values = self.get_column_values(step, column)?;
                Ok(quote_style.format_array(&values))
            }

            VarRefType::StepFirst { step, column } => {
                let values = self.get_column_values(step, column)?;
                values
                    .first()
                    .map(|v| quote_style.format_value(v))
                    .ok_or_else(|| Error::variable(format!("Step '{}' has no results", step)))
            }

            VarRefType::StepIndex { step, index, column } => {
                let values = self.get_column_values(step, column)?;
                values
                    .get(*index)
                    .map(|v| quote_style.format_value(v))
                    .ok_or_else(|| {
                        Error::variable(format!(
                            "Step '{}' has no row at index {}",
                            step, index
                        ))
                    })
            }

            VarRefType::Alias { alias, column } => {
                if let Some((current_alias, row)) = &self.foreach_row {
                    if current_alias == alias {
                        let value = extract_json_value(row, column);
                        return Ok(quote_style.format_value(&value));
                    }
                }
                // Not a foreach alias - try as a step reference (first row)
                if let Ok(values) = self.get_column_values(alias, column) {
                    if let Some(v) = values.first() {
                        return Ok(quote_style.format_value(v));
                    }
                }
                Err(Error::variable(format!(
                    "Alias '{}' not found in foreach context or as step",
                    alias
                )))
            }

            VarRefType::Unknown { content } => {
                Err(Error::variable(format!("Unknown variable syntax: {}", content)))
            }
        }
    }

    /// Get all values from a column in step results
    fn get_column_values(&self, step: &str, column: &str) -> Result<Vec<String>> {
        if !self.step_results.contains(step) {
            return Err(Error::variable(format!("Step '{}' not found in results", step)));
        }

        self.step_results.column_values(step, column)
    }
}

/// Context for validation substitution (uses example/placeholder values)
#[derive(Debug, Default)]
pub struct ValidationContext {
    /// User-defined examples (var_ref inner -> value)
    pub examples: HashMap<String, ExampleValue>,

    /// Input examples (from input definitions)
    pub input_examples: HashMap<String, String>,

    /// Default quote style
    pub default_quote_style: QuoteStyle,

    /// Track which substitutions used defaults
    pub used_defaults: Vec<String>,
}

impl ValidationContext {
    /// Create a new validation context
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an example value
    pub fn with_example(mut self, var_ref: impl Into<String>, value: ExampleValue) -> Self {
        self.examples.insert(var_ref.into(), value);
        self
    }

    /// Add input example
    pub fn with_input_example(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.input_examples.insert(name.into(), value.into());
        self
    }

    /// Set quote style
    pub fn with_quote_style(mut self, style: QuoteStyle) -> Self {
        self.default_quote_style = style;
        self
    }

    /// Resolve a variable reference for validation
    pub fn resolve(&mut self, var_ref: &VarRef) -> String {
        self.resolve_with_quote_style(var_ref, self.default_quote_style)
    }

    /// Resolve with specific quote style
    pub fn resolve_with_quote_style(
        &mut self,
        var_ref: &VarRef,
        quote_style: QuoteStyle,
    ) -> String {
        // Check for user-defined example
        if let Some(example) = self.examples.get(&var_ref.inner) {
            return match example {
                ExampleValue::Single(s) => quote_style.format_value(s),
                ExampleValue::Array(arr) => quote_style.format_array(arr),
            };
        }

        // Check for input example
        if let VarRefType::Input { name } = &var_ref.ref_type {
            if let Some(example) = self.input_examples.get(name) {
                return example.clone();
            }
        }

        // Use default placeholder and track it
        self.used_defaults.push(var_ref.inner.clone());
        var_ref.default_placeholder()
    }

    /// Get count of substitutions that used defaults
    pub fn default_count(&self) -> usize {
        self.used_defaults.len()
    }

    /// Get list of variable refs that used defaults
    pub fn defaults_used(&self) -> &[String] {
        &self.used_defaults
    }
}

/// Substitute all variables in a string using execution context
pub fn substitute(input: &str, context: &SubstitutionContext) -> Result<String> {
    substitute_with_quote_style(input, context, context.default_quote_style)
}

/// Substitute with specific quote style
pub fn substitute_with_quote_style(
    input: &str,
    context: &SubstitutionContext,
    quote_style: QuoteStyle,
) -> Result<String> {
    let vars = VarRef::parse_all(input);

    if vars.is_empty() {
        return Ok(input.to_string());
    }

    let mut result = input.to_string();
    for var in vars {
        let replacement = context.resolve_with_quote_style(&var, quote_style)?;
        result = result.replace(&var.full_match, &replacement);
    }

    Ok(result)
}

/// Substitute variables for validation (using examples/placeholders)
pub fn substitute_for_validation(input: &str, context: &mut ValidationContext) -> String {
    substitute_for_validation_with_quote_style(input, context, context.default_quote_style)
}

/// Substitute for validation with specific quote style
pub fn substitute_for_validation_with_quote_style(
    input: &str,
    context: &mut ValidationContext,
    quote_style: QuoteStyle,
) -> String {
    let vars = VarRef::parse_all(input);

    if vars.is_empty() {
        return input.to_string();
    }

    let mut result = input.to_string();
    for var in vars {
        let replacement = context.resolve_with_quote_style(&var, quote_style);
        result = result.replace(&var.full_match, &replacement);
    }

    result
}

/// Extract a value from a JSON object by column name
fn extract_json_value(row: &JsonValue, column: &str) -> String {
    row.get(column)
        .map(|v| match v {
            JsonValue::Null => String::new(),
            JsonValue::Bool(b) => b.to_string(),
            JsonValue::Number(n) => n.to_string(),
            JsonValue::String(s) => s.clone(),
            JsonValue::Array(_) | JsonValue::Object(_) => v.to_string(),
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::result::ResultWriter;

    /// Helper to create a step result handle with test data
    fn create_test_handle(
        dir: &std::path::Path,
        step_name: &str,
        rows: &[serde_json::Value],
    ) -> crate::execution::result::ResultHandle {
        let path = dir.join(format!("{}.jsonl", step_name));
        let mut writer = ResultWriter::new(&path, step_name).unwrap();
        writer.write_rows(rows).unwrap();
        writer.finish().unwrap()
    }

    #[test]
    fn test_substitute_input() {
        let context = SubstitutionContext::new().with_input("name", "Alice");
        let result = substitute("Hello {{inputs.name}}!", &context).unwrap();
        assert_eq!(result, "Hello Alice!");
    }

    #[test]
    fn test_substitute_missing_input() {
        let context = SubstitutionContext::new();
        let result = substitute("Hello {{inputs.name}}!", &context);
        assert!(result.is_err());
    }

    #[test]
    fn test_substitute_step_array() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = vec![
            serde_json::json!({"id": "1", "name": "Alice"}),
            serde_json::json!({"id": "2", "name": "Bob"}),
        ];

        let handle = create_test_handle(temp_dir.path(), "users", &results);
        let context = SubstitutionContext::new().with_step_handle("users", handle);
        let result = substitute("WHERE id IN ({{users.*.id}})", &context).unwrap();
        assert_eq!(result, "WHERE id IN ('1','2')");
    }

    #[test]
    fn test_substitute_step_first() {
        let temp_dir = tempfile::tempdir().unwrap();
        let results = vec![serde_json::json!({"id": "1", "name": "Alice"})];

        let handle = create_test_handle(temp_dir.path(), "users", &results);
        let context = SubstitutionContext::new().with_step_handle("users", handle);
        let result = substitute("WHERE id = {{users.first.id}}", &context).unwrap();
        assert_eq!(result, "WHERE id = '1'");
    }

    #[test]
    fn test_substitute_foreach() {
        let row = serde_json::json!({"id": "42", "email": "test@example.com"});
        let context = SubstitutionContext::new().with_foreach_row("u", row);

        let result = substitute("WHERE id = {{u.id}}", &context).unwrap();
        assert_eq!(result, "WHERE id = '42'");
    }

    #[test]
    fn test_validation_with_examples() {
        let mut context = ValidationContext::new()
            .with_example("users.*.Email", ExampleValue::Array(vec![
                "user1@example.com".to_string(),
                "user2@example.com".to_string(),
            ]));

        let result = substitute_for_validation(
            "WHERE Email IN ({{users.*.Email}})",
            &mut context,
        );

        assert!(result.contains("user1@example.com"));
        assert!(result.contains("user2@example.com"));
        assert_eq!(context.default_count(), 0);
    }

    #[test]
    fn test_validation_with_defaults() {
        let mut context = ValidationContext::new();

        let result = substitute_for_validation(
            "WHERE User = '{{inputs.target}}'",
            &mut context,
        );

        assert!(result.contains("placeholder"));
        assert_eq!(context.default_count(), 1);
        assert!(context.defaults_used().contains(&"inputs.target".to_string()));
    }

    #[test]
    fn test_validation_input_example() {
        let mut context = ValidationContext::new()
            .with_input_example("url", "https://example.com/test");

        let result = substitute_for_validation(
            "WHERE Url == '{{inputs.url}}'",
            &mut context,
        );

        assert!(result.contains("https://example.com/test"));
        assert_eq!(context.default_count(), 0);
    }

    #[test]
    fn test_quote_styles() {
        let context = SubstitutionContext::new()
            .with_input("name", "O'Brien")
            .with_quote_style(QuoteStyle::Single);

        let result = substitute("WHERE name = {{inputs.name}}", &context).unwrap();
        // String inputs are returned raw (no quoting)
        assert_eq!(result, "WHERE name = O'Brien");
    }

    #[test]
    fn test_array_input_type() {
        // Array inputs are split and quoted
        let context = SubstitutionContext::new()
            .with_typed_input("ips", "8.8.8.8, 1.1.1.1", InputType::Array)
            .with_quote_style(QuoteStyle::Single);

        let result = substitute("dynamic([{{inputs.ips}}])", &context).unwrap();
        assert_eq!(result, "dynamic(['8.8.8.8','1.1.1.1'])");
    }

    #[test]
    fn test_array_input_single_value() {
        // Single value in array input
        let context = SubstitutionContext::new()
            .with_typed_input("ip", "8.8.8.8", InputType::Array)
            .with_quote_style(QuoteStyle::Single);

        let result = substitute("dynamic([{{inputs.ip}}])", &context).unwrap();
        assert_eq!(result, "dynamic(['8.8.8.8'])");
    }

    #[test]
    fn test_string_input_default() {
        // Without explicit type, inputs are strings (raw)
        let context = SubstitutionContext::new()
            .with_input("value", "test,value")
            .with_quote_style(QuoteStyle::Single);

        let result = substitute("{{inputs.value}}", &context).unwrap();
        // String type (default) returns raw value without splitting
        assert_eq!(result, "test,value");
    }
}
