//! Result storage implementations

use super::{OutputFormat, ResultManifest};
use crate::error::Result;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Trait for result storage backends
#[async_trait]
pub trait ResultStore: Send + Sync {
    /// Store query results
    async fn store_results(
        &self,
        execution_id: &str,
        workspace: &str,
        step: &str,
        columns: &[String],
        rows: &[Vec<serde_json::Value>],
        format: OutputFormat,
    ) -> Result<PathBuf>;

    /// Store manifest
    async fn store_manifest(&self, manifest: &ResultManifest) -> Result<PathBuf>;

    /// Load manifest
    async fn load_manifest(&self, execution_id: &str) -> Result<ResultManifest>;

    /// List all execution IDs
    async fn list_executions(&self) -> Result<Vec<String>>;

    /// Delete an execution's results
    async fn delete_execution(&self, execution_id: &str) -> Result<()>;

    /// Get the base path for outputs
    fn output_base(&self) -> &Path;
}

/// File-system based result store
pub struct FileResultStore {
    base_path: PathBuf,
}

impl FileResultStore {
    /// Create a new file result store
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Get default output directory
    pub fn default_output_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".kql-panopticon")
            .join("output")
    }

    /// Ensure directory exists
    fn ensure_dir(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }
}

#[async_trait]
impl ResultStore for FileResultStore {
    async fn store_results(
        &self,
        execution_id: &str,
        workspace: &str,
        step: &str,
        columns: &[String],
        rows: &[Vec<serde_json::Value>],
        format: OutputFormat,
    ) -> Result<PathBuf> {
        let dir = super::build_output_path(&self.base_path, execution_id, workspace, step);
        self.ensure_dir(&dir)?;

        // Write JSON if enabled
        if format.includes_json() {
            let json_path = dir.join("results.json");
            let data = serde_json::json!({
                "columns": columns,
                "rows": rows,
                "row_count": rows.len(),
            });
            let json = serde_json::to_string_pretty(&data)?;
            tokio::fs::write(&json_path, json).await?;
        }

        // Write CSV if enabled
        if format.includes_csv() {
            let csv_path = dir.join("results.csv");
            let mut csv_content = String::new();

            // Header
            csv_content.push_str(&columns.join(","));
            csv_content.push('\n');

            // Rows
            for row in rows {
                let values: Vec<String> = row
                    .iter()
                    .map(|v| {
                        let s = match v {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Null => String::new(),
                            other => other.to_string(),
                        };
                        // Escape CSV
                        if s.contains(',') || s.contains('"') || s.contains('\n') {
                            format!("\"{}\"", s.replace('"', "\"\""))
                        } else {
                            s
                        }
                    })
                    .collect();
                csv_content.push_str(&values.join(","));
                csv_content.push('\n');
            }

            tokio::fs::write(&csv_path, csv_content).await?;
        }

        Ok(dir)
    }

    async fn store_manifest(&self, manifest: &ResultManifest) -> Result<PathBuf> {
        let dir = self.base_path.join(&manifest.execution_id);
        self.ensure_dir(&dir)?;

        let path = dir.join("manifest.json");
        manifest.save(&path)?;
        Ok(path)
    }

    async fn load_manifest(&self, execution_id: &str) -> Result<ResultManifest> {
        let path = self.base_path.join(execution_id).join("manifest.json");
        ResultManifest::load(&path)
    }

    async fn list_executions(&self) -> Result<Vec<String>> {
        let mut executions = Vec::new();

        if self.base_path.exists() {
            for entry in std::fs::read_dir(&self.base_path)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        executions.push(name.to_string());
                    }
                }
            }
        }

        Ok(executions)
    }

    async fn delete_execution(&self, execution_id: &str) -> Result<()> {
        let path = self.base_path.join(execution_id);
        if path.exists() {
            std::fs::remove_dir_all(&path)?;
        }
        Ok(())
    }

    fn output_base(&self) -> &Path {
        &self.base_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_file_result_store() {
        let temp_dir = std::env::temp_dir().join("kql-test-results");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let store = FileResultStore::new(&temp_dir);

        let columns = vec!["id".to_string(), "name".to_string()];
        let rows = vec![
            vec![serde_json::json!("1"), serde_json::json!("Alice")],
            vec![serde_json::json!("2"), serde_json::json!("Bob")],
        ];

        let path = store
            .store_results("exec-1", "workspace1", "step1", &columns, &rows, OutputFormat::Both)
            .await
            .unwrap();

        assert!(path.join("results.json").exists());
        assert!(path.join("results.csv").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
