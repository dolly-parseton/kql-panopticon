//! Query pack definitions
//!
//! Defines the structure of query packs - collections of KQL queries
//! to be executed in parallel.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A query pack containing multiple queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPack {
    /// Pack kind (must be "query")
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Pack name
    pub name: String,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
    /// Version
    #[serde(default)]
    pub version: Option<String>,
    /// Queries in this pack
    pub queries: Vec<Query>,
}

fn default_kind() -> String {
    "query".to_string()
}

/// A single query definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    /// Query name (optional, auto-generated if not provided)
    #[serde(default)]
    pub name: Option<String>,
    /// The KQL query
    pub query: String,
    /// Description of what this query does
    #[serde(default)]
    pub description: Option<String>,
    /// Timespan for the query (e.g., "P7D")
    #[serde(default)]
    pub timespan: Option<String>,
}

impl QueryPack {
    /// Load a query pack from a file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;

        let pack: QueryPack = if path.extension().map_or(false, |e| e == "json") {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };

        pack.validate()?;
        Ok(pack)
    }

    /// Validate the pack
    pub fn validate(&self) -> Result<()> {
        if self.kind != "query" {
            return Err(Error::pack(format!(
                "Invalid pack kind: '{}', expected 'query'",
                self.kind
            )));
        }

        if self.name.is_empty() {
            return Err(Error::pack("Pack name cannot be empty"));
        }

        if self.queries.is_empty() {
            return Err(Error::pack("Pack must contain at least one query"));
        }

        for (i, query) in self.queries.iter().enumerate() {
            if query.query.trim().is_empty() {
                return Err(Error::pack(format!("Query {} is empty", i + 1)));
            }
        }

        Ok(())
    }

    /// Get query names (auto-generating if needed)
    pub fn query_names(&self) -> Vec<String> {
        self.queries
            .iter()
            .enumerate()
            .map(|(i, q)| {
                q.name
                    .clone()
                    .unwrap_or_else(|| format!("query_{}", i + 1))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_queries() {
        let pack = QueryPack {
            kind: "query".to_string(),
            name: "test".to_string(),
            description: None,
            version: None,
            queries: vec![],
        };

        assert!(pack.validate().is_err());
    }

    #[test]
    fn test_validate_wrong_kind() {
        let pack = QueryPack {
            kind: "investigation".to_string(),
            name: "test".to_string(),
            description: None,
            version: None,
            queries: vec![Query {
                name: None,
                query: "test".to_string(),
                description: None,
                timespan: None,
            }],
        };

        assert!(pack.validate().is_err());
    }
}
