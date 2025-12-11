//! Execution engines for queries and investigations
//!
//! This module provides the core execution abstraction and implementations:
//!
//! - [`ExecutionEngine`] - Trait defining the execution contract
//! - [`QueryExecutor`] - Parallel query execution across workspaces
//! - [`InvestigationExecutor`] - Ordered, chained execution with variable passing
//! - [`JobRegistry`] - Background job management with progress tracking
//! - [`progress`] - Progress reporting types
//!
//! ## Design Philosophy
//!
//! The execution layer abstracts the differences between:
//!
//! 1. **Query Packs** - Independent queries run in parallel across workspaces
//! 2. **Investigation Packs** - Dependent steps run in order with data passing
//!
//! Both share a common interface for progress reporting and result handling,
//! allowing the shell/TUI to treat them uniformly.
//!
//! ## Job Management
//!
//! The [`JobRegistry`] provides:
//! - Job spawning and lifecycle management
//! - Progress subscription (push) for TUI monitors
//! - Progress polling (pull) for shell prompts
//! - Result retrieval and job cancellation

pub mod engine;
pub mod investigation;
pub mod progress;
pub mod query;
pub mod registry;
pub mod trace;

// Re-exports
pub use engine::{ExecutionEngine, ExecutionOptions};
pub use investigation::InvestigationExecutor;
pub use progress::{ProgressSender, ProgressUpdate};
pub use query::{
    QueryExecutor, QueryExecutorConfig, QueryExecutorResult, WorkspaceQueryResult,
    QueryJobBuilder, QuerySettings, QueryJobResult, JobSuccess,
};
pub use registry::{JobRegistry, JobStatus, JobSummary, JobResult, JobEvent};
pub use trace::{ExecutionTrace, StepTrace};
