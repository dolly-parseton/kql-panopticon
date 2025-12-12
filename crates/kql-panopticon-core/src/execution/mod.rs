//! Execution engine for pack execution
//!
//! This module provides the core execution abstraction:
//!
//! - [`ExecutionEngine`] - Trait defining the execution contract
//! - [`PackExecutor`] - Dependency-driven pack execution
//! - [`JobRegistry`] - Background job management with progress tracking
//! - [`StepHandler`] - Internal trait for step type handlers
//!
//! ## Execution Model
//!
//! The executor runs pack steps in topological order based on dependencies.
//! Each step type (KQL, HTTP, File) is handled by a dedicated handler that
//! implements the internal [`StepHandler`] trait.
//!
//! All handlers produce normalized output as `Vec<serde_json::Value>` rows,
//! which enables uniform variable substitution regardless of step type.
//!
//! ## Architecture
//!
//! ```text
//! PackExecutor
//!   ├── KqlHandler    (KQL queries → Azure Log Analytics)
//!   ├── HttpHandler   (HTTP requests → external APIs)
//!   └── [Future: FileHandler, etc.]
//!
//! Step execution:
//!   1. Resolve dependencies (topological sort)
//!   2. For each step:
//!      a. Substitute variables from previous results
//!      b. Dispatch to appropriate handler
//!      c. Store results for downstream steps
//!   3. Write outputs and return results
//! ```

pub mod engine;
pub mod executor;
mod handlers;
mod output;
pub mod progress;
pub mod registry;
mod step;
pub mod trace;
pub mod types;

// Re-exports
pub use engine::{ExecutionEngine, ExecutionMode, ExecutionOptions};
pub use executor::PackExecutor;
pub use output::{format_csv_value, write_csv_results};
pub use progress::{ProgressSender, ProgressUpdate};
pub use registry::{JobEvent, JobRegistry, JobResult, JobStatus, JobSummary};
pub use step::{StepContext, StepOutput};
pub use trace::{ExecutionTrace, StepTrace};
pub use types::{
    ExecutionStatus, PackExecutorConfig, PackExecutorResult, StepResult, StepStatus,
    WorkspaceResult,
};
