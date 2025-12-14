//! Reporting phase for pack execution
//!
//! The reporting phase generates output from acquisition and processing results
//! using Tera templates.
//!
//! ## Architecture
//!
//! ```text
//! ReportingPhaseHandler
//!   └── TemplateStepHandler  → Tera template rendering
//! ```
//!
//! ## Template Context
//!
//! Reports have access to:
//! - `meta.timestamp` - Execution timestamp
//! - `meta.pack_name` - Pack name
//! - `inputs.*` - User inputs
//! - `acquisition.*` - Acquisition step results (as arrays)
//! - `processing.*` - Processing step results (as objects)

mod context;
mod handler;
mod output;
mod phase;
pub mod steps;

pub use context::{ReportMetadata, ReportingContext};
pub use handler::{ReportingStepHandler, ReportingStepType};
pub use output::{GeneratedReport, ReportingStepOutput};
pub use phase::{ReportingPhaseHandler, ReportingPhaseOutput, ReportingStatus, ReportingStepStatus};
