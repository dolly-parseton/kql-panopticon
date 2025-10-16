use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum KqlPanopticonError {
    #[error("Azure authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Failed to get Azure token: {0}")]
    TokenAcquisitionFailed(String),

    #[error("HTTP request failed: {0}")]
    HttpRequestFailed(String),

    #[error("Failed to parse JSON response: {0}")]
    JsonParseFailed(String),

    #[error("Azure API error (status {status}): {message}")]
    AzureApiError { status: u16, message: String },

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
        KqlPanopticonError::JsonParseFailed(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, KqlPanopticonError>;
