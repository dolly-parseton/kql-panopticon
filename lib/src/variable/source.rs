//! Variable source types for pipe-based substitution syntax.
//!
//! Sources represent the origin of data in `{{...}}` references:
//! - `{{inputs.name}}` - User-provided input values
//! - `{{secrets.name}}` - Environment variable secrets
//! - `{{step}}` - Step result (for step-level transforms)
//! - `{{step.column}}` - Column from step result

use crate::error::{Error, Result};

/// A source reference in the `{{...}}` syntax.
///
/// This represents the data source before any transforms are applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// User input: `{{inputs.name}}`
    Input { name: String },

    /// Secret from environment: `{{secrets.name}}`
    Secret { name: String },

    /// Step-level reference: `{{step}}`
    ///
    /// Used with step-level transforms like `is_empty`, `length`, `any()`, `all()`.
    Step { name: String },

    /// Step column reference: `{{step.column}}`
    ///
    /// Used with column transforms like `first`, `array`, `for_each`.
    StepColumn { step: String, column: String },
}

impl Source {
    /// Parse a source from the part before the first pipe.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// Source::parse("inputs.user") // Input { name: "user" }
    /// Source::parse("secrets.api_key") // Secret { name: "api_key" }
    /// Source::parse("step") // Step { name: "step" }
    /// Source::parse("step.column") // StepColumn { step: "step", column: "column" }
    /// ```
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();

        if s.is_empty() {
            return Err(Error::variable("Empty source reference"));
        }

        // Check for inputs.name
        if let Some(name) = s.strip_prefix("inputs.") {
            let name = name.trim();
            if name.is_empty() {
                return Err(Error::variable("Input name cannot be empty"));
            }
            if !is_valid_identifier(name) {
                return Err(Error::variable(format!(
                    "Invalid input name '{}': must be alphanumeric with underscores",
                    name
                )));
            }
            return Ok(Source::Input {
                name: name.to_string(),
            });
        }

        // Check for secrets.name
        if let Some(name) = s.strip_prefix("secrets.") {
            let name = name.trim();
            if name.is_empty() {
                return Err(Error::variable("Secret name cannot be empty"));
            }
            if !is_valid_identifier(name) {
                return Err(Error::variable(format!(
                    "Invalid secret name '{}': must be alphanumeric with underscores",
                    name
                )));
            }
            return Ok(Source::Secret {
                name: name.to_string(),
            });
        }

        // Check for step.column or just step
        if let Some(dot_pos) = s.find('.') {
            let step = &s[..dot_pos];
            let column = &s[dot_pos + 1..];

            if step.is_empty() {
                return Err(Error::variable("Step name cannot be empty"));
            }
            if !is_valid_identifier(step) {
                return Err(Error::variable(format!(
                    "Invalid step name '{}': must be alphanumeric with underscores",
                    step
                )));
            }

            if column.is_empty() {
                return Err(Error::variable(format!(
                    "Column name cannot be empty in '{}'",
                    s
                )));
            }
            if !is_valid_identifier(column) {
                return Err(Error::variable(format!(
                    "Invalid column name '{}': must be alphanumeric with underscores",
                    column
                )));
            }

            Ok(Source::StepColumn {
                step: step.to_string(),
                column: column.to_string(),
            })
        } else {
            // Just a step name
            if !is_valid_identifier(s) {
                return Err(Error::variable(format!(
                    "Invalid step name '{}': must be alphanumeric with underscores",
                    s
                )));
            }
            Ok(Source::Step {
                name: s.to_string(),
            })
        }
    }

    /// Get the step dependency, if any.
    ///
    /// Returns `None` for inputs and secrets.
    pub fn step_dependency(&self) -> Option<&str> {
        match self {
            Source::Input { .. } | Source::Secret { .. } => None,
            Source::Step { name } => Some(name),
            Source::StepColumn { step, .. } => Some(step),
        }
    }

    /// Check if this source is an input.
    pub fn is_input(&self) -> bool {
        matches!(self, Source::Input { .. })
    }

    /// Check if this source is a secret.
    pub fn is_secret(&self) -> bool {
        matches!(self, Source::Secret { .. })
    }

    /// Check if this source is a step (with or without column).
    pub fn is_step(&self) -> bool {
        matches!(self, Source::Step { .. } | Source::StepColumn { .. })
    }

    /// Check if this source has a column selected.
    pub fn has_column(&self) -> bool {
        matches!(self, Source::StepColumn { .. })
    }

    /// Get the input name if this is an input source.
    pub fn input_name(&self) -> Option<&str> {
        match self {
            Source::Input { name } => Some(name),
            _ => None,
        }
    }

    /// Get the secret name if this is a secret source.
    pub fn secret_name(&self) -> Option<&str> {
        match self {
            Source::Secret { name } => Some(name),
            _ => None,
        }
    }
}

