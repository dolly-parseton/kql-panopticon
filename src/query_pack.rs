use serde::{Deserialize, Serialize};
use crate::query_job::QuerySettings;
use crate::error::Result;
use std::path::{Path, PathBuf};

/// A query pack containing one or more KQL queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPack {
    /// Pack name
    pub name: String,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Optional author
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Optional version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Single query (for simple packs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,

    /// Multiple queries (for complex packs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queries: Option<Vec<PackQuery>>,

    /// Execution settings (optional - uses defaults if omitted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<QuerySettings>,

    /// Workspace scope (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspaces: Option<WorkspaceScope>,
}

/// A single query within a pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackQuery {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub query: String,
}

/// Workspace selection scope
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "lowercase")]
pub enum WorkspaceScope {
    /// Execute on all available workspaces
    All,

    /// Execute on specific workspace IDs
    Selected { ids: Vec<String> },

    /// Execute on workspaces matching pattern
    Pattern { pattern: String },
}

impl QueryPack {
    /// Load a query pack from a file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;

        // Try YAML first, fall back to JSON
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            Ok(serde_json::from_str(&content)?)
        } else {
            // Default to YAML for .yaml, .yml, or no extension
            Ok(serde_yaml::from_str(&content)?)
        }
    }

    /// Save a query pack to a file
    #[allow(dead_code)]
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let content = if path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::to_string_pretty(self)?
        } else {
            serde_yaml::to_string(self)?
        };

        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get all queries from the pack (handles both single and multiple query formats)
    pub fn get_queries(&self) -> Vec<PackQuery> {
        if let Some(queries) = &self.queries {
            queries.clone()
        } else if let Some(query) = &self.query {
            vec![PackQuery {
                name: self.name.clone(),
                description: self.description.clone(),
                query: query.clone(),
            }]
        } else {
            vec![]
        }
    }

    /// Validate the query pack
    pub fn validate(&self) -> Result<()> {
        // Must have either query or queries
        if self.query.is_none() && self.queries.is_none() {
            return Err(crate::error::KqlPanopticonError::QueryPackValidation(
                "Query pack must contain either 'query' or 'queries' field".into()
            ));
        }

        // Can't have both
        if self.query.is_some() && self.queries.is_some() {
            return Err(crate::error::KqlPanopticonError::QueryPackValidation(
                "Query pack cannot have both 'query' and 'queries' fields".into()
            ));
        }

        // If queries array, must not be empty
        if let Some(queries) = &self.queries {
            if queries.is_empty() {
                return Err(crate::error::KqlPanopticonError::QueryPackValidation(
                    "Query pack 'queries' array cannot be empty".into()
                ));
            }
        }

        Ok(())
    }

    /// Get the pack's file path in the standard library location
    pub fn get_library_path(relative_path: &str) -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or(crate::error::KqlPanopticonError::HomeDirectoryNotFound)?;

        Ok(home.join(".kql-panopticon/packs").join(relative_path))
    }

    /// List all query packs in the library
    pub fn list_library_packs() -> Result<Vec<PathBuf>> {
        let packs_dir = Self::get_library_path("")?;

        if !packs_dir.exists() {
            std::fs::create_dir_all(&packs_dir)?;
            return Ok(vec![]);
        }

        let mut packs = Vec::new();

        // Recursively find all .yaml, .yml, .json files
        for entry in walkdir::WalkDir::new(&packs_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                    if ext == "yaml" || ext == "yml" || ext == "json" {
                        packs.push(entry.path().to_path_buf());
                    }
                }
            }
        }

        Ok(packs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_minimal_pack() {
        let yaml = r#"
name: "Test Query"
query: "SecurityEvent | limit 10"
"#;
        let pack: QueryPack = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(pack.name, "Test Query");
        assert_eq!(pack.get_queries().len(), 1);
    }

    #[test]
    fn test_load_full_pack() {
        let yaml = r#"
name: "Security Hunt"
description: "Multi-query investigation"
queries:
  - name: "Query 1"
    query: "SecurityEvent | limit 5"
  - name: "Query 2"
    query: "SigninLogs | limit 5"
settings:
  timeout: 60
workspaces:
  scope: all
"#;
        let pack: QueryPack = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(pack.get_queries().len(), 2);
        pack.validate().unwrap();
    }

    #[test]
    fn test_validate_empty_pack() {
        let pack = QueryPack {
            name: "Test".into(),
            description: None,
            author: None,
            version: None,
            query: None,
            queries: None,
            settings: None,
            workspaces: None,
        };
        assert!(pack.validate().is_err());
    }

    #[test]
    fn test_validate_both_query_and_queries() {
        let pack = QueryPack {
            name: "Test".into(),
            description: None,
            author: None,
            version: None,
            query: Some("SecurityEvent".into()),
            queries: Some(vec![PackQuery {
                name: "Q1".into(),
                description: None,
                query: "SigninLogs".into(),
            }]),
            settings: None,
            workspaces: None,
        };
        assert!(pack.validate().is_err());
    }
}
