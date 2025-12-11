//! Variable reference parsing
//!
//! Parses `{{...}}` variable references from strings.
//!
//! ## Supported Syntax
//!
//! - `{{inputs.name}}` - User-provided input value
//! - `{{secrets.name}}` - Environment variable secret
//! - `{{step.*.Column}}` - All values from a column (array)
//! - `{{step.first.Column}}` - First row's column value
//! - `{{step[N].Column}}` - Nth row's column value
//! - `{{alias.Column}}` - Current row in foreach iteration

use crate::error::{Error, Result};
use regex::Regex;
use std::sync::LazyLock;

/// Regex for matching variable references
static VAR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{([^}]+)\}\}").expect("Invalid regex")
});

/// Regex for index syntax like "step[0]"
static INDEX_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([a-zA-Z_][a-zA-Z0-9_]*)\[(\d+)\]$").expect("Invalid regex")
});

/// A parsed variable reference
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarRef {
    /// The full matched text including braces
    pub full_match: String,
    /// The inner content without braces
    pub inner: String,
    /// The parsed reference type
    pub ref_type: VarRefType,
}

/// Type of variable reference
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarRefType {
    /// User input: `{{inputs.name}}`
    Input { name: String },

    /// Secret from environment: `{{secrets.name}}`
    Secret { name: String },

    /// All values from column: `{{step.*.column}}`
    StepArray { step: String, column: String },

    /// First row value: `{{step.first.column}}`
    StepFirst { step: String, column: String },

    /// Indexed row: `{{step[N].column}}`
    StepIndex { step: String, index: usize, column: String },

    /// Foreach alias: `{{alias.column}}`
    Alias { alias: String, column: String },

    /// Unknown/unparseable reference
    Unknown { content: String },
}

impl VarRef {
    /// Parse all variable references from a string
    pub fn parse_all(input: &str) -> Vec<VarRef> {
        VAR_REGEX
            .captures_iter(input)
            .map(|cap| {
                let full_match = cap.get(0).unwrap().as_str().to_string();
                let inner = cap.get(1).unwrap().as_str().trim().to_string();
                let ref_type = Self::parse_ref_type(&inner);
                VarRef {
                    full_match,
                    inner,
                    ref_type,
                }
            })
            .collect()
    }

    /// Check if a string contains any variable references
    pub fn contains_vars(input: &str) -> bool {
        VAR_REGEX.is_match(input)
    }

    /// Parse the type of reference from the inner content
    fn parse_ref_type(inner: &str) -> VarRefType {
        let parts: Vec<&str> = inner.split('.').collect();

        match parts.as_slice() {
            // {{inputs.name}}
            ["inputs", name] => VarRefType::Input {
                name: (*name).to_string(),
            },

            // {{secrets.name}}
            ["secrets", name] => VarRefType::Secret {
                name: (*name).to_string(),
            },

            // {{step.*.column}}
            [step, "*", column] => VarRefType::StepArray {
                step: (*step).to_string(),
                column: (*column).to_string(),
            },

            // {{step.first.column}}
            [step, "first", column] => VarRefType::StepFirst {
                step: (*step).to_string(),
                column: (*column).to_string(),
            },

            // {{step_or_alias.column}} or {{step[N].column}}
            [step_or_alias, column] => {
                // Check for index syntax: step[N]
                if let Some((step, index)) = parse_index_syntax(step_or_alias) {
                    VarRefType::StepIndex {
                        step,
                        index,
                        column: (*column).to_string(),
                    }
                } else {
                    // Treat as alias reference (context-dependent at resolution time)
                    VarRefType::Alias {
                        alias: (*step_or_alias).to_string(),
                        column: (*column).to_string(),
                    }
                }
            }

            // Unknown format
            _ => VarRefType::Unknown {
                content: inner.to_string(),
            },
        }
    }

    /// Check if this is an array reference (returns multiple values)
    pub fn is_array(&self) -> bool {
        matches!(self.ref_type, VarRefType::StepArray { .. })
    }

    /// Get the step name this reference depends on (if any)
    pub fn step_dependency(&self) -> Option<&str> {
        match &self.ref_type {
            VarRefType::StepArray { step, .. } => Some(step),
            VarRefType::StepFirst { step, .. } => Some(step),
            VarRefType::StepIndex { step, .. } => Some(step),
            _ => None,
        }
    }

