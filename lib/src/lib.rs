//! # kql-panopticon-core
//!
//! Core library for KQL query execution against Azure Log Analytics.
//!
//! ## Overview
//!
//! This library provides the foundational components for:
//! - Azure Log Analytics client with token caching and workspace discovery
//! - Three-phase pack execution: Acquisition → Processing → Reporting
//! - File-backed result storage for memory efficiency
//!
//! ## Architecture
//!
//! ```text
//! PackExecutor (orchestrator)
//!   ├── AcquisitionPhaseHandler  → Data collection (per workspace)
//!   │     ├── KqlStepHandler     → Azure Log Analytics queries
//!   │     ├── HttpStepHandler    → External API calls
//!   │     └── FileStepHandler    → Local file reads
//!   ├── ProcessingPhaseHandler   → Data transformation (global)
//!   │     └── ScoringStepHandler → Risk scoring
//!   └── ReportingPhaseHandler    → Output generation (global)
//!         └── TemplateStepHandler → Tera template rendering
//! ```
//!
//! ## Modules
//!
//! - [`client`] - Azure authentication and Log Analytics API client
//! - [`workspace`] - Workspace discovery and management
//! - [`pack`] - Pack definitions (queries, processing, reporting)
//! - [`execution`] - Three-phase pack execution engine
//! - [`variable`] - Variable parsing and substitution
//! - [`schema`] - Workspace schema caching and column discovery
//! - [`tracing`] - Execution tracing and TUI event layer
//! - [`validation`] - KQL syntax validation (optional feature)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kql_panopticon_core::{Client, Pack, PackExecutor, PackExecutorConfig};
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

// ============================================================================
// Core Re-exports
// ============================================================================

// Client and Azure types
pub use client::{Client, Column, QueryResponse, Subscription, Table};
pub use error::{Error, Result};
pub use workspace::{Workspace, WorkspaceListResponse, WorkspaceProperties, WorkspaceResource};

// ============================================================================
// Pack Definition Types
// ============================================================================

pub use pack::{
    // Core pack structure
    Acquisition, Pack, Processing, Reporting,
    // Acquisition types
    AcquisitionStepType, AggregateStrategy, AuthMethod, HttpMethod, HttpRequest,
    HttpResponse, Input, InputType, OnEmpty, OnError, OutputConfig, QuoteStyle,
    SecretsConfig, Step, StepType,
    // Processing types
    MatchedIndicator, ProcessingStep, ProcessingStepConfig, ScoringConfig,
    ScoringIndicator, ScoringResult, ScoringThreshold,
    // Reporting types
    ReportDefinition, ReportFormat,
};

// ============================================================================
// Execution Engine
// ============================================================================

pub use execution::{
    // Executor
    ExecutionMode, ExecutionOptions, PackExecutor, PackExecutorConfig, PackExecutorResult,
    // Execution status and results
    ExecutionStatus, StepResult, WorkspaceResult,
    // Phase handlers (for advanced customization)
    AcquisitionPhaseHandler, ProcessingPhaseHandler, ReportingPhaseHandler,
    // Phase step handlers (for extending with custom steps)
    AcquisitionStepHandler, ProcessingStepHandler, ReportingStepHandler,
    // Phase contexts
    AcquisitionContext, ProcessingContext, ReportingContext,
    // Phase outputs
    AcquisitionPhaseOutput, AcquisitionStepOutput, ProcessingPhaseOutput,
    ProcessingStepOutput, ReportingPhaseOutput, ReportingStepOutput,
    // Result storage (with Polars LazyFrame support)
    LazyFrame, ResultContext, ResultHandle, ResultWriter, RowIterator,
    // Job registry
    JobEvent, JobRegistry, JobResult, JobStatus, JobSummary,
    // Progress
    ProgressSender, ProgressUpdate,
    // Tracing
    ExecutionTrace, StepTrace,
};

// Step status types (acquisition phase uses a simpler enum)
pub use execution::{StepExecutionStatus, StepStatus};

// ============================================================================
// Tracing and Logging
// ============================================================================

pub use crate::tracing::{
    tui_channel, ExecutionPhase, FileLayer, LogLevel, TuiEvent, TuiLayer,
};

// ============================================================================
// Schema Registry
// ============================================================================

pub use schema::{ColumnDef, SchemaRegistry, SchemaType, TableInfo, WorkspaceSchema};

// Module declarations
pub mod client;
pub mod error;
pub mod execution;
pub mod pack;
pub mod schema;
pub mod tracing;
pub mod variable;
pub mod workspace;

// KQL validation (via .NET FFI to Kusto.Language)
pub mod validation;

// ============================================================================
// Prelude - Commonly used types for TUI and other consumers
// ============================================================================

/// A "batteries included" prelude for consumers of kql-panopticon-core.
///
/// Import with:
/// ```rust,ignore
/// use kql_panopticon_core::prelude::*;
/// ```
///
/// This provides the most commonly needed types without polluting your namespace
/// with everything exported at the crate root.
pub mod prelude {
    // Core types you'll use in every application
    pub use crate::error::{Error, Result};
    pub use crate::Client;
    pub use crate::Workspace;

    // Pack types for loading and configuring packs
    pub use crate::Pack;

    // Execution types for running packs
    pub use crate::execution::{
        ExecutionMode, ExecutionOptions, ExecutionStatus, PackExecutor, PackExecutorConfig,
        PackExecutorResult, ProgressSender, ProgressUpdate, StepResult, WorkspaceResult,
    };

    // Result access for working with query results
    pub use crate::execution::{ResultContext, ResultHandle};

    // Tracing for TUI integration
    pub use crate::tracing::{tui_channel, TuiEvent, TuiLayer};

    // Job management
    pub use crate::execution::{JobRegistry, JobStatus, JobSummary};
}
