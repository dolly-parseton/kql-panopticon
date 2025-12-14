//! Acquisition phase execution context
//!
//! Provides all necessary state for executing acquisition steps.

use crate::error::Result;
use crate::execution::result::{ResultContext, ResultHandle, ResultWriter};
use crate::pack::InputType;
use crate::variable::SubstitutionContext;
use crate::workspace::Workspace;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

/// Context for acquisition phase execution
///
/// Provides:
/// - Workspace information for KQL queries
/// - Variable substitution context
/// - Result writing capabilities
/// - Step result access for dependencies
pub struct AcquisitionContext<'a> {
    /// Target workspace (required for KQL steps)
    pub workspace: &'a Workspace,

    /// Output directory for result files
    output_dir: &'a Path,

    /// Step execution timeout
    pub timeout: Duration,

    /// Substitution context (inputs, secrets, previous results)
    substitution: SubstitutionContext,

    /// Accumulated step results
    results: ResultContext,
}

impl<'a> AcquisitionContext<'a> {
    /// Create a new acquisition context
    pub fn new(
        workspace: &'a Workspace,
        output_dir: &'a Path,
        timeout: Duration,
    ) -> Self {
        Self {
            workspace,
            output_dir,
            timeout,
            substitution: SubstitutionContext::new(),
            results: ResultContext::new(),
        }
    }

    /// Create context with initial inputs
    pub fn with_inputs(
        workspace: &'a Workspace,
        output_dir: &'a Path,
        timeout: Duration,
        inputs: HashMap<String, String>,
        input_types: HashMap<String, InputType>,
    ) -> Self {
        let mut ctx = Self::new(workspace, output_dir, timeout);
        ctx.substitution.inputs = inputs;
        ctx.substitution.input_types = input_types;
        ctx
    }

    /// Get the output directory
    pub fn output_dir(&self) -> &Path {
        self.output_dir
    }

    /// Get the substitution context for variable resolution
    pub fn substitution(&self) -> &SubstitutionContext {
        &self.substitution
    }

    /// Get mutable access to the substitution context
    pub fn substitution_mut(&mut self) -> &mut SubstitutionContext {
        &mut self.substitution
    }

    /// Get the accumulated results context
    pub fn results(&self) -> &ResultContext {
        &self.results
    }

    /// Create a result writer for a step
    pub fn writer(&self, step_name: &str) -> Result<ResultWriter> {
        ResultWriter::for_step(self.output_dir, step_name)
    }

    /// Register a completed step's result handle
    ///
    /// This makes the step's results available for:
    /// - Variable substitution in downstream steps
    /// - Condition evaluation in `when` clauses
    pub fn register_result(&mut self, step_name: impl Into<String>, handle: ResultHandle) {
        let name = step_name.into();
        self.substitution.step_results.insert(name.clone(), handle.clone());
        self.results.insert(name, handle);
    }

    /// Check if a step has results
    pub fn has_step_results(&self, step_name: &str) -> bool {
        self.results.has_results(step_name)
    }

    /// Get row count for a step
    pub fn step_row_count(&self, step_name: &str) -> Result<usize> {
        self.results.row_count(step_name)
    }

    /// Set the foreach iteration row
    ///
    /// Used during foreach step execution to provide the current
    /// row to variable substitution.
    pub fn set_foreach_row(&mut self, alias: String, row: JsonValue) {
        self.substitution.foreach_row = Some((alias, row));
    }

    /// Clear the foreach iteration row
    pub fn clear_foreach_row(&mut self) {
        self.substitution.foreach_row = None;
    }

    /// Take ownership of the accumulated results
    ///
    /// Called at the end of the acquisition phase.
    pub fn take_results(self) -> ResultContext {
        self.results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_workspace() -> Workspace {
        Workspace {
            workspace_id: "test-ws-id".to_string(),
            resource_id: "/subscriptions/test-sub/resourceGroups/test-rg/providers/Microsoft.OperationalInsights/workspaces/test-workspace".to_string(),
            name: "test-workspace".to_string(),
            resource_group: "test-rg".to_string(),
            subscription_id: "test-sub".to_string(),
            subscription_name: "Test Subscription".to_string(),
            tenant_id: "test-tenant".to_string(),
            location: "eastus".to_string(),
        }
    }

    #[test]
    fn test_context_creation() {
        let ws = test_workspace();
        let output_dir = PathBuf::from("/tmp/test");
        let ctx = AcquisitionContext::new(&ws, &output_dir, Duration::from_secs(60));

        assert_eq!(ctx.workspace.name, "test-workspace");
        assert_eq!(ctx.timeout, Duration::from_secs(60));
        assert!(ctx.results().is_context_empty());
    }

    #[test]
    fn test_context_with_inputs() {
        let ws = test_workspace();
        let output_dir = PathBuf::from("/tmp/test");

        let mut inputs = HashMap::new();
        inputs.insert("target".to_string(), "10.0.0.1".to_string());

        let mut types = HashMap::new();
        types.insert("target".to_string(), InputType::String);

        let ctx = AcquisitionContext::with_inputs(
            &ws,
            &output_dir,
            Duration::from_secs(60),
            inputs,
            types,
        );

        assert_eq!(
            ctx.substitution().inputs.get("target"),
            Some(&"10.0.0.1".to_string())
        );
    }
}
