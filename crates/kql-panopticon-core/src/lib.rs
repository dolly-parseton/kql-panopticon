//! # kql-panopticon-core
//!
//! Core library for KQL query execution and chained investigations against Azure Log Analytics.
//!
//! ## Overview
//!
//! This library provides the foundational components for:
//! - Azure Log Analytics client with token caching and workspace discovery
//! - Query execution with concurrent multi-workspace support
//! - Chained investigation execution with dependency ordering
//! - Variable substitution and data passing between steps
//! - Result storage and retrieval
//!
//! ## Architecture
//!
//! The library is organized into several modules:
//!
//! - [`client`] - Azure authentication and Log Analytics API client
//! - [`workspace`] - Workspace discovery and management
//! - [`execution`] - Query and investigation execution engines
//! - [`variable`] - Variable parsing and substitution
//! - [`result`] - Result storage and formats
//! - [`pack`] - Query pack and investigation pack definitions
//! - [`validation`] - KQL syntax and schema validation (optional feature)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kql_panopticon_core::{Client, QueryExecutor, ExecutionEngine};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize client with Azure credentials
//!     let client = Client::new().await?;
//!
//!     // Discover workspaces
//!     let workspaces = client.discover_workspaces().await?;
//!
//!     // Execute a query
//!     let executor = QueryExecutor::new(client);
//!     let results = executor.execute(query_config, workspaces, None).await?;
//!
//!     Ok(())
//! }
//! ```

// Re-export core types at crate root
pub use client::{Client, Column, QueryResponse, Subscription, Table};
pub use error::{Error, Result};
pub use workspace::{Workspace, WorkspaceListResponse, WorkspaceProperties, WorkspaceResource};

// Re-export execution types
pub use execution::{
    ExecutionEngine, JobRegistry, JobStatus, JobSummary, JobResult, JobEvent,
    ProgressSender, ProgressUpdate,
    QueryExecutor, QueryExecutorConfig, QueryExecutorResult,
    QueryJobBuilder, QuerySettings, QueryJobResult, JobSuccess,
};

// Module declarations
pub mod client;
pub mod error;
pub mod execution;
pub mod pack;
pub mod result;
pub mod variable;
pub mod workspace;

// Optional KQL validation (requires .NET AOT library)
#[cfg(feature = "kql-validation")]
pub mod validation;
