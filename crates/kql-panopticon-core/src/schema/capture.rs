//! Schema capture - discovers table schemas from workspaces
//!
//! Implements the "Heavy Pack" approach from ADR-001:
//! 1. Run `search * | distinct $table` to discover tables
//! 2. For each table, run `T | getschema` to capture columns
//! 3. Update the schema registry with discovered schemas
//!
//! This runs in the background after workspace selection, with progress
//! surfaced in the REPL.

use super::{ColumnDef, SchemaRegistry, SchemaType, TableInfo};
use crate::client::Client;
use crate::error::{Error, Result};
use crate::workspace::Workspace;
use chrono::Duration;
use log::{debug, info, warn};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Default timespan for table discovery (7 days) - KQL format for ago()
const DEFAULT_DISCOVERY_TIMESPAN: &str = "7d";

/// Maximum tables to capture per workspace (safety limit)
const MAX_TABLES_PER_WORKSPACE: usize = 200;

/// Schema capture progress callback
pub type ProgressCallback = Box<dyn Fn(CaptureProgress) + Send + Sync>;

/// Progress information during schema capture
#[derive(Debug, Clone)]
pub struct CaptureProgress {
    /// Current phase
    pub phase: CapturePhase,
    /// Workspace being captured
    pub workspace_id: String,
    /// Workspace name (for display)
    pub workspace_name: String,
    /// Tables discovered (after discovery phase)
    pub tables_discovered: usize,
    /// Tables captured so far
    pub tables_captured: usize,
    /// Current table being captured (if in capture phase)
    pub current_table: Option<String>,
    /// Any error message
    pub error: Option<String>,
}

/// Capture phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturePhase {
    /// Starting capture
    Starting,
    /// Discovering tables
    Discovering,
    /// Capturing table schemas
    Capturing,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed,
}

impl std::fmt::Display for CapturePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapturePhase::Starting => write!(f, "starting"),
            CapturePhase::Discovering => write!(f, "discovering tables"),
            CapturePhase::Capturing => write!(f, "capturing schemas"),
            CapturePhase::Completed => write!(f, "completed"),
            CapturePhase::Failed => write!(f, "failed"),
        }
    }
}

/// Schema capture configuration
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Timespan for table discovery query
    pub discovery_timespan: String,
    /// Maximum tables to capture
    pub max_tables: usize,
    /// Skip tables matching these patterns (e.g., internal tables)
    pub skip_patterns: Vec<String>,
    /// Force capture even if not stale
    pub force: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            discovery_timespan: DEFAULT_DISCOVERY_TIMESPAN.to_string(),
            max_tables: MAX_TABLES_PER_WORKSPACE,
            skip_patterns: vec![
                // Skip internal/system tables
                "_".to_string(),
            ],
            force: false,
        }
    }
}

/// Schema capture orchestrator
pub struct SchemaCapture {
    client: Arc<Client>,
    registry: Arc<RwLock<SchemaRegistry>>,
    config: CaptureConfig,
}

