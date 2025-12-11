//! Schema type definitions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Schema classification for tables
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaType {
    /// Fixed schema, same across all workspaces (e.g., SecurityEvent, SigninLogs)
    Canonical,
    /// Base schema with workspace-specific extensions (e.g., AzureDiagnostics, Syslog)
    Extensible,
    /// Entirely workspace-specific, no shared schema (e.g., *_CL custom logs)
    Custom,
}

impl Default for SchemaType {
    fn default() -> Self {
        Self::Canonical
    }
}

/// Column definition for schema registry
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnDef {
    /// Column name
    pub name: String,
    /// KQL data type (string, long, datetime, dynamic, etc.)
    pub data_type: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl ColumnDef {
    /// Create a new column definition
    pub fn new(name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into(),
            description: None,
        }
    }

    /// Create with description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Convenience constructors for common types
    pub fn string(name: impl Into<String>) -> Self {
        Self::new(name, "string")
    }

    pub fn long(name: impl Into<String>) -> Self {
        Self::new(name, "long")
    }

    pub fn datetime(name: impl Into<String>) -> Self {
        Self::new(name, "datetime")
    }

    pub fn dynamic(name: impl Into<String>) -> Self {
        Self::new(name, "dynamic")
    }

    pub fn bool(name: impl Into<String>) -> Self {
        Self::new(name, "bool")
    }

    pub fn real(name: impl Into<String>) -> Self {
        Self::new(name, "real")
    }

    pub fn guid(name: impl Into<String>) -> Self {
        Self::new(name, "guid")
    }
}

/// Table information in the registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    /// Table name
    pub name: String,
    /// Schema type classification
    #[serde(default)]
    pub schema_type: SchemaType,
    /// Column definitions (base columns for extensible tables)
    pub columns: Vec<ColumnDef>,
    /// Optional table description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Source of this schema (sentinel, defender, custom, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl TableInfo {
    /// Create a new canonical table
    pub fn canonical(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            schema_type: SchemaType::Canonical,
            columns: Vec::new(),
            description: None,
            source: None,
        }
    }

    /// Create a new extensible table
    pub fn extensible(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            schema_type: SchemaType::Extensible,
            columns: Vec::new(),
            description: None,
            source: None,
        }
    }

    /// Create a new custom table
    pub fn custom(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            schema_type: SchemaType::Custom,
            columns: Vec::new(),
            description: None,
            source: None,
        }
    }

    /// Add a column
    pub fn column(mut self, col: ColumnDef) -> Self {
        self.columns.push(col);
        self
    }

    /// Add a column by name and type
    pub fn with_column(mut self, name: impl Into<String>, data_type: impl Into<String>) -> Self {
        self.columns.push(ColumnDef::new(name, data_type));
        self
    }

    /// Set description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set source
    pub fn source(mut self, src: impl Into<String>) -> Self {
        self.source = Some(src.into());
        self
    }

    /// Get a column by name (case-insensitive)
    pub fn get_column(&self, name: &str) -> Option<&ColumnDef> {
        self.columns
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }

    /// Check if table has a column (case-insensitive)
    pub fn has_column(&self, name: &str) -> bool {
        self.get_column(name).is_some()
    }
}

/// Per-workspace schema information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSchema {
    /// Workspace ID
    pub workspace_id: String,
    /// Optional friendly name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Tables present in this workspace
    #[serde(default)]
    pub tables: HashSet<String>,
    /// Extended columns for extensible tables (table_name -> additional columns)
    #[serde(default)]
    pub extensions: HashMap<String, Vec<ColumnDef>>,
    /// Custom table schemas (for *_CL tables that only exist in this workspace)
    #[serde(default)]
    pub custom_tables: HashMap<String, TableInfo>,
    /// Last time this workspace's schema was updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<DateTime<Utc>>,
}

