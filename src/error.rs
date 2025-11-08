use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum KqlPanopticonError {
    #[error("Azure authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Failed to get Azure token: {0}")]
    TokenAcquisitionFailed(String),

    #[error("HTTP request failed: {0}")]
    HttpRequestFailed(String),

    #[error("Failed to parse response: {0}")]
    ParseFailed(String),

    #[error("Azure API error (status {status}): {message}")]
    AzureApiError { status: u16, message: String },

    #[error("Azure API rate limit exceeded. Retry after {retry_after} seconds")]
    RateLimitExceeded { retry_after: u64 },

    #[error("Workspace not found: {0}")]
    WorkspaceNotFound(String),

    #[error("Query execution failed: {0}")]
    QueryExecutionFailed(String),

    #[error("No subscriptions found")]
    NoSubscriptionsFound,

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Query pack validation failed: {0}")]
    QueryPackValidation(String),

    #[error("Query pack not found: {0}")]
    QueryPackNotFound(String),

    #[error("Home directory not found")]
    HomeDirectoryNotFound,

    #[error("{0}")]
    Other(String),
}

impl From<reqwest::Error> for KqlPanopticonError {
    fn from(err: reqwest::Error) -> Self {
        KqlPanopticonError::HttpRequestFailed(err.to_string())
    }
}

impl From<std::io::Error> for KqlPanopticonError {
    fn from(err: std::io::Error) -> Self {
        KqlPanopticonError::IoError(err.to_string())
    }
}

impl From<anyhow::Error> for KqlPanopticonError {
    fn from(err: anyhow::Error) -> Self {
        KqlPanopticonError::Other(err.to_string())
    }
}

impl From<serde_json::Error> for KqlPanopticonError {
    fn from(err: serde_json::Error) -> Self {
        KqlPanopticonError::ParseFailed(format!("JSON: {}", err))
    }
}

impl From<serde_yaml::Error> for KqlPanopticonError {
    fn from(err: serde_yaml::Error) -> Self {
        KqlPanopticonError::ParseFailed(format!("YAML: {}", err))
    }
}

pub type Result<T> = std::result::Result<T, KqlPanopticonError>;
