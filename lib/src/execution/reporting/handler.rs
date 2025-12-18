//! Reporting step handler trait
//!
//! Defines the interface for step handlers in the reporting phase.

use crate::error::Result;
use crate::pack::ReportDefinition;
use async_trait::async_trait;

use super::context::ReportingContext;
use super::output::ReportingStepOutput;

/// Step type enum for reporting phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportingStepType {
    /// Template-based report (Tera)
    Template,
}

/// Handler trait for reporting phase steps
///
/// Currently implemented by:
/// - `TemplateStepHandler` - Tera template-based reports
#[async_trait]
pub trait ReportingStepHandler: Send + Sync {
    /// Which step type this handler processes
    fn handles(&self) -> ReportingStepType;

    /// Execute the reporting step
    ///
    /// Reporting steps:
    /// 1. Load template (inline or from file)
    /// 2. Build Tera context from template_data
    /// 3. Render template
    /// 4. Write to output file
    async fn execute(
        &self,
        report: &ReportDefinition,
        ctx: &ReportingContext<'_>,
    ) -> Result<ReportingStepOutput>;

    /// Validate report definition
    fn validate(&self, report: &ReportDefinition) -> Result<()>;
}
