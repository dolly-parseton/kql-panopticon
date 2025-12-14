//! Acquisition phase for pack execution
//!
//! The acquisition phase collects data from various sources:
//! - KQL queries against Azure Log Analytics workspaces
//! - HTTP requests to external APIs
//! - Local file reads (CSV, JSON, YAML)
//!
//! ## Architecture
//!
//! ```text
//! AcquisitionPhaseHandler
//!   ├── KqlStepHandler     → Azure Log Analytics
//!   ├── HttpStepHandler    → External APIs
//!   └── FileStepHandler    → Local files
//! ```
//!
//! ## Execution Flow
//!
//! 1. Steps are sorted in topological order based on dependencies
//! 2. Each step is executed with:
//!    - Dependency checking (all deps must succeed)
//!    - Condition evaluation (`when` clauses)
//!    - Variable substitution
//!    - Result writing to JSONL
//! 3. Results are accumulated in `ResultContext` for downstream phases

mod context;
mod handler;
mod output;
mod phase;
pub mod steps;

pub use context::AcquisitionContext;
pub use handler::AcquisitionStepHandler;
pub use output::AcquisitionStepOutput;
pub use phase::{AcquisitionPhaseHandler, AcquisitionPhaseOutput, AcquisitionStepStatus, StepExecutionStatus};
