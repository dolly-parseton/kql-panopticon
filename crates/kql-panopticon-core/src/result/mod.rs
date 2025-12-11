//! Result storage and retrieval
//!
//! Handles storing and loading execution results.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub mod store;

pub use store::{FileResultStore, ResultStore};

/// Output format for results
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// JSON format
    #[default]
    Json,
    /// CSV format
    Csv,
    /// Both JSON and CSV
    Both,
}

impl OutputFormat {
    /// Get file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Both => "json", // Primary format
        }
    }

    /// Check if JSON output is enabled
    pub fn includes_json(&self) -> bool {
        matches!(self, Self::Json | Self::Both)
    }

    /// Check if CSV output is enabled
    pub fn includes_csv(&self) -> bool {
        matches!(self, Self::Csv | Self::Both)
    }
}

/// Manifest for an execution result set
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultManifest {
    /// Unique execution ID
    pub execution_id: String,
    /// Type of execution (query/investigation)
    pub execution_type: String,
    /// Name of the pack/query
    pub name: String,
    /// When execution started
    pub started_at: String,
    /// When execution completed
    pub completed_at: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Status
    pub status: String,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Workspaces queried
    pub workspaces: Vec<String>,
    /// Output paths
    pub outputs: Vec<OutputEntry>,
}

/// Entry in the outputs list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputEntry {
    /// Step or query name
    pub name: String,
    /// Workspace
    pub workspace: String,
    /// File path (relative to manifest)
    pub path: String,
    /// Format (json/csv)
    pub format: String,
    /// Row count
    pub rows: usize,
}

impl ResultManifest {
    /// Create a new manifest
    pub fn new(execution_id: impl Into<String>, execution_type: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            execution_id: execution_id.into(),
            execution_type: execution_type.into(),
            name: name.into(),
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            duration_ms: None,
            status: "running".to_string(),
            error: None,
            workspaces: Vec::new(),
            outputs: Vec::new(),
        }
    }

    /// Mark as completed successfully
    pub fn complete(&mut self, duration_ms: u64) {
        self.status = "success".to_string();
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.duration_ms = Some(duration_ms);
    }

    /// Mark as failed
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = "failed".to_string();
        self.error = Some(error.into());
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Add an output entry
    pub fn add_output(&mut self, entry: OutputEntry) {
        self.outputs.push(entry);
    }

    /// Save manifest to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load manifest from file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let manifest = serde_json::from_str(&content)?;
        Ok(manifest)
    }
}

/// Build output path structure
pub fn build_output_path(
    base: &Path,
    execution_id: &str,
    workspace: &str,
    step: &str,
) -> PathBuf {
    base.join(execution_id)
        .join(sanitize_path_component(workspace))
        .join(sanitize_path_component(step))
}

/// Sanitize a string for use in file paths
pub fn sanitize_path_component(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format() {
        assert!(OutputFormat::Json.includes_json());
        assert!(!OutputFormat::Json.includes_csv());
        assert!(OutputFormat::Both.includes_json());
        assert!(OutputFormat::Both.includes_csv());
    }

    #[test]
    fn test_sanitize_path() {
        assert_eq!(sanitize_path_component("Test Workspace"), "test_workspace");
        assert_eq!(sanitize_path_component("my-step_1"), "my-step_1");
    }

    #[test]
    fn test_build_output_path() {
        let path = build_output_path(
            Path::new("/output"),
            "exec-123",
            "My Workspace",
            "Step 1",
        );
        assert_eq!(
            path,
            PathBuf::from("/output/exec-123/my_workspace/step_1")
        );
    }
}
