//! # kql-panopticon-core
//!
//! Core library for KQL query execution against Azure Log Analytics.
//!
//! ## Overview
//!
//! This library provides the foundational components for:
//! - Azure Log Analytics client with token caching and workspace discovery
//! - Pack execution with dependency ordering and variable substitution
//! - Result storage and retrieval
//!
//! ## Architecture
//!
//! - [`client`] - Azure authentication and Log Analytics API client
//! - [`workspace`] - Workspace discovery and management
//! - [`pack`] - Pack definitions (queries with optional dependencies)
//! - [`execution`] - Pack execution engine
//! - [`variable`] - Variable parsing and substitution
//! - [`result`] - Result storage and formats
//! - [`validation`] - KQL syntax validation (optional feature)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kql_panopticon_core::{Client, Pack, PackExecutor, ExecutionEngine};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize client
//!     let client = Client::new().await?;
//!
//!     // Discover workspaces
//!     let workspaces = client.discover_workspaces().await?;
//!
//!     // Load and execute a pack
//!     let pack = Pack::load_from_file("queries.yaml")?;
//!     let executor = PackExecutor::new(client);
//!     let config = PackExecutorConfig::new(pack);
//!     let results = executor.execute(config, workspaces, None).await?;
//!
//!     Ok(())
//! }
//! ```

// Re-export core types at crate root
pub use client::{Client, Column, QueryResponse, Subscription, Table};
pub use error::{Error, Result};
pub use workspace::{Workspace, WorkspaceListResponse, WorkspaceProperties, WorkspaceResource};

// Re-export pack types
pub use pack::{
    Pack, Step, StepType, Input,
    HttpRequest, HttpResponse, HttpMethod, AuthMethod,
    AggregateStrategy, OnEmpty, OnError, QuoteStyle,
    OutputConfig, SecretsConfig, ReportConfig, ScoringConfig,
};

// Re-export execution types
pub use execution::{
    ExecutionEngine, ExecutionOptions, ExecutionMode,
    PackExecutor, PackExecutorConfig, PackExecutorResult,
    WorkspaceResult, StepResult, ExecutionStatus, StepStatus,
    JobRegistry, JobStatus, JobSummary, JobResult, JobEvent,
    ProgressSender, ProgressUpdate,
};

// Module declarations
pub mod client;
pub mod error;
pub mod execution;
pub mod pack;
pub mod result;
pub mod variable;
pub mod workspace;

// KQL validation (via .NET FFI to Kusto.Language)
pub mod validation;