impl SchemaCapture {
    /// Create a new schema capture orchestrator
    pub fn new(
        client: Arc<Client>,
        registry: Arc<RwLock<SchemaRegistry>>,
    ) -> Self {
        Self {
            client,
            registry,
            config: CaptureConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(mut self, config: CaptureConfig) -> Self {
        self.config = config;
        self
    }

    /// Check if workspace needs schema capture
    pub async fn needs_capture(&self, workspace_id: &str) -> bool {
        if self.config.force {
            return true;
        }

        let registry = self.registry.read().await;
        registry.is_workspace_stale(workspace_id)
    }

    /// Capture schema for a workspace
    pub async fn capture_workspace(
        &self,
        workspace: &Workspace,
        progress: Option<ProgressCallback>,
    ) -> Result<CaptureResult> {
        let workspace_id = &workspace.workspace_id;
        let workspace_name = &workspace.name;

        info!("Starting schema capture for workspace '{}'", workspace_name);

        // Report starting
        if let Some(ref cb) = progress {
            cb(CaptureProgress {
                phase: CapturePhase::Starting,
                workspace_id: workspace_id.clone(),
                workspace_name: workspace_name.clone(),
                tables_discovered: 0,
                tables_captured: 0,
                current_table: None,
                error: None,
            });
        }

        // Phase 1: Discover tables
        if let Some(ref cb) = progress {
            cb(CaptureProgress {
                phase: CapturePhase::Discovering,
                workspace_id: workspace_id.clone(),
                workspace_name: workspace_name.clone(),
                tables_discovered: 0,
                tables_captured: 0,
                current_table: None,
                error: None,
            });
        }

        let tables = match self.discover_tables(workspace_id).await {
            Ok(t) => t,
            Err(e) => {
                if let Some(ref cb) = progress {
                    cb(CaptureProgress {
                        phase: CapturePhase::Failed,
                        workspace_id: workspace_id.clone(),
                        workspace_name: workspace_name.clone(),
                        tables_discovered: 0,
                        tables_captured: 0,
                        current_table: None,
                        error: Some(e.to_string()),
                    });
                }
                return Err(e);
            }
        };

        let tables_discovered = tables.len();
        info!("Discovered {} tables in '{}'", tables_discovered, workspace_name);

        // Phase 2: Capture schema for each table
        let mut tables_captured = 0;
        let mut canonical_count = 0;
        let mut custom_count = 0;
        let mut errors = Vec::new();

        for table_name in &tables {
            if let Some(ref cb) = progress {
                cb(CaptureProgress {
                    phase: CapturePhase::Capturing,
                    workspace_id: workspace_id.clone(),
                    workspace_name: workspace_name.clone(),
                    tables_discovered,
                    tables_captured,
                    current_table: Some(table_name.clone()),
                    error: None,
                });
            }

            match self.capture_table_schema(workspace_id, table_name).await {
                Ok(table_info) => {
                    // Update registry
                    let mut registry = self.registry.write().await;

                    // Determine if this is a custom table or canonical
                    if table_name.ends_with("_CL") {
                        registry.add_custom_table(workspace_id, table_info);
                        custom_count += 1;
                    } else if registry.has_table(table_name) {
                        // Table exists in registry - just mark workspace has it
                        registry.add_workspace_table(workspace_id, table_name);
                        canonical_count += 1;
                    } else {
                        // Unknown table - add as canonical
                        registry.add_table(table_info);
                        registry.add_workspace_table(workspace_id, table_name);
                        canonical_count += 1;
                    }

                    tables_captured += 1;
                }
                Err(e) => {
                    warn!("Failed to capture schema for '{}': {}", table_name, e);
                    errors.push((table_name.clone(), e.to_string()));
                }
            }
        }

        // Update workspace timestamp
        {
            let mut registry = self.registry.write().await;
            registry.touch_workspace(workspace_id);
        }

        // Report completion
        let phase = if errors.is_empty() {
            CapturePhase::Completed
        } else if tables_captured > 0 {
            CapturePhase::Completed // Partial success
        } else {
            CapturePhase::Failed
        };

        if let Some(ref cb) = progress {
            cb(CaptureProgress {
                phase,
                workspace_id: workspace_id.clone(),
                workspace_name: workspace_name.clone(),
                tables_discovered,
                tables_captured,
                current_table: None,
                error: if errors.is_empty() {
                    None
                } else {
                    Some(format!("{} table(s) failed", errors.len()))
                },
            });
        }

        info!(
            "Schema capture complete for '{}': {} tables ({} canonical, {} custom), {} errors",
            workspace_name, tables_captured, canonical_count, custom_count, errors.len()
        );

        Ok(CaptureResult {
            workspace_id: workspace_id.clone(),
            workspace_name: workspace_name.clone(),
            tables_discovered,
            tables_captured,
            canonical_count,
            custom_count,
            errors,
        })
    }

    /// Discover tables in a workspace
    async fn discover_tables(&self, workspace_id: &str) -> Result<Vec<String>> {
        let query = format!(
            "search * | where TimeGenerated > ago({}) | distinct $table",
            self.config.discovery_timespan
        );

        debug!("Running table discovery query for workspace {}", workspace_id);

        let response = self
            .client
            .query_workspace(workspace_id, &query, None)
            .await?;

        // Extract table names from response
        let mut tables = Vec::new();

        if let Some(table) = response.tables.first() {
            // Find the $table column index
            let table_col_idx = table
                .columns
                .iter()
                .position(|c| c.name == "$table")
                .ok_or_else(|| Error::schema("Table discovery response missing $table column"))?;

            for row in &table.rows {
                if let Some(serde_json::Value::String(name)) = row.get(table_col_idx) {
                    // Apply skip patterns
                    let skip = self.config.skip_patterns.iter().any(|p| {
                        if p == "_" {
                            name.starts_with('_')
                        } else {
                            name.contains(p)
                        }
                    });

                    if !skip && tables.len() < self.config.max_tables {
                        tables.push(name.clone());
                    }
                }
            }
        }

        Ok(tables)
    }

    /// Capture schema for a single table
    async fn capture_table_schema(
        &self,
        workspace_id: &str,
        table_name: &str,
    ) -> Result<TableInfo> {
        let query = format!("{} | getschema", table_name);

        debug!("Capturing schema for table '{}'", table_name);

        let response = self
            .client
            .query_workspace(workspace_id, &query, None)
            .await?;

        // Parse getschema response
        // Columns: ColumnName, ColumnOrdinal, DataType, ColumnType
        let mut columns = Vec::new();

        if let Some(table) = response.tables.first() {
            // Find column indices
            let name_idx = table
                .columns
                .iter()
                .position(|c| c.name == "ColumnName")
                .ok_or_else(|| Error::schema("getschema missing ColumnName"))?;

            let type_idx = table
                .columns
                .iter()
                .position(|c| c.name == "DataType" || c.name == "ColumnType")
                .ok_or_else(|| Error::schema("getschema missing DataType/ColumnType"))?;

            for row in &table.rows {
                if let (
                    Some(serde_json::Value::String(col_name)),
                    Some(serde_json::Value::String(col_type)),
                ) = (row.get(name_idx), row.get(type_idx))
                {
                    columns.push(ColumnDef::new(col_name, col_type));
                }
            }
        }

        // Determine schema type
        let schema_type = if table_name.ends_with("_CL") {
            SchemaType::Custom
        } else if is_extensible_table(table_name) {
            SchemaType::Extensible
        } else {
            SchemaType::Canonical
        };

        Ok(TableInfo {
            name: table_name.to_string(),
            schema_type,
            columns,
            description: None,
            source: Some("captured".to_string()),
        })
    }
}

/// Check if a table is known to be extensible
fn is_extensible_table(name: &str) -> bool {
    matches!(
        name,
        "AzureDiagnostics"
            | "Syslog"
            | "AzureMetrics"
            | "ContainerLog"
            | "ContainerLogV2"
            | "InsightsMetrics"
    )
}

/// Result of schema capture
#[derive(Debug, Clone)]
pub struct CaptureResult {
    /// Workspace ID
    pub workspace_id: String,
    /// Workspace name
    pub workspace_name: String,
    /// Number of tables discovered
    pub tables_discovered: usize,
    /// Number of tables successfully captured
    pub tables_captured: usize,
    /// Canonical tables captured
    pub canonical_count: usize,
    /// Custom tables captured
    pub custom_count: usize,
    /// Errors encountered (table_name, error_message)
    pub errors: Vec<(String, String)>,
}

impl CaptureResult {
    /// Check if capture was fully successful
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    /// Check if capture had partial success
    pub fn is_partial(&self) -> bool {
        !self.errors.is_empty() && self.tables_captured > 0
    }
}

/// Schema capture status for a workspace
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaStatus {
    /// No schema captured
    None,
    /// Schema capture in progress
    Capturing,
    /// Schema available and fresh
    Available,
    /// Schema available but stale (> 30 days)
    Stale,
}

impl std::fmt::Display for SchemaStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaStatus::None => write!(f, "none"),
            SchemaStatus::Capturing => write!(f, "capturing"),
            SchemaStatus::Available => write!(f, "available"),
            SchemaStatus::Stale => write!(f, "stale"),
        }
    }
}

/// Get schema status for a workspace
pub fn get_schema_status(
    registry: &SchemaRegistry,
    workspace_id: &str,
    staleness_days: i64,
) -> SchemaStatus {
    match registry.get_workspace(workspace_id) {
        None => SchemaStatus::None,
        Some(ws) => {
            if ws.is_stale(Duration::days(staleness_days)) {
                SchemaStatus::Stale
            } else {
                SchemaStatus::Available
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_extensible_table() {
        assert!(is_extensible_table("AzureDiagnostics"));
        assert!(is_extensible_table("Syslog"));
        assert!(!is_extensible_table("SecurityEvent"));
        assert!(!is_extensible_table("MyData_CL"));
    }

    #[test]
    fn test_capture_config_default() {
        let config = CaptureConfig::default();
        assert_eq!(config.discovery_timespan, "7d");
        assert_eq!(config.max_tables, 200);
        assert!(!config.force);
    }

    #[test]
    fn test_schema_status_display() {
        assert_eq!(SchemaStatus::None.to_string(), "none");
        assert_eq!(SchemaStatus::Available.to_string(), "available");
        assert_eq!(SchemaStatus::Stale.to_string(), "stale");
    }
}