    /// Validate that this reference can be resolved
    pub fn validate(&self, available_steps: &[&str], available_inputs: &[&str]) -> Result<()> {
        match &self.ref_type {
            VarRefType::Input { name } => {
                if !available_inputs.contains(&name.as_str()) {
                    return Err(Error::variable(format!(
                        "Unknown input '{}'. Available: {:?}",
                        name, available_inputs
                    )));
                }
            }
            VarRefType::StepArray { step, .. }
            | VarRefType::StepFirst { step, .. }
            | VarRefType::StepIndex { step, .. } => {
                if !available_steps.contains(&step.as_str()) {
                    return Err(Error::variable(format!(
                        "Unknown step '{}'. Available: {:?}",
                        step, available_steps
                    )));
                }
            }
            VarRefType::Alias { alias, .. } => {
                // Aliases are validated at execution time when foreach context is known
                // Could also be a step reference - try both
                if !available_steps.contains(&alias.as_str()) {
                    // Not a known step - assume it's a foreach alias (validated at runtime)
                }
            }
            VarRefType::Secret { .. } => {
                // Secrets are validated at execution time against environment
            }
            VarRefType::Unknown { content } => {
                return Err(Error::variable(format!(
                    "Unknown variable syntax: '{}'",
                    content
                )));
            }
        }
        Ok(())
    }

    /// Generate a default placeholder value for validation
    pub fn default_placeholder(&self) -> String {
        match &self.ref_type {
            VarRefType::Input { name } => format!("placeholder_{}", name),
            VarRefType::Secret { name } => format!("secret_{}", name),
            VarRefType::StepArray { step, column } => {
                format!("'placeholder_{}_{}', 'placeholder_{}_{}_2'", step, column, step, column)
            }
            VarRefType::StepFirst { step, column } => {
                format!("placeholder_{}_{}", step, column)
            }
            VarRefType::StepIndex { step, index, column } => {
                format!("placeholder_{}_{}_{}", step, index, column)
            }
            VarRefType::Alias { alias, column } => {
                format!("placeholder_{}_{}", alias, column)
            }
            VarRefType::Unknown { content } => {
                format!("unknown_{}", content.replace('.', "_"))
            }
        }
    }
}

/// Parse index syntax like "step[0]" -> ("step", 0)
fn parse_index_syntax(s: &str) -> Option<(String, usize)> {
    INDEX_REGEX.captures(s).map(|cap| {
        let step = cap.get(1).unwrap().as_str().to_string();
        let index: usize = cap.get(2).unwrap().as_str().parse().unwrap();
        (step, index)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_input() {
        let refs = VarRef::parse_all("Hello {{inputs.username}}!");
        assert_eq!(refs.len(), 1);
        assert!(matches!(
            &refs[0].ref_type,
            VarRefType::Input { name } if name == "username"
        ));
    }

    #[test]
    fn test_parse_secret() {
        let refs = VarRef::parse_all("Token: {{secrets.api_key}}");
        assert_eq!(refs.len(), 1);
        assert!(matches!(
            &refs[0].ref_type,
            VarRefType::Secret { name } if name == "api_key"
        ));
    }

    #[test]
    fn test_parse_step_array() {
        let refs = VarRef::parse_all("WHERE id IN ({{users.*.UserId}})");
        assert_eq!(refs.len(), 1);
        assert!(matches!(
            &refs[0].ref_type,
            VarRefType::StepArray { step, column } if step == "users" && column == "UserId"
        ));
        assert!(refs[0].is_array());
    }

    #[test]
    fn test_parse_step_first() {
        let refs = VarRef::parse_all("{{results.first.Name}}");
        assert_eq!(refs.len(), 1);
        assert!(matches!(
            &refs[0].ref_type,
            VarRefType::StepFirst { step, column } if step == "results" && column == "Name"
        ));
        assert!(!refs[0].is_array());
    }

    #[test]
    fn test_parse_step_index() {
        let refs = VarRef::parse_all("{{users[0].Email}}");
        assert_eq!(refs.len(), 1);
        assert!(matches!(
            &refs[0].ref_type,
            VarRefType::StepIndex { step, index, column }
                if step == "users" && *index == 0 && column == "Email"
        ));
    }

    #[test]
    fn test_parse_alias() {
        let refs = VarRef::parse_all("WHERE user = '{{u.UserPrincipalName}}'");
        assert_eq!(refs.len(), 1);
        assert!(matches!(
            &refs[0].ref_type,
            VarRefType::Alias { alias, column } if alias == "u" && column == "UserPrincipalName"
        ));
    }

    #[test]
    fn test_parse_multiple() {
        let refs = VarRef::parse_all("{{inputs.x}} and {{step.*.y}} and {{secrets.key}}");
        assert_eq!(refs.len(), 3);
    }

    #[test]
    fn test_contains_vars() {
        assert!(VarRef::contains_vars("Hello {{name}}"));
        assert!(!VarRef::contains_vars("Hello world"));
    }

    #[test]
    fn test_step_dependency() {
        let refs = VarRef::parse_all("{{users.*.Id}}");
        assert_eq!(refs[0].step_dependency(), Some("users"));

        let refs = VarRef::parse_all("{{inputs.name}}");
        assert_eq!(refs[0].step_dependency(), None);
    }

    #[test]
    fn test_default_placeholder() {
        let refs = VarRef::parse_all("{{users.*.Email}}");
        let placeholder = refs[0].default_placeholder();
        assert!(placeholder.contains("placeholder"));
        assert!(placeholder.contains("users"));
    }
}
