//! Safe Rust wrappers for KQL validation
//!
//! This module provides the high-level API for validating KQL queries.

use crate::error::Error;
use crate::ffi::{return_codes, DEFAULT_BUFFER_SIZE, MAX_BUFFER_SIZE};
use crate::loader::{self, LoadedLibrary};
use crate::schema::Schema;
use crate::types::ValidationResult;
use std::ffi::c_int;

/// KQL query validator
///
/// This is the main entry point for validating KQL queries. It manages
/// the connection to the native library and provides safe wrappers
/// around the FFI functions.
///
/// # Example
///
/// ```no_run
/// use kql_language_ffi::{KqlValidator, Schema, Table};
///
/// fn main() -> Result<(), kql_language_ffi::Error> {
///     let validator = KqlValidator::new()?;
///
///     // Syntax-only validation
///     let result = validator.validate_syntax("SecurityEvent | take 10")?;
///     assert!(result.is_valid());
///
///     // With schema
///     let schema = Schema::new()
///         .table(Table::new("SecurityEvent")
///             .with_column("TimeGenerated", "datetime")
///             .with_column("Account", "string"));
///     let result = validator.validate_with_schema(
///         "SecurityEvent | project TimeGenerated, Account",
///         &schema
///     )?;
///     Ok(())
/// }
/// ```
pub struct KqlValidator {
    lib: &'static LoadedLibrary,
}

impl KqlValidator {
    /// Create a new validator instance
    ///
    /// This loads the native library if not already loaded and
    /// initializes the KQL parser.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The native library cannot be found
    /// - The library fails to load
    /// - Initialization fails
    pub fn new() -> Result<Self, Error> {
        let lib = loader::load_library()?;
        Ok(Self { lib })
    }

    /// Validate a KQL query for syntax errors only
    ///
    /// This performs syntax-only validation without any schema awareness.
    /// It will catch basic syntax errors but won't validate table or
    /// column names.
    ///
    /// # Arguments
    ///
    /// * `query` - The KQL query string to validate
    ///
    /// # Returns
    ///
    /// A `ValidationResult` containing any diagnostics found.
    pub fn validate_syntax(&self, query: &str) -> Result<ValidationResult, Error> {
        let query_bytes = query.as_bytes();

        self.call_ffi_with_retry(|buffer| unsafe {
            (self.lib.validate_syntax)(
                query_bytes.as_ptr(),
                query_bytes.len() as c_int,
                buffer.as_mut_ptr(),
                buffer.len() as c_int,
            )
        })
    }

    /// Validate a KQL query with schema awareness
    ///
    /// This performs full semantic validation using the provided schema.
    /// It validates that referenced tables and columns exist and have
    /// the correct types.
    ///
    /// # Arguments
    ///
    /// * `query` - The KQL query string to validate
    /// * `schema` - The database schema to validate against
    ///
    /// # Returns
    ///
    /// A `ValidationResult` containing any diagnostics found.
    ///
    /// # Errors
    ///
    /// Returns an error if schema validation is not supported by the
    /// loaded library.
    pub fn validate_with_schema(&self, query: &str, schema: &Schema) -> Result<ValidationResult, Error> {
        let validate_fn = self.lib.validate_with_schema.ok_or_else(|| Error::Internal {
            message: "Schema validation not supported by loaded library".to_string(),
        })?;

        let query_bytes = query.as_bytes();
        let schema_json = serde_json::to_string(schema)?;
        let schema_bytes = schema_json.as_bytes();

        self.call_ffi_with_retry(|buffer| unsafe {
            validate_fn(
                query_bytes.as_ptr(),
                query_bytes.len() as c_int,
                schema_bytes.as_ptr(),
                schema_bytes.len() as c_int,
                buffer.as_mut_ptr(),
                buffer.len() as c_int,
            )
        })
    }

    /// Check if schema validation is supported
    pub fn supports_schema_validation(&self) -> bool {
        self.lib.supports_schema_validation()
    }

    /// Check if completion is supported
    pub fn supports_completion(&self) -> bool {
        self.lib.supports_completion()
    }

    /// Check if classification is supported
    pub fn supports_classification(&self) -> bool {
        self.lib.supports_classification()
    }

    /// Get syntax classifications for a KQL query (for syntax highlighting)
    ///
    /// Returns a list of classified spans that can be used to highlight
    /// different parts of the query (keywords, operators, identifiers, etc.)
    ///
    /// # Arguments
    ///
    /// * `query` - The KQL query string to classify
    ///
    /// # Returns
    ///
    /// A `ClassificationResult` containing spans with their classification kinds.
    ///
    /// # Errors
    ///
    /// Returns an error if classification is not supported by the loaded library.
    pub fn get_classifications(&self, query: &str) -> Result<crate::classification::ClassificationResult, Error> {
        let classify_fn = self.lib.get_classifications.ok_or_else(|| Error::Internal {
            message: "Classification not supported by loaded library".to_string(),
        })?;

        let query_bytes = query.as_bytes();

        self.call_ffi_json(|buffer| unsafe {
            classify_fn(
                query_bytes.as_ptr(),
                query_bytes.len() as c_int,
                buffer.as_mut_ptr(),
                buffer.len() as c_int,
            )
        })
    }

