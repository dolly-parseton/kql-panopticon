//! KQL validation module (optional feature)
//!
//! This module provides KQL syntax and schema validation.
//! It is only compiled when the `kql-validation` feature is enabled.
//!
//! ## Implementation Options
//!
//! ### Option 1: .NET AOT FFI (Full Validation)
//!
//! Uses the official Microsoft.Azure.Kusto.Language library compiled to
//! a native library via .NET AOT. This provides:
//! - Full KQL syntax validation
//! - Schema-aware semantic validation
//! - Comprehensive error messages
//!
//! ### Option 2: rust-kql (Subset Validation)
//!
//! Uses the community rust-kql crate for basic syntax checking.
//! This provides:
//! - Basic KQL syntax validation
//! - No schema awareness
//! - Limited operator support
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kql_panopticon_core::validation::{KqlValidator, ValidationResult};
//!
//! let validator = KqlValidator::new()?;
//! let result = validator.validate("SecurityEvent | take 10")?;
//!
//! if result.is_valid() {
//!     println!("Query is valid");
//! } else {
//!     for error in result.errors() {
//!         println!("Error at line {}: {}", error.line, error.message);
//!     }
//! }
//! ```

use crate::error::Result;

/// KQL query validator
///
/// Validates KQL queries for syntax and optionally semantic correctness.
pub struct KqlValidator {
    // TODO: Add FFI handle to .NET AOT library
    // or rust-kql parser instance
    _placeholder: (),
}

impl KqlValidator {
    /// Create a new validator
    ///
    /// For .NET AOT: Loads the native library
    /// For rust-kql: Initializes the parser
    pub fn new() -> Result<Self> {
        // TODO: Initialize based on available backend
        todo!("Initialize KQL validator")
    }

    /// Validate a KQL query (syntax only)
    pub fn validate_syntax(&self, query: &str) -> Result<ValidationResult> {
        let _ = query;
        todo!("Implement syntax validation")
    }

    /// Validate with schema awareness
    ///
    /// Requires schema definition (table names, column types, etc.)
    pub fn validate_with_schema(
        &self,
        query: &str,
        schema: &Schema,
    ) -> Result<ValidationResult> {
        let _ = (query, schema);
        todo!("Implement schema-aware validation")
    }

    /// Parse query and return AST (for advanced use cases)
    pub fn parse(&self, query: &str) -> Result<ParsedQuery> {
        let _ = query;
        todo!("Implement query parsing")
    }
}

/// Result of query validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the query is valid
    pub valid: bool,
    /// Validation errors (if any)
    pub errors: Vec<ValidationError>,
    /// Warnings (valid but potentially problematic)
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    /// Check if validation passed
    pub fn is_valid(&self) -> bool {
        self.valid && self.errors.is_empty()
    }

    /// Get error messages
    pub fn errors(&self) -> &[ValidationError] {
        &self.errors
    }

    /// Get warning messages
    pub fn warnings(&self) -> &[ValidationWarning] {
        &self.warnings
    }
}

/// A validation error
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Error message
    pub message: String,
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based)
    pub column: usize,
    /// Length of the problematic span
    pub length: usize,
    /// Error code (if available)
    pub code: Option<String>,
}

/// A validation warning
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// Warning message
    pub message: String,
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based)
    pub column: usize,
}

/// Schema definition for semantic validation
#[derive(Debug, Clone, Default)]
pub struct Schema {
    /// Tables in the schema
    pub tables: Vec<Table>,
    /// Functions in the schema
    pub functions: Vec<Function>,
}

/// Table definition
#[derive(Debug, Clone)]
pub struct Table {
    /// Table name
    pub name: String,
    /// Columns
    pub columns: Vec<Column>,
}

/// Column definition
#[derive(Debug, Clone)]
pub struct Column {
    /// Column name
    pub name: String,
    /// KQL data type
    pub data_type: String,
}

/// Function definition
#[derive(Debug, Clone)]
pub struct Function {
    /// Function name
    pub name: String,
    /// Parameter types
    pub parameters: Vec<String>,
    /// Return type
    pub return_type: String,
}

/// Parsed query representation
#[derive(Debug, Clone)]
pub struct ParsedQuery {
    /// Referenced tables
    pub tables: Vec<String>,
    /// Referenced columns
    pub columns: Vec<String>,
    /// Operators used
    pub operators: Vec<String>,
    /// Time range (if specified)
    pub time_range: Option<String>,
}

// FFI types for .NET AOT integration
#[cfg(feature = "kql-validation")]
mod ffi {
    //! FFI bindings for .NET AOT KQL validator
    //!
    //! These bindings will be generated/implemented when the .NET AOT
    //! library is built and integrated.

    /// Initialize the validator library
    #[allow(dead_code)]
    extern "C" {
        fn kql_validator_init() -> i32;
        fn kql_validator_cleanup();
        fn kql_validate_syntax(
            query: *const u8,
            query_len: i32,
            errors: *mut u8,
            errors_len: i32,
        ) -> i32;
    }
}
