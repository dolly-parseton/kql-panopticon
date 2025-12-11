//! Variable substitution engine
//!
//! Replaces variable references with actual values.

use super::{ExtractedValue, VarRef, VarRefType};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Quote style for substituted values
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuoteStyle {
    /// Single quotes: 'value'
    #[default]
    Single,
    /// Double quotes: "value"
    Double,
    /// No quotes: value
    Verbatim,
}

impl QuoteStyle {
    /// Quote a single value
    pub fn quote(&self, value: &str) -> String {
        match self {
            Self::Single => format!("'{}'", value.replace('\'', "''")),
            Self::Double => format!("\"{}\"", value.replace('"', "\\\"")),
            Self::Verbatim => value.to_string(),
        }
    }

    /// Quote an array of values and join
    pub fn quote_array(&self, values: &[String]) -> String {
        values
            .iter()
            .map(|v| self.quote(v))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Context for variable substitution
#[derive(Debug, Default)]
pub struct SubstitutionContext {
    /// User-provided inputs
    pub inputs: HashMap<String, String>,
    /// Extracted values from steps
    pub extractions: HashMap<String, ExtractedValue>,
    /// Step results (for direct column access)
    pub step_results: HashMap<String, Vec<serde_json::Value>>,
    /// Current foreach row (alias -> row data)
    pub foreach_row: Option<(String, serde_json::Value)>,
    /// Secrets (resolved from environment)
    pub secrets: HashMap<String, String>,
    /// Default quote style
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

    /// Add an extraction
    pub fn with_extraction(mut self, name: impl Into<String>, value: ExtractedValue) -> Self {
        self.extractions.insert(name.into(), value);
        self
    }

    /// Add step results
    pub fn with_step_results(
        mut self,
        step: impl Into<String>,
        results: Vec<serde_json::Value>,
    ) -> Self {
        self.step_results.insert(step.into(), results);
        self
    }

    /// Set foreach row
    pub fn with_foreach_row(mut self, alias: impl Into<String>, row: serde_json::Value) -> Self {
        self.foreach_row = Some((alias.into(), row));
        self
    }

    /// Add a secret
    pub fn with_secret(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.secrets.insert(name.into(), value.into());
        self
    }

    /// Resolve a variable reference
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
            VarRefType::Input { name } => self
                .inputs
                .get(name)
                .cloned()
                .ok_or_else(|| Error::variable(format!("Input '{}' not found", name))),

            VarRefType::Secret { name } => self
                .secrets
                .get(name)
                .cloned()
                .ok_or_else(|| Error::variable(format!("Secret '{}' not found", name))),

            VarRefType::StepArray { step, column } => {
                let values = self.get_column_values(step, column)?;
                Ok(quote_style.quote_array(&values))
            }

            VarRefType::StepFirst { step, column } => {
                let values = self.get_column_values(step, column)?;
                values
                    .first()
                    .map(|v| quote_style.quote(v))
                    .ok_or_else(|| Error::variable(format!("Step '{}' has no results", step)))
            }

            VarRefType::StepIndex { step, index, column } => {
                let values = self.get_column_values(step, column)?;
                values
                    .get(*index)
                    .map(|v| quote_style.quote(v))
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
                        let value = row
                            .get(column)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| {
                                row.get(column)
                                    .map(|v| v.to_string())
                                    .unwrap_or_default()
                            });
                        return Ok(quote_style.quote(&value));
                    }
                }
                // Try as legacy extraction
                self.extractions
                    .get(&format!("{}.{}", alias, column))
                    .or_else(|| self.extractions.get(alias))
                    .map(|v| v.to_substitution_string(quote_style))
                    .ok_or_else(|| {
                        Error::variable(format!("Alias '{}' not in scope", alias))
                    })
            }

            VarRefType::LegacyExtract { step, extract } => {
                let key = format!("{}.{}", step, extract);
                self.extractions
                    .get(&key)
                    .map(|v| v.to_substitution_string(quote_style))
                    .ok_or_else(|| {
                        Error::variable(format!("Extraction '{}.{}' not found", step, extract))
                    })
            }

            VarRefType::Unknown { content } => {
                Err(Error::variable(format!("Unknown variable syntax: {}", content)))
            }
        }
    }

    /// Get all values from a column in step results
    fn get_column_values(&self, step: &str, column: &str) -> Result<Vec<String>> {
        let results = self.step_results.get(step).ok_or_else(|| {
            Error::variable(format!("Step '{}' not found in results", step))
        })?;

        let values: Vec<String> = results
            .iter()
            .filter_map(|row| {
                row.get(column).map(|v| {
                    v.as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| v.to_string())
                })
            })
            .collect();

        Ok(values)
    }
}

/// Substitute all variables in a string
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quote_styles() {
        assert_eq!(QuoteStyle::Single.quote("test"), "'test'");
        assert_eq!(QuoteStyle::Double.quote("test"), "\"test\"");
        assert_eq!(QuoteStyle::Verbatim.quote("test"), "test");
    }

    #[test]
    fn test_quote_escaping() {
        assert_eq!(QuoteStyle::Single.quote("it's"), "'it''s'");
        assert_eq!(QuoteStyle::Double.quote("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn test_substitute_input() {
        let context = SubstitutionContext::new().with_input("name", "Alice");

        let result = substitute("Hello {{inputs.name}}!", &context).unwrap();
        assert_eq!(result, "Hello Alice!");
    }

    #[test]
    fn test_substitute_missing() {
        let context = SubstitutionContext::new();

        let result = substitute("Hello {{inputs.name}}!", &context);
        assert!(result.is_err());
    }

    #[test]
    fn test_substitute_step_array() {
        let results = vec![
            serde_json::json!({"id": "1", "name": "Alice"}),
            serde_json::json!({"id": "2", "name": "Bob"}),
        ];

        let context = SubstitutionContext::new().with_step_results("users", results);

        let result = substitute("WHERE id IN ({{users.*.id}})", &context).unwrap();
        assert_eq!(result, "WHERE id IN ('1', '2')");
    }
}
