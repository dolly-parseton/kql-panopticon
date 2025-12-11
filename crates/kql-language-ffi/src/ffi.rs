//! Raw FFI declarations for the native library
//!
//! This module contains the low-level FFI function declarations
//! that map to the C ABI exposed by the .NET AOT library.
//!
//! These functions should not be called directly - use the safe
//! wrappers in the `validator` module instead.

use std::ffi::c_int;

/// Type alias for FFI function pointers
pub type FfiResult = c_int;

/// FFI function type: Initialize the library
///
/// Returns 0 on success, negative error code on failure.
pub type KqlInitFn = unsafe extern "C" fn() -> FfiResult;

/// FFI function type: Cleanup the library
pub type KqlCleanupFn = unsafe extern "C" fn();

/// FFI function type: Validate KQL syntax
///
/// # Arguments
/// * `query` - Pointer to UTF-8 encoded query string
/// * `query_len` - Length of the query in bytes
/// * `output` - Pointer to output buffer for JSON result
/// * `output_max_len` - Maximum size of output buffer
///
/// # Returns
/// * `> 0` - Success, value is the length of JSON written to output
/// * `0` - Success, empty result
/// * `-1` - Buffer too small
/// * `-2` - Parse error in input
/// * `-3` - Internal error
pub type KqlValidateSyntaxFn =
    unsafe extern "C" fn(query: *const u8, query_len: c_int, output: *mut u8, output_max_len: c_int) -> FfiResult;

/// FFI function type: Validate KQL with schema
///
/// # Arguments
/// * `query` - Pointer to UTF-8 encoded query string
/// * `query_len` - Length of the query in bytes
/// * `schema_json` - Pointer to UTF-8 encoded JSON schema
/// * `schema_len` - Length of the schema JSON in bytes
/// * `output` - Pointer to output buffer for JSON result
/// * `output_max_len` - Maximum size of output buffer
///
/// # Returns
/// Same as `KqlValidateSyntaxFn`
pub type KqlValidateWithSchemaFn = unsafe extern "C" fn(
    query: *const u8,
    query_len: c_int,
    schema_json: *const u8,
    schema_len: c_int,
    output: *mut u8,
    output_max_len: c_int,
) -> FfiResult;

/// FFI function type: Get the last error message
///
/// # Arguments
/// * `output` - Pointer to output buffer for error message
/// * `output_max_len` - Maximum size of output buffer
///
/// # Returns
/// * `> 0` - Length of error message written
/// * `0` - No error message available
/// * `-1` - Buffer too small
pub type KqlGetLastErrorFn = unsafe extern "C" fn(output: *mut u8, output_max_len: c_int) -> FfiResult;

/// FFI function type: Get completions at cursor position
///
/// # Arguments
/// * `query` - Pointer to UTF-8 encoded query string
/// * `query_len` - Length of the query in bytes
/// * `cursor_pos` - Cursor position (0-based character offset)
/// * `schema_json` - Pointer to UTF-8 encoded JSON schema (can be null)
/// * `schema_len` - Length of the schema JSON in bytes (0 if null)
/// * `output` - Pointer to output buffer for JSON result
/// * `output_max_len` - Maximum size of output buffer
///
/// # Returns
/// Same as `KqlValidateSyntaxFn`
pub type KqlGetCompletionsFn = unsafe extern "C" fn(
    query: *const u8,
    query_len: c_int,
    cursor_pos: c_int,
    schema_json: *const u8,
    schema_len: c_int,
    output: *mut u8,
    output_max_len: c_int,
) -> FfiResult;

/// FFI function type: Get syntax classifications
///
/// # Arguments
/// * `query` - Pointer to UTF-8 encoded query string
/// * `query_len` - Length of the query in bytes
/// * `output` - Pointer to output buffer for JSON result
/// * `output_max_len` - Maximum size of output buffer
///
/// # Returns
/// Same as `KqlValidateSyntaxFn`
pub type KqlGetClassificationsFn =
    unsafe extern "C" fn(query: *const u8, query_len: c_int, output: *mut u8, output_max_len: c_int) -> FfiResult;

/// Symbol names in the native library
pub mod symbols {
    /// Initialize function symbol
    pub const KQL_INIT: &str = "kql_init";

    /// Cleanup function symbol
    pub const KQL_CLEANUP: &str = "kql_cleanup";

    /// Validate syntax function symbol
    pub const KQL_VALIDATE_SYNTAX: &str = "kql_validate_syntax";

    /// Validate with schema function symbol
    pub const KQL_VALIDATE_WITH_SCHEMA: &str = "kql_validate_with_schema";

    /// Get last error function symbol
    pub const KQL_GET_LAST_ERROR: &str = "kql_get_last_error";

    /// Get completions function symbol
    pub const KQL_GET_COMPLETIONS: &str = "kql_get_completions";

    /// Get classifications function symbol
    pub const KQL_GET_CLASSIFICATIONS: &str = "kql_get_classifications";
}

/// Return codes from FFI functions
pub mod return_codes {
    use std::ffi::c_int;

    /// Buffer too small - need to retry with larger buffer
    pub const BUFFER_TOO_SMALL: c_int = -1;

    /// Check if return code indicates success
    pub fn is_success(code: c_int) -> bool {
        code >= 0
    }

    /// Check if return code indicates buffer too small
    pub fn is_buffer_too_small(code: c_int) -> bool {
        code == BUFFER_TOO_SMALL
    }
}

/// Default buffer size for FFI output (64KB)
pub const DEFAULT_BUFFER_SIZE: usize = 64 * 1024;

/// Maximum buffer size for FFI output (4MB)
pub const MAX_BUFFER_SIZE: usize = 4 * 1024 * 1024;
