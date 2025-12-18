//! Processing phase for pack execution
//!
//! The processing phase runs after acquisition completes, transforming
//! and analyzing the collected data.
//!
//! ## Architecture
//!
//! ```text
//! ProcessingPhaseHandler
//!   └── ScoringStepHandler  → Risk scoring
//! ```
//!
//! ## Execution Flow
//!
//! 1. Processing steps run sequentially (may support dependencies later)
//! 2. Each step reads from acquisition results
//! 3. Results are stored as computed fields (not rows)
//! 4. Results are available in reporting as `{{ processing.step_name.field }}`

mod context;
mod handler;
mod output;
mod phase;
pub mod steps;

pub use context::ProcessingContext;
pub use handler::{ProcessingStepHandler, ProcessingStepType};
pub use output::ProcessingStepOutput;
pub use phase::{ProcessingPhaseHandler, ProcessingPhaseOutput, ProcessingStatus, ProcessingStepStatus};
