//! Error types for kql-panopticon-core
//!
//! Provides a unified error hierarchy for all operations in the library.

use thiserror::Error;

/// Main error type for the library
#[derive(Error, Debug, Clone)]
pub enum Error {
    /// Azure authentication or API errors
    #[error("Azure error: {message}")]
    Azure {
        message: String,
        #[source]
        source: Option<Box<Error>>,
    },

    /// Query execution errors
    #[error("Query error: {message}")]
    Query {
        message: String,
        workspace: Option<String>,
        query: Option<String>,
    },

    /// Investigation execution errors
    #[error("Investigation error in step '{step}': {message}")]
    Investigation {
        message: String,
        step: String,
        workspace: Option<String>,
    },

    /// Variable substitution errors
    #[error("Variable error: {message}")]
    Variable {
        message: String,
        variable: Option<String>,
    },

    /// Pack loading or validation errors
    #[error("Pack error: {message}")]
    Pack {
        message: String,
        path: Option<String>,
    },

    /// HTTP step errors
    #[error("HTTP error: {message}")]
    Http {
        message: String,
        url: Option<String>,
        status: Option<u16>,
    },

    /// Result storage errors
    #[error("Storage error: {message}")]
    Storage {
        message: String,
        path: Option<String>,
    },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Config { message: String },

    /// IO errors
    #[error("IO error: {message}")]
    Io { message: String },

    /// Validation errors (when kql-validation feature is enabled)
    #[error("Validation error: {message}")]
    Validation {
        message: String,
        line: Option<usize>,
        column: Option<usize>,
    },

    /// Timeout errors
    #[error("Timeout: {message}")]
    Timeout { message: String },

    /// Generic/wrapped errors
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Create an Azure error
    pub fn azure(message: impl Into<String>) -> Self {
        Self::Azure {
            message: message.into(),
            source: None,
        }
    }

    /// Create a query error
    pub fn query(message: impl Into<String>) -> Self {
        Self::Query {
            message: message.into(),
            workspace: None,
            query: None,
        }
    }

    /// Create a query error with context
    pub fn query_with_context(
        message: impl Into<String>,
        workspace: impl Into<String>,
        query: impl Into<String>,
    ) -> Self {
        Self::Query {
            message: message.into(),
            workspace: Some(workspace.into()),
            query: Some(query.into()),
        }
    }

    /// Create an investigation error
    pub fn investigation(message: impl Into<String>, step: impl Into<String>) -> Self {
        Self::Investigation {
            message: message.into(),
            step: step.into(),
            workspace: None,
        }
    }

    /// Create a variable error
    pub fn variable(message: impl Into<String>) -> Self {
        Self::Variable {
            message: message.into(),
            variable: None,
        }
    }

    /// Create a pack error
    pub fn pack(message: impl Into<String>) -> Self {
        Self::Pack {
            message: message.into(),
            path: None,
        }
    }

    /// Create an HTTP error
    pub fn http(message: impl Into<String>) -> Self {
        Self::Http {
            message: message.into(),
            url: None,
            status: None,
        }
    }

    /// Create a storage error
    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage {
            message: message.into(),
            path: None,
        }
    }

    /// Create a config error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create an IO error
    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::Timeout {
            message: message.into(),
        }
    }

    /// Create a generic/other error
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }
}

/// Convenience Result type using our Error
pub type Result<T> = std::result::Result<T, Error>;

// Conversion implementations for common error types

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            message: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Pack {
            message: format!("JSON parse error: {}", err),
            path: None,
        }
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Self {
        Self::Pack {
            message: format!("YAML parse error: {}", err),
            path: None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Self::Http {
            message: err.to_string(),
            url: err.url().map(|u| u.to_string()),
            status: err.status().map(|s| s.as_u16()),
        }
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Self {
        Self::Variable {
            message: format!("Regex error: {}", err),
            variable: None,
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::query("Test error");
        assert_eq!(err.to_string(), "Query error: Test error");
    }

    #[test]
    fn test_error_with_context() {
        let err = Error::query_with_context("Failed", "workspace1", "SELECT *");
        match err {
            Error::Query {
                workspace, query, ..
            } => {
                assert_eq!(workspace, Some("workspace1".to_string()));
                assert_eq!(query, Some("SELECT *".to_string()));
            }
            _ => panic!("Wrong error type"),
        }
    }
}
