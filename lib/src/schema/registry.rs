//! Schema registry implementation

use super::defaults;
use super::types::{ColumnDef, RegistryData, SchemaType, TableInfo, WorkspaceSchema};
use crate::error::{Error, Result};
use chrono::Duration;
use tracing::{debug, info};
use std::path::{Path, PathBuf};

/// Default staleness threshold (30 days)
const DEFAULT_STALENESS_DAYS: i64 = 30;

/// Schema file name
const SCHEMA_FILE: &str = "schemas.json";

/// Schema registry for tracking table definitions and workspace mappings
///
/// The registry maintains:
/// - Canonical table schemas (shared across workspaces)
/// - Per-workspace information (which tables exist, extensions for extensible tables)
/// - Custom table schemas (workspace-specific *_CL tables)
#[derive(Debug, Clone)]
pub struct SchemaRegistry {
    /// Underlying data
    data: RegistryData,
    /// Path to schema file (None = in-memory only)
    path: Option<PathBuf>,
    /// Whether data has been modified since last save
    dirty: bool,
}

impl SchemaRegistry {
    // ========== Lifecycle ==========

    /// Create a new empty registry (in-memory only)
    pub fn new() -> Self {
        Self {
            data: RegistryData::default(),
            path: None,
            dirty: false,
        }
    }