impl WorkspaceSchema {
    /// Create a new workspace schema
    pub fn new(workspace_id: impl Into<String>) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            name: None,
            tables: HashSet::new(),
            extensions: HashMap::new(),
            custom_tables: HashMap::new(),
            last_updated: None,
        }
    }

    /// Set friendly name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Add a table to this workspace
    pub fn add_table(&mut self, table_name: impl Into<String>) {
        self.tables.insert(table_name.into());
    }

    /// Check if workspace has a table
    pub fn has_table(&self, table_name: &str) -> bool {
        // Check both canonical tables and custom tables
        self.tables.contains(table_name)
            || self.custom_tables.contains_key(table_name)
    }

    /// Add extension columns for an extensible table
    pub fn add_extensions(&mut self, table_name: impl Into<String>, columns: Vec<ColumnDef>) {
        self.extensions.insert(table_name.into(), columns);
    }

    /// Add a custom table schema
    pub fn add_custom_table(&mut self, table: TableInfo) {
        self.tables.insert(table.name.clone());
        self.custom_tables.insert(table.name.clone(), table);
    }

    /// Mark as updated now
    pub fn touch(&mut self) {
        self.last_updated = Some(Utc::now());
    }

    /// Check if schema is stale (older than given duration)
    pub fn is_stale(&self, max_age: chrono::Duration) -> bool {
        match self.last_updated {
            Some(updated) => Utc::now() - updated > max_age,
            None => true, // Never updated = stale
        }
    }
}

/// Serializable registry data for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryData {
    /// Schema version for migrations
    #[serde(default = "default_version")]
    pub version: u32,
    /// Canonical table schemas
    #[serde(default)]
    pub tables: HashMap<String, TableInfo>,
    /// Per-workspace schema information
    #[serde(default)]
    pub workspaces: HashMap<String, WorkspaceSchema>,
}

fn default_version() -> u32 {
    1
}

impl Default for RegistryData {
    fn default() -> Self {
        Self {
            version: 1,
            tables: HashMap::new(),
            workspaces: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_def_constructors() {
        let col = ColumnDef::string("Name");
        assert_eq!(col.name, "Name");
        assert_eq!(col.data_type, "string");

        let col = ColumnDef::datetime("TimeGenerated");
        assert_eq!(col.data_type, "datetime");
    }

    #[test]
    fn test_table_info_builder() {
        let table = TableInfo::canonical("SecurityEvent")
            .with_column("TimeGenerated", "datetime")
            .with_column("Computer", "string")
            .source("sentinel")
            .description("Windows Security Events");

        assert_eq!(table.name, "SecurityEvent");
        assert_eq!(table.schema_type, SchemaType::Canonical);
        assert_eq!(table.columns.len(), 2);
        assert!(table.has_column("TimeGenerated"));
        assert!(table.has_column("timegenerated")); // case-insensitive
        assert!(!table.has_column("NotAColumn"));
    }

    #[test]
    fn test_workspace_schema() {
        let mut ws = WorkspaceSchema::new("ws-123")
            .with_name("Production");

        ws.add_table("SecurityEvent");
        ws.add_table("SigninLogs");

        assert!(ws.has_table("SecurityEvent"));
        assert!(!ws.has_table("NotATable"));

        // Add custom table
        let custom = TableInfo::custom("MyData_CL")
            .with_column("CustomField", "string");
        ws.add_custom_table(custom);

        assert!(ws.has_table("MyData_CL"));
        assert!(ws.custom_tables.contains_key("MyData_CL"));
    }

    #[test]
    fn test_workspace_staleness() {
        let mut ws = WorkspaceSchema::new("ws-123");

        // Never updated = stale
        assert!(ws.is_stale(chrono::Duration::days(30)));

        // Just updated = not stale
        ws.touch();
        assert!(!ws.is_stale(chrono::Duration::days(30)));
    }

    #[test]
    fn test_registry_data_serialization() {
        let mut data = RegistryData::default();
        data.tables.insert(
            "SecurityEvent".to_string(),
            TableInfo::canonical("SecurityEvent")
                .with_column("TimeGenerated", "datetime"),
        );

        let json = serde_json::to_string_pretty(&data).unwrap();
        let parsed: RegistryData = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, 1);
        assert!(parsed.tables.contains_key("SecurityEvent"));
    }
}
