//! KQL Language FFI Bindings
//!
//! This crate provides Rust bindings to Microsoft's `Kusto.Language` library
//! via .NET AOT compilation. It enables KQL validation, completion, and
//! syntax highlighting in Rust applications.
//!
//! ## Features
//!
//! - **Syntax Validation**: Check KQL queries for syntax errors
//! - **Schema Validation**: Validate queries against a database schema
//! - **Completion** (planned): Get completion suggestions at cursor position
//! - **Classification** (planned): Get syntax highlighting tokens
//!
//! ## Usage
//!
//! ```no_run
//! use kql_language_ffi::{KqlValidator, ValidationResult};
//!
//! fn main() -> Result<(), kql_language_ffi::Error> {
//!     let validator = KqlValidator::new()?;
//!     let result = validator.validate_syntax("SecurityEvent | take 10")?;
//!
//!     if result.is_valid() {
//!         println!("Query is valid!");
//!     } else {
//!         for diagnostic in result.diagnostics() {
//!             println!("Error at {}:{}: {}", diagnostic.line, diagnostic.column, diagnostic.message);
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Native Library
//!
//! This crate requires a native library built from the .NET AOT project.
//! The library can be:
//!
//! 1. Built from source: `cd dotnet && dotnet publish -c Release -r <rid>`
//! 2. Downloaded from releases (if using `bundled` feature)
//! 3. Specified via `KQL_LANGUAGE_FFI_PATH` environment variable

mod classification;
mod completion;
mod error;
mod ffi;
mod loader;
mod schema;
mod types;
mod validator;

pub use classification::{ClassificationKind, ClassificationResult, ClassifiedSpan};
pub use completion::{CompletionItem, CompletionKind, CompletionResult};
pub use error::Error;
pub use schema::{Column, Function, Schema, Table};
pub use types::{Diagnostic, DiagnosticSeverity, ValidationResult};
pub use validator::KqlValidator;

/// Result type alias for this crate
pub type Result<T> = std::result::Result<T, Error>;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if the native library is available
///
/// Returns `true` if the native library can be loaded, `false` otherwise.
/// This is a lightweight check that doesn't fully initialize the library.
pub fn is_available() -> bool {
    loader::find_library_path().is_some()
}

/// Get the path to the native library, if found
pub fn library_path() -> Option<std::path::PathBuf> {
    loader::find_library_path()
}
