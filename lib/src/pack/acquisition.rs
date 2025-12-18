//! Acquisition configuration for pack execution
//!
//! Acquisition defines how data is collected from workspaces:
//! - Inputs: User-provided values for variable substitution
//! - Secrets: Environment variable references
//! - Steps: KQL queries, HTTP calls, or file reads
//! - Output: Where results are stored

use super::types::{Input, OutputConfig, SecretsConfig, Step};
use serde::{Deserialize, Serialize};

/// Data acquisition configuration
///
/// Runs per-workspace, collecting data through defined steps.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Acquisition {
    /// User-provided inputs for variable substitution
    #[serde(default)]
    pub inputs: Vec<Input>,

    /// Secrets from environment variables
    #[serde(default)]
    pub secrets: Option<SecretsConfig>,

    /// Execution steps (KQL, HTTP, File)
    #[serde(default)]
    pub steps: Vec<Step>,

    /// Output folder configuration
    #[serde(default)]
    pub output: Option<OutputConfig>,
}

impl Acquisition {
    /// Create a new empty acquisition config
    pub fn new() -> Self {
        Self::default()
    }

    /// Get required inputs (required=true and no default)
    pub fn required_inputs(&self) -> Vec<&Input> {
        self.inputs
            .iter()
            .filter(|i| i.required && i.default.is_none())
            .collect()
    }

    /// Check if any steps have dependencies or conditions
    pub fn has_dependencies(&self) -> bool {
        self.steps
            .iter()
            .any(|s| !s.depends_on.is_empty() || s.when.is_some())
    }

    /// Get step by name
    pub fn get_step(&self, name: &str) -> Option<&Step> {
        self.steps.iter().find(|s| s.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_acquisition() {
        let acq = Acquisition::new();
        assert!(acq.inputs.is_empty());
        assert!(acq.steps.is_empty());
        assert!(!acq.has_dependencies());
    }
}
