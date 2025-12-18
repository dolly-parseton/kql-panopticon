//! Processing step handler trait
//!
//! Defines the interface for step handlers in the processing phase.

use crate::error::Result;
use crate::pack::ProcessingStep;
use async_trait::async_trait;

use super::context::ProcessingContext;
use super::output::ProcessingStepOutput;

/// Step type enum for processing phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessingStepType {
    /// Risk scoring based on indicators
    Scoring,
    // Future: Polars, Aggregate, Join, etc.
}

/// Handler trait for processing phase steps
///
/// Currently implemented by:
/// - `ScoringStepHandler` - Risk scoring based on indicators
#[async_trait]
pub trait ProcessingStepHandler: Send + Sync {
    /// Which step type this handler processes
    fn handles(&self) -> ProcessingStepType;

    /// Execute the processing step
    ///
    /// Processing steps:
    /// 1. Read data from acquisition results via `ctx.acquisition_results()`
    /// 2. Perform computation (scoring, aggregation, etc.)
    /// 3. Write results to JSONL via `ctx.writer(step_name)`
    /// 4. Return ProcessingStepOutput with ResultHandle
    async fn execute(
        &self,
        step: &ProcessingStep,
        ctx: &ProcessingContext<'_>,
    ) -> Result<ProcessingStepOutput>;

    /// Validate step configuration
    fn validate(&self, step: &ProcessingStep) -> Result<()>;
}
