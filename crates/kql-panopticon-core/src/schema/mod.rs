//! Schema registry for KQL table and column definitions
//!
//! Provides a persistent cache of table schemas, tracking:
//! - Canonical schemas for well-known tables (SecurityEvent, SigninLogs, etc.)
//! - Extensible table schemas (AzureDiagnostics, Syslog) with per-workspace extensions
//! - Custom table schemas (*_CL tables) that are workspace-specific
//! - Which workspaces contain which tables
//!
//! ## Schema Types
//!
//! Tables fall into three categories:
//!
//! - **Canonical**: Fixed schema, same across all workspaces (e.g., SecurityEvent)
//! - **Extensible**: Base schema with workspace-specific extensions (e.g., AzureDiagnostics)
//! - **Custom**: Entirely workspace-specific, no shared schema (e.g., MyData_CL)
//!
//! ## Storage
//!
//! Schemas are persisted to `~/.kql-panopticon/schemas.json` and loaded on startup.
//!
//! ## Schema Capture
//!
//! The [`capture`] module provides workspace schema discovery:
//! 1. Run `search * | distinct $table` to discover tables
//! 2. For each table, run `T | getschema` to capture columns
//! 3. Update the registry with discovered schemas

mod capture;
mod registry;
mod types;

pub use capture::{
    CaptureConfig, CapturePhase, CaptureProgress, CaptureResult,
    ProgressCallback, SchemaCapture, SchemaStatus, get_schema_status,
};
pub use registry::SchemaRegistry;
pub use types::{
    SchemaType, TableInfo, WorkspaceSchema, ColumnDef, RegistryData,
};

// Re-export FFI types that we use for validation integration
pub use kql_language_ffi::{Schema as ValidationSchema, Table as ValidationTable, Column as ValidationColumn};