    /// Create a registry with default schemas
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.load_defaults();
        registry
    }

    /// Load registry from the default location (~/.kql-panopticon/schemas.json)
    pub fn load_default() -> Result<Self> {
        let path = Self::default_path()?;
        Self::load_from_path(&path)
    }

    /// Load registry from a specific path
    pub fn load_from_path(path: &Path) -> Result<Self> {
        if path.exists() {
            debug!("Loading schema registry from {:?}", path);
            let content = std::fs::read_to_string(path)?;
            let data: RegistryData = serde_json::from_str(&content).map_err(|e| {
                Error::config(format!("Failed to parse schema file: {}", e))
            })?;
            info!(
                "Loaded {} tables, {} workspaces from schema registry",
                data.tables.len(),
                data.workspaces.len()
            );
            Ok(Self {
                data,
                path: Some(path.to_path_buf()),
                dirty: false,
            })
        } else {
            debug!(
                "Schema file not found at {:?}, creating new registry",
                path
            );
            let mut registry = Self::with_defaults();
            registry.path = Some(path.to_path_buf());
            registry.dirty = true;
            Ok(registry)
        }
    }

    /// Get the default schema file path
    pub fn default_path() -> Result<PathBuf> {
        let config_dir = dirs::home_dir()
            .ok_or_else(|| Error::config("Could not determine home directory"))?
            .join(".kql-panopticon");
        Ok(config_dir.join(SCHEMA_FILE))
    }

    /// Save registry to disk
    pub fn save(&mut self) -> Result<()> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| Error::config("No path configured for schema registry"))?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&self.data)?;
        std::fs::write(path, content)?;
        self.dirty = false;
        debug!("Saved schema registry to {:?}", path);
        Ok(())
    }

    /// Save if dirty
    pub fn save_if_dirty(&mut self) -> Result<()> {
        if self.dirty {
            self.save()
        } else {
            Ok(())
        }
    }

    /// Check if registry has unsaved changes
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    // ========== Table Operations ==========

    /// Get a canonical table schema
    pub fn get_table(&self, name: &str) -> Option<&TableInfo> {
        self.data.tables.get(name)
    }

    /// Check if a canonical table exists
    pub fn has_table(&self, name: &str) -> bool {
        self.data.tables.contains_key(name)
    }

    /// Add or update a canonical table schema
    pub fn add_table(&mut self, table: TableInfo) {
        self.data.tables.insert(table.name.clone(), table);
        self.dirty = true;
    }

    /// Get all canonical table names
    pub fn table_names(&self) -> Vec<&str> {
        self.data.tables.keys().map(|s| s.as_str()).collect()
    }

    /// Get all canonical tables
    pub fn tables(&self) -> impl Iterator<Item = &TableInfo> {
        self.data.tables.values()
    }

    // ========== Workspace Operations ==========

    /// Get workspace schema info
    pub fn get_workspace(&self, workspace_id: &str) -> Option<&WorkspaceSchema> {
        self.data.workspaces.get(workspace_id)
    }

    /// Get mutable workspace schema (creates if not exists)
    pub fn get_or_create_workspace(&mut self, workspace_id: &str) -> &mut WorkspaceSchema {
        self.dirty = true;
        self.data
            .workspaces
            .entry(workspace_id.to_string())
            .or_insert_with(|| WorkspaceSchema::new(workspace_id))
    }

    /// Check if workspace has a specific table
    pub fn workspace_has_table(&self, workspace_id: &str, table_name: &str) -> bool {
        self.data
            .workspaces
            .get(workspace_id)
            .map(|ws| ws.has_table(table_name))
            .unwrap_or(false)
    }

    /// Add a table to a workspace's known tables
    pub fn add_workspace_table(&mut self, workspace_id: &str, table_name: &str) {
        self.get_or_create_workspace(workspace_id)
            .add_table(table_name);
    }

    /// Get all workspace IDs
    pub fn workspace_ids(&self) -> Vec<&str> {
        self.data.workspaces.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a workspace's schema is stale
    pub fn is_workspace_stale(&self, workspace_id: &str) -> bool {
        self.data
            .workspaces
            .get(workspace_id)
            .map(|ws| ws.is_stale(Duration::days(DEFAULT_STALENESS_DAYS)))
            .unwrap_or(true)
    }

    /// Mark workspace as updated
    pub fn touch_workspace(&mut self, workspace_id: &str) {
        self.get_or_create_workspace(workspace_id).touch();
    }

    // ========== Extended Schema Operations ==========

    /// Get full column list for a table in a specific workspace
    /// (combines canonical columns with workspace extensions)
    pub fn get_columns_for_workspace(
        &self,
        table_name: &str,
        workspace_id: &str,
    ) -> Option<Vec<&ColumnDef>> {
        // First check canonical schema
        let table = self.data.tables.get(table_name)?;

        let mut columns: Vec<&ColumnDef> = table.columns.iter().collect();

        // Add workspace-specific extensions for extensible tables
        if table.schema_type == SchemaType::Extensible {
            if let Some(ws) = self.data.workspaces.get(workspace_id) {
                if let Some(extensions) = ws.extensions.get(table_name) {
                    columns.extend(extensions.iter());
                }
            }
        }

        Some(columns)
    }

    /// Get a custom table schema from a specific workspace
    pub fn get_custom_table(&self, workspace_id: &str, table_name: &str) -> Option<&TableInfo> {
        self.data
            .workspaces
            .get(workspace_id)?
            .custom_tables
            .get(table_name)
    }

    /// Add extension columns to an extensible table for a workspace
    pub fn add_table_extensions(
        &mut self,
        workspace_id: &str,
        table_name: &str,
        columns: Vec<ColumnDef>,
    ) {
        self.get_or_create_workspace(workspace_id)
            .add_extensions(table_name, columns);
    }

    /// Add a custom table to a workspace
    pub fn add_custom_table(&mut self, workspace_id: &str, table: TableInfo) {
        self.get_or_create_workspace(workspace_id)
            .add_custom_table(table);
    }

    // ========== Validation Integration ==========

    /// Convert to FFI validation schema (for a specific workspace)
    pub fn to_validation_schema(&self, workspace_id: Option<&str>) -> kql_language_tools::Schema {
        let mut schema = kql_language_tools::Schema::new();

        // Add canonical tables
        for table_info in self.data.tables.values() {
            let mut table = kql_language_tools::Table::new(&table_info.name);

            // Add base columns
            for col in &table_info.columns {
                table = table.with_column(&col.name, &col.data_type);
            }

            // Add workspace extensions if applicable
            if table_info.schema_type == SchemaType::Extensible {
                if let Some(ws_id) = workspace_id {
                    if let Some(ws) = self.data.workspaces.get(ws_id) {
                        if let Some(extensions) = ws.extensions.get(&table_info.name) {
                            for col in extensions {
                                table = table.with_column(&col.name, &col.data_type);
                            }
                        }
                    }
                }
            }

            schema = schema.table(table);
        }

        // Add workspace custom tables
        if let Some(ws_id) = workspace_id {
            if let Some(ws) = self.data.workspaces.get(ws_id) {
                for custom_table in ws.custom_tables.values() {
                    let mut table = kql_language_tools::Table::new(&custom_table.name);
                    for col in &custom_table.columns {
                        table = table.with_column(&col.name, &col.data_type);
                    }
                    schema = schema.table(table);
                }
            }
        }

        schema
    }

    // ========== Defaults ==========

    /// Load default table schemas
    fn load_defaults(&mut self) {
        let tables = defaults::load_defaults();
        for table in tables {
            self.data.tables.insert(table.name.clone(), table);
        }
        self.dirty = true;
    }

    // ========== Statistics ==========

    /// Get statistics about the registry
    pub fn stats(&self) -> RegistryStats {
        let canonical_count = self
            .data
            .tables
            .values()
            .filter(|t| t.schema_type == SchemaType::Canonical)
            .count();
        let extensible_count = self
            .data
            .tables
            .values()
            .filter(|t| t.schema_type == SchemaType::Extensible)
            .count();
        let custom_count: usize = self
            .data
            .workspaces
            .values()
            .map(|ws| ws.custom_tables.len())
            .sum();

        RegistryStats {
            total_tables: self.data.tables.len(),
            canonical_tables: canonical_count,
            extensible_tables: extensible_count,
            custom_tables: custom_count,
            workspaces: self.data.workspaces.len(),
        }
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the schema registry
#[derive(Debug, Clone)]
pub struct RegistryStats {
    pub total_tables: usize,
    pub canonical_tables: usize,
    pub extensible_tables: usize,
    pub custom_tables: usize,
    pub workspaces: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_registry() {
        let registry = SchemaRegistry::new();
        assert_eq!(registry.data.tables.len(), 0);
        assert!(!registry.is_dirty());
    }

    #[test]
    fn test_with_defaults() {
        let registry = SchemaRegistry::with_defaults();
        assert!(registry.has_table("SecurityEvent"));
        assert!(registry.has_table("SigninLogs"));
        assert!(registry.has_table("AzureDiagnostics"));
        assert!(registry.is_dirty()); // Defaults mark as dirty
    }

    #[test]
    fn test_add_table() {
        let mut registry = SchemaRegistry::new();
        registry.add_table(TableInfo::canonical("TestTable").with_column("Col1", "string"));

        assert!(registry.has_table("TestTable"));
        assert!(registry.is_dirty());

        let table = registry.get_table("TestTable").unwrap();
        assert!(table.has_column("Col1"));
    }

    #[test]
    fn test_workspace_operations() {
        let mut registry = SchemaRegistry::with_defaults();

        // Add table to workspace
        registry.add_workspace_table("ws-123", "SecurityEvent");
        assert!(registry.workspace_has_table("ws-123", "SecurityEvent"));
        assert!(!registry.workspace_has_table("ws-123", "NotATtable"));

        // Workspace that doesn't exist
        assert!(!registry.workspace_has_table("ws-999", "SecurityEvent"));
    }

    #[test]
    fn test_custom_tables() {
        let mut registry = SchemaRegistry::new();

        let custom = TableInfo::custom("MyData_CL")
            .with_column("CustomField", "string")
            .with_column("Value", "long");

        registry.add_custom_table("ws-123", custom);

        assert!(registry.workspace_has_table("ws-123", "MyData_CL"));

        let retrieved = registry.get_custom_table("ws-123", "MyData_CL").unwrap();
        assert_eq!(retrieved.columns.len(), 2);
    }

    #[test]
    fn test_table_extensions() {
        let mut registry = SchemaRegistry::with_defaults();

        // Add extensions to AzureDiagnostics for a workspace
        let extensions = vec![
            ColumnDef::string("CustomField1"),
            ColumnDef::long("CustomField2"),
        ];
        registry.add_table_extensions("ws-123", "AzureDiagnostics", extensions);

        // Get columns for workspace (should include extensions)
        let cols = registry
            .get_columns_for_workspace("AzureDiagnostics", "ws-123")
            .unwrap();
        let col_names: Vec<_> = cols.iter().map(|c| c.name.as_str()).collect();
        assert!(col_names.contains(&"TimeGenerated")); // Base column
        assert!(col_names.contains(&"CustomField1")); // Extension
        assert!(col_names.contains(&"CustomField2")); // Extension
    }

    #[test]
    fn test_validation_schema_conversion() {
        let registry = SchemaRegistry::with_defaults();
        let schema = registry.to_validation_schema(None);

        assert!(!schema.is_empty());
        assert!(schema.get_table("SecurityEvent").is_some());
    }

    #[test]
    fn test_stats() {
        let mut registry = SchemaRegistry::with_defaults();
        registry.add_custom_table("ws-1", TableInfo::custom("Custom_CL"));

        let stats = registry.stats();
        assert!(stats.canonical_tables > 0);
        assert!(stats.extensible_tables > 0);
        assert_eq!(stats.custom_tables, 1);
        assert_eq!(stats.workspaces, 1);
    }
}