    /// Get completion suggestions at a cursor position
    ///
    /// Returns completion items (keywords, functions, tables, columns, etc.)
    /// that are valid at the given cursor position.
    ///
    /// # Arguments
    ///
    /// * `query` - The KQL query string
    /// * `cursor_position` - Cursor position (0-based character offset)
    /// * `schema` - Optional schema for context-aware completions
    ///
    /// # Returns
    ///
    /// A `CompletionResult` containing completion items.
    ///
    /// # Errors
    ///
    /// Returns an error if completion is not supported by the loaded library.
    pub fn get_completions(
        &self,
        query: &str,
        cursor_position: usize,
        schema: Option<&Schema>,
    ) -> Result<crate::completion::CompletionResult, Error> {
        let completions_fn = self.lib.get_completions.ok_or_else(|| Error::Internal {
            message: "Completion not supported by loaded library".to_string(),
        })?;

        let query_bytes = query.as_bytes();
        let schema_json = schema.map(serde_json::to_string).transpose()?;

        self.call_ffi_json(|buffer| unsafe {
            let (schema_ptr, schema_len) = match &schema_json {
                Some(json) => (json.as_ptr(), json.len() as c_int),
                None => (std::ptr::null(), 0),
            };

            completions_fn(
                query_bytes.as_ptr(),
                query_bytes.len() as c_int,
                cursor_position as c_int,
                schema_ptr,
                schema_len,
                buffer.as_mut_ptr(),
                buffer.len() as c_int,
            )
        })
    }

    /// Call an FFI function with automatic buffer retry on overflow
    fn call_ffi_with_retry<F>(&self, mut ffi_call: F) -> Result<ValidationResult, Error>
    where
        F: FnMut(&mut Vec<u8>) -> c_int,
    {
        let mut buffer = vec![0u8; DEFAULT_BUFFER_SIZE];
        let mut result = ffi_call(&mut buffer);

        // Handle buffer too small - retry with larger buffer
        if return_codes::is_buffer_too_small(result) {
            // Double the buffer size and retry
            let new_size = buffer.len() * 2;
            if new_size > MAX_BUFFER_SIZE {
                return Err(Error::BufferTooSmall {
                    needed: new_size,
                    available: MAX_BUFFER_SIZE,
                });
            }
            buffer.resize(new_size, 0);
            result = ffi_call(&mut buffer);

            // If still too small, give up
            if return_codes::is_buffer_too_small(result) {
                return Err(Error::BufferTooSmall {
                    needed: 0, // Unknown
                    available: buffer.len(),
                });
            }
        }

        // Check for other errors
        if !return_codes::is_success(result) {
            let error_msg = self.get_last_error().unwrap_or_default();
            return Err(Error::from_ffi_code(result, &error_msg));
        }

        // Parse JSON result
        if result == 0 {
            // Empty result means valid query
            return Ok(ValidationResult::valid());
        }

        let json_len = result as usize;
        let json_str = std::str::from_utf8(&buffer[..json_len])?;

        log::trace!("FFI returned JSON: {}", json_str);

        let validation_result: ValidationResult = serde_json::from_str(json_str)?;
        Ok(validation_result)
    }

    /// Call an FFI function and deserialize JSON result to a generic type
    fn call_ffi_json<T, F>(&self, mut ffi_call: F) -> Result<T, Error>
    where
        T: for<'de> serde::Deserialize<'de> + Default,
        F: FnMut(&mut Vec<u8>) -> c_int,
    {
        let mut buffer = vec![0u8; DEFAULT_BUFFER_SIZE];
        let mut result = ffi_call(&mut buffer);

        // Handle buffer too small - retry with larger buffer
        if return_codes::is_buffer_too_small(result) {
            let new_size = buffer.len() * 2;
            if new_size > MAX_BUFFER_SIZE {
                return Err(Error::BufferTooSmall {
                    needed: new_size,
                    available: MAX_BUFFER_SIZE,
                });
            }
            buffer.resize(new_size, 0);
            result = ffi_call(&mut buffer);

            if return_codes::is_buffer_too_small(result) {
                return Err(Error::BufferTooSmall {
                    needed: 0,
                    available: buffer.len(),
                });
            }
        }

        // Check for errors
        if !return_codes::is_success(result) {
            let error_msg = self.get_last_error().unwrap_or_default();
            return Err(Error::from_ffi_code(result, &error_msg));
        }

        // Parse JSON result
        if result == 0 {
            return Ok(T::default());
        }

        let json_len = result as usize;
        let json_str = std::str::from_utf8(&buffer[..json_len])?;

        log::trace!("FFI returned JSON: {}", json_str);

        let parsed_result: T = serde_json::from_str(json_str)?;
        Ok(parsed_result)
    }

