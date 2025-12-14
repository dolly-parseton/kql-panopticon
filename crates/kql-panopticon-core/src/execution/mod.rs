//! Pack execution engine
//!
//! This module provides the execution infrastructure for running packs:
//!
//! ## Architecture
//!
//! ```text
//! PackExecutor (orchestrator)
//!   ├── AcquisitionPhaseHandler
//!   │     ├── KqlStepHandler
//!   │     ├── HttpStepHandler
//!   │     └── FileStepHandler
//!   ├── ProcessingPhaseHandler
//!   │     └── ScoringStepHandler
//!   └── ReportingPhaseHandler
//!         └── TemplateStepHandler
//! ```
//!
//! ## Phases
//!
//! 1. **Acquisition** - Collect data from sources (per workspace)
//! 2. **Processing** - Transform and analyze data (global)
//! 3. **Reporting** - Generate output reports (global)
//!
//! ## Result Storage
//!
//! Results are stored as JSONL files for memory efficiency.
//! Use [`result::ResultHandle`] for lazy access.

// Phase modules
pub mod acquisition;
pub mod processing;
pub mod reporting;

// Result storage
pub mod result;

// Supporting modules
mod executor;
pub mod progress;
pub mod registry;
pub mod trace;
pub mod types;

// Re-exports from phase modules
pub use acquisition::{
    AcquisitionContext, AcquisitionPhaseHandler, AcquisitionPhaseOutput,
    AcquisitionStepHandler, AcquisitionStepOutput, AcquisitionStepStatus,
    StepExecutionStatus,
};
pub use processing::{
    ProcessingContext, ProcessingPhaseHandler, ProcessingPhaseOutput,
    ProcessingStatus, ProcessingStepHandler, ProcessingStepOutput, ProcessingStepStatus,
    ProcessingStepType,
};
pub use reporting::{
    GeneratedReport, ReportingContext, ReportingPhaseHandler, ReportingPhaseOutput,
    ReportingStatus, ReportingStepHandler, ReportingStepOutput, ReportingStepStatus,
    ReportingStepType,
};

// Re-exports from result module
pub use result::{LazyFrame, ResultContext, ResultHandle, ResultWriter, RowIterator};

// Re-exports from executor
pub use executor::{ExecutionMode, ExecutionOptions, PackExecutor};

// Re-exports from other modules
pub use progress::{ProgressSender, ProgressUpdate};
pub use registry::{JobEvent, JobRegistry, JobResult, JobStatus, JobSummary};
pub use trace::{ExecutionTrace, StepTrace};
pub use types::{
    ExecutionStatus, PackExecutorConfig, PackExecutorResult, StepResult, StepStatus,
    WorkspaceResult,
};