/// Check if a string is a valid identifier (alphanumeric with underscores).
fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let first_char = s.chars().next().unwrap();
    if !first_char.is_ascii_alphabetic() && first_char != '_' {
        return false;
    }

    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_input() {
        let source = Source::parse("inputs.user").unwrap();
        assert_eq!(
            source,
            Source::Input {
                name: "user".to_string()
            }
        );

        let source = Source::parse("inputs.target_user").unwrap();
        assert_eq!(
            source,
            Source::Input {
                name: "target_user".to_string()
            }
        );
    }

    #[test]
    fn test_parse_secret() {
        let source = Source::parse("secrets.api_key").unwrap();
        assert_eq!(
            source,
            Source::Secret {
                name: "api_key".to_string()
            }
        );
    }

    #[test]
    fn test_parse_step() {
        let source = Source::parse("suspicious_ips").unwrap();
        assert_eq!(
            source,
            Source::Step {
                name: "suspicious_ips".to_string()
            }
        );
    }

    #[test]
    fn test_parse_step_column() {
        let source = Source::parse("suspicious_ips.IPAddress").unwrap();
        assert_eq!(
            source,
            Source::StepColumn {
                step: "suspicious_ips".to_string(),
                column: "IPAddress".to_string()
            }
        );
    }

    #[test]
    fn test_parse_with_whitespace() {
        let source = Source::parse("  inputs.user  ").unwrap();
        assert_eq!(
            source,
            Source::Input {
                name: "user".to_string()
            }
        );
    }

    #[test]
    fn test_parse_empty_error() {
        assert!(Source::parse("").is_err());
        assert!(Source::parse("   ").is_err());
    }

    #[test]
    fn test_parse_invalid_input_name() {
        assert!(Source::parse("inputs.").is_err());
        assert!(Source::parse("inputs.123invalid").is_err());
    }

    #[test]
    fn test_parse_invalid_step_column() {
        assert!(Source::parse("step.").is_err());
        assert!(Source::parse(".column").is_err());
    }

    #[test]
    fn test_step_dependency() {
        let input = Source::parse("inputs.user").unwrap();
        assert_eq!(input.step_dependency(), None);

        let secret = Source::parse("secrets.key").unwrap();
        assert_eq!(secret.step_dependency(), None);

        let step = Source::parse("my_step").unwrap();
        assert_eq!(step.step_dependency(), Some("my_step"));

        let step_col = Source::parse("my_step.column").unwrap();
        assert_eq!(step_col.step_dependency(), Some("my_step"));
    }

    #[test]
    fn test_source_type_checks() {
        let input = Source::parse("inputs.user").unwrap();
        assert!(input.is_input());
        assert!(!input.is_secret());
        assert!(!input.is_step());
        assert!(!input.has_column());

        let secret = Source::parse("secrets.key").unwrap();
        assert!(!secret.is_input());
        assert!(secret.is_secret());
        assert!(!secret.is_step());

        let step = Source::parse("my_step").unwrap();
        assert!(step.is_step());
        assert!(!step.has_column());

        let step_col = Source::parse("my_step.column").unwrap();
        assert!(step_col.is_step());
        assert!(step_col.has_column());
    }

    #[test]
    fn test_valid_identifiers_with_hyphens() {
        // Allow hyphens in step/column names (common in pack definitions)
        let source = Source::parse("my-step.my-column").unwrap();
        assert_eq!(
            source,
            Source::StepColumn {
                step: "my-step".to_string(),
                column: "my-column".to_string()
            }
        );
    }
}