    /// Get the last error message from the native library
    fn get_last_error(&self) -> Option<String> {
        let mut buffer = vec![0u8; 1024];
        let result = unsafe { (self.lib.get_last_error)(buffer.as_mut_ptr(), buffer.len() as c_int) };

        if return_codes::is_success(result) && result > 0 {
            let len = result as usize;
            String::from_utf8(buffer[..len].to_vec()).ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests require the native library to be available
    // They are ignored by default and can be run with:
    // cargo test --features test-native -- --ignored

    #[test]
    #[ignore]
    fn test_validate_syntax_valid() {
        let validator = KqlValidator::new().expect("Failed to create validator");
        let result = validator
            .validate_syntax("T | take 10")
            .expect("Validation failed");
        assert!(result.is_valid());
    }

    #[test]
    #[ignore]
    fn test_validate_syntax_invalid() {
        let validator = KqlValidator::new().expect("Failed to create validator");
        let result = validator
            .validate_syntax("T | invalid_operator")
            .expect("Validation failed");
        assert!(!result.is_valid());
        assert!(result.has_errors());
    }

    #[test]
    #[ignore]
    fn test_validate_with_schema() {
        let validator = KqlValidator::new().expect("Failed to create validator");

        let schema = Schema::new().table(
            crate::schema::Table::new("SecurityEvent")
                .with_column("TimeGenerated", "datetime")
                .with_column("Account", "string"),
        );

        let result = validator
            .validate_with_schema("SecurityEvent | project TimeGenerated, Account", &schema)
            .expect("Validation failed");
        assert!(result.is_valid());
    }

    #[test]
    #[ignore]
    fn test_validate_with_schema_unknown_column() {
        let validator = KqlValidator::new().expect("Failed to create validator");

        let schema = Schema::new().table(
            crate::schema::Table::new("SecurityEvent")
                .with_column("TimeGenerated", "datetime"),
        );

        let result = validator
            .validate_with_schema("SecurityEvent | project UnknownColumn", &schema)
            .expect("Validation failed");
        assert!(!result.is_valid());
    }

    #[test]
    #[ignore]
    fn test_get_classifications() {
        let validator = KqlValidator::new().expect("Failed to create validator");
        let result = validator
            .get_classifications("SecurityEvent | where TimeGenerated > ago(1h) | take 10")
            .expect("Classification failed");

        // Should have some spans
        assert!(!result.spans.is_empty(), "Expected classification spans");

        // Print spans for debugging
        for span in &result.spans {
            println!(
                "Span: start={}, length={}, kind={:?}",
                span.start, span.length, span.kind
            );
        }
    }

    #[test]
    #[ignore]
    fn test_get_completions_after_pipe() {
        let validator = KqlValidator::new().expect("Failed to create validator");

        // Get completions after the pipe operator
        let query = "SecurityEvent | ";
        let cursor_pos = query.len(); // cursor at end

        let result = validator
            .get_completions(query, cursor_pos, None)
            .expect("Completion failed");

        // Should have completion items (operators like where, project, etc.)
        assert!(!result.items.is_empty(), "Expected completion items");

        // Print items for debugging
        println!("Completions at position {} in '{}':", cursor_pos, query);
        for item in &result.items {
            println!(
                "  {} ({:?}) - edit_start: {}",
                item.label, item.kind, item.edit_start
            );
        }

        // Should include common operators
        let labels: Vec<_> = result.items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            labels.iter().any(|l| l.contains("where") || l.contains("project")),
            "Expected 'where' or 'project' in completions"
        );
    }

    #[test]
    #[ignore]
    fn test_get_completions_with_schema() {
        let validator = KqlValidator::new().expect("Failed to create validator");

        let schema = Schema::new().table(
            crate::schema::Table::new("SecurityEvent")
                .with_column("TimeGenerated", "datetime")
                .with_column("Account", "string")
                .with_column("Computer", "string"),
        );

        // Get completions after 'project ' - should include column names
        let query = "SecurityEvent | project ";
        let cursor_pos = query.len();

        let result = validator
            .get_completions(query, cursor_pos, Some(&schema))
            .expect("Completion failed");

        assert!(!result.items.is_empty(), "Expected completion items");

        // Print items for debugging
        println!(
            "Completions with schema at position {} in '{}':",
            cursor_pos, query
        );
        for item in &result.items {
            println!(
                "  {} ({:?}) - detail: {:?}",
                item.label, item.kind, item.detail
            );
        }
    }
}
