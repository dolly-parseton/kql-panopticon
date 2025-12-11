//! KQL validation module
//!
//! This module provides KQL syntax and schema validation using the
//! Microsoft.Azure.Kusto.Language library via FFI bindings.
//!
//! ## Features
//!
//! - **Syntax Validation**: Validates KQL query syntax without schema context
//! - **Schema-Aware Validation**: Validates queries against table/column definitions
//! - **Detailed Diagnostics**: Line/column positions, error codes, severity levels
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kql_panopticon_core::validation::{KqlValidator, Schema, Table, Column};
//!
//! // Create validator (loads native library)
//! let validator = KqlValidator::new()?;
//!
//! // Syntax-only validation
//! let result = validator.validate_syntax("SecurityEvent | take 10")?;
//! if result.is_valid() {
//!     println!("Query syntax is valid");
//! }
//!
//! // Schema-aware validation
//! let schema = Schema {
//!     tables: vec![
//!         Table {
//!             name: "SecurityEvent".to_string(),
//!             columns: vec![
//!                 Column { name: "TimeGenerated".to_string(), data_type: "datetime".to_string() },
//!                 Column { name: "Computer".to_string(), data_type: "string".to_string() },
//!             ],
//!         },
//!     ],
//!     ..Default::default()
//! };
//!
//! let result = validator.validate_with_schema("SecurityEvent | where Computer == 'srv01'", &schema)?;
//! for diagnostic in result.diagnostics() {
//!     println!("[{}] {}", diagnostic.severity, diagnostic.message);
//! }
//! ```

use crate::error::{Error, Result};

// Re-export types from kql-language-ffi for convenience
pub use kql_language_ffi::{Column, Diagnostic, DiagnosticSeverity, Function, Schema, Table, ValidationResult};

/// KQL query validator
///
/// Wraps the FFI bindings to Microsoft's Kusto.Language library.
/// The validator is created once and can be reused for multiple queries.
pub struct KqlValidator {
    inner: kql_language_ffi::KqlValidator,
}

impl KqlValidator {
    /// Create a new validator
    ///
    /// This loads the native library and initializes the Kusto parser.
    /// The library is loaded once per process and cached.
    pub fn new() -> Result<Self> {
        let inner = kql_language_ffi::KqlValidator::new().map_err(|e| Error::Validation {
            message: format!("Failed to initialize KQL validator: {}", e),
            line: None,
            column: None,
        })?;

        Ok(Self { inner })
    }

    /// Validate a KQL query (syntax only)
    ///
    /// Checks the query for syntax errors without any schema context.
    /// This is faster than schema-aware validation but won't catch
    /// semantic errors like unknown table or column names.
    pub fn validate_syntax(&self, query: &str) -> Result<ValidationResult> {
        self.inner.validate_syntax(query).map_err(|e| Error::Validation {
            message: format!("Syntax validation failed: {}", e),
            line: None,
            column: None,
        })
    }

    /// Validate with schema awareness
    ///
    /// Validates the query against the provided schema, catching errors like:
    /// - Unknown table names
    /// - Unknown column names
    /// - Type mismatches in comparisons
    /// - Invalid function arguments
    pub fn validate_with_schema(&self, query: &str, schema: &Schema) -> Result<ValidationResult> {
        self.inner
            .validate_with_schema(query, schema)
            .map_err(|e| Error::Validation {
                message: format!("Schema validation failed: {}", e),
                line: None,
                column: None,
            })
    }

    /// Check if schema validation is supported
    ///
    /// Returns true if the native library supports schema-aware validation.
    pub fn supports_schema_validation(&self) -> bool {
        self.inner.supports_schema_validation()
    }

    /// Check if completion is supported (Phase 2)
    ///
    /// Returns true if the native library supports code completion.
    pub fn supports_completion(&self) -> bool {
        self.inner.supports_completion()
    }

    /// Check if classification is supported (Phase 3)
    ///
    /// Returns true if the native library supports syntax classification.
    pub fn supports_classification(&self) -> bool {
        self.inner.supports_classification()
    }
}

/// Extension trait for ValidationResult convenience methods
pub trait ValidationResultExt {
    /// Check if the query is valid (no errors)
    fn is_valid(&self) -> bool;

    /// Get all diagnostics
    fn diagnostics(&self) -> &[Diagnostic];

    /// Get only error-level diagnostics
    fn errors(&self) -> Vec<&Diagnostic>;

    /// Get only warning-level diagnostics
    fn warnings(&self) -> Vec<&Diagnostic>;
}

impl ValidationResultExt for ValidationResult {
    fn is_valid(&self) -> bool {
        self.valid
    }

    fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    fn errors(&self) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .collect()
    }

    fn warnings(&self) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require the native library to be built and available.
    // Run `./build.sh` in crates/kql-language-ffi/dotnet first.

    #[test]
    #[ignore = "requires native library"]
    fn test_validator_creation() {
        let validator = KqlValidator::new();
        assert!(validator.is_ok(), "Failed to create validator: {:?}", validator.err());
    }

    #[test]
    #[ignore = "requires native library"]
    fn test_valid_syntax() {
        let validator = KqlValidator::new().unwrap();
        let result = validator.validate_syntax("SecurityEvent | take 10").unwrap();
        assert!(result.is_valid());
        assert!(result.diagnostics().is_empty());
    }

    #[test]
    #[ignore = "requires native library"]
    fn test_invalid_syntax() {
        let validator = KqlValidator::new().unwrap();
        let result = validator.validate_syntax("SecurityEvent | take").unwrap();
        assert!(!result.is_valid());
        assert!(!result.diagnostics().is_empty());
    }

    #[test]
    #[ignore = "requires native library"]
    fn test_schema_validation() {
        let validator = KqlValidator::new().unwrap();

        let schema = Schema {
            tables: vec![Table {
                name: "SecurityEvent".to_string(),
                columns: vec![
                    Column {
                        name: "TimeGenerated".to_string(),
                        data_type: "datetime".to_string(),
                    },
                    Column {
                        name: "Computer".to_string(),
                        data_type: "string".to_string(),
                    },
                ],
            }],
            functions: vec![],
        };

        let result = validator
            .validate_with_schema("SecurityEvent | where Computer == 'srv01'", &schema)
            .unwrap();
        assert!(result.is_valid());
    }
}
