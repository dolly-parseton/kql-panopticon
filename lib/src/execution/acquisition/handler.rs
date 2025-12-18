//! Acquisition step handler trait
//!
//! Defines the interface for step handlers in the acquisition phase.

use crate::error::Result;
use crate::pack::{AcquisitionStepType, Step};
use async_trait::async_trait;
use std::time::Duration;

use super::context::AcquisitionContext;
use super::output::AcquisitionStepOutput;

/// Handler trait for acquisition phase steps
///
/// Implemented by:
/// - `KqlStepHandler` - Azure Log Analytics queries
/// - `HttpStepHandler` - External HTTP API requests
/// - `FileStepHandler` - Local file reading
#[async_trait]
pub trait AcquisitionStepHandler: Send + Sync {
    /// Which step type this handler processes
    fn handles(&self) -> AcquisitionStepType;

    /// Execute the step and return results
    ///
    /// The handler should:
    /// 1. Substitute variables using `ctx.substitution`
    /// 2. Execute the step's operation (query, request, file read)
    /// 3. Write results using `ctx.writer(step_name)`
    /// 4. Return the output with row count and duration
    async fn execute(
        &self,
        step: &Step,
        ctx: &mut AcquisitionContext<'_>,
    ) -> Result<AcquisitionStepOutput>;

    /// Validate step configuration
    ///
    /// Called during pack validation to ensure the step is well-formed
    /// before execution begins.
    fn validate(&self, step: &Step) -> Result<()>;

    /// Get the default timeout for this handler type
    fn default_timeout(&self) -> Duration {
        Duration::from_secs(120)
    }
}
