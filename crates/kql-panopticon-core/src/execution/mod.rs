//! Execution engine for pack execution
//!
//! This module provides the core execution abstraction:
//!
//! - [`ExecutionEngine`] - Trait defining the execution contract
//! - [`PackExecutor`] - Dependency-driven pack execution
//! - [`JobRegistry`] - Background job management with progress tracking
//! - [`progress`] - Progress reporting types
//!
//! ## Execution Model
//!
//! The executor runs pack steps in topological order based on dependencies.
//! Steps with no dependencies can run concurrently (based on configuration).
//! The same executor handles both simple parallel packs and complex chained
//! investigations - the difference is purely in how the pack is authored.

pub mod engine;
pub mod executor;
pub mod progress;
pub mod registry;
pub mod trace;

// Re-exports
pub use engine::{ExecutionEngine, ExecutionOptions, ExecutionMode};
pub use executor::{
    PackExecutor, PackExecutorConfig, PackExecutorResult,
    WorkspaceResult, StepResult, ExecutionStatus, StepStatus,
};
pub use progress::{ProgressSender, ProgressUpdate};
pub use registry::{JobRegistry, JobStatus, JobSummary, JobResult, JobEvent};
pub use trace::{ExecutionTrace, StepTrace};
