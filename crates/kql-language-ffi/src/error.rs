//! Error types for the KQL Language FFI crate

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur when using the KQL Language FFI
#[derive(Debug, Error)]
pub enum Error {
    /// The native library could not be found
    #[error("Native library not found. Searched paths: {searched_paths:?}. Set KQL_LANGUAGE_FFI_PATH to specify location.")]
    LibraryNotFound { searched_paths: Vec<PathBuf> },

    /// The native library failed to load
    #[error("Failed to load native library from {path}: {message}")]
    LibraryLoadFailed { path: PathBuf, message: String },

    /// A required symbol was not found in the library
    #[error("Symbol '{symbol}' not found in native library")]
    SymbolNotFound { symbol: String },

    /// The library initialization failed
    #[error("Library initialization failed: {message}")]
    InitializationFailed { message: String },

    /// FFI call returned an error code
    #[error("FFI call failed with code {code}: {message}")]
    FfiError { code: i32, message: String },

    /// Output buffer was too small
    #[error("Output buffer too small (needed {needed} bytes, had {available})")]
    BufferTooSmall { needed: usize, available: usize },

    /// JSON serialization/deserialization failed
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// UTF-8 conversion failed
    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// The library is not initialized
    #[error("Library not initialized. Call KqlValidator::new() first.")]
    NotInitialized,

    /// An internal error occurred
    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl Error {
    /// Create a library load failure error
    pub fn library_load_failed(path: impl Into<PathBuf>, err: impl std::fmt::Display) -> Self {
        Self::LibraryLoadFailed {
            path: path.into(),
            message: err.to_string(),
        }
    }

    /// Create an FFI error from a return code
    pub fn from_ffi_code(code: i32, context: &str) -> Self {
        let message = match code {
            -1 => "Buffer too small".to_string(),
            -2 => "Parse error in input".to_string(),
            -3 => "Internal error".to_string(),
            _ => format!("Unknown error code: {}", code),
        };
        Self::FfiError {
            code,
            message: format!("{}: {}", context, message),
        }
    }
}

/// FFI error codes returned by the native library
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum FfiErrorCode {
    /// Success (or positive value indicating output length)
    Success = 0,
    /// Output buffer too small
    BufferTooSmall = -1,
    /// Parse error in input
    ParseError = -2,
    /// Internal error
    InternalError = -3,
}

impl FfiErrorCode {
    /// Check if a return code indicates success
    pub fn is_success(code: i32) -> bool {
        code >= 0
    }

    /// Convert a return code to an error code
    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            -1 => Some(Self::BufferTooSmall),
            -2 => Some(Self::ParseError),
            -3 => Some(Self::InternalError),
            _ if code >= 0 => Some(Self::Success),
            _ => None,
        }
    }
}
