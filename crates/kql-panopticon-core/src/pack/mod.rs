//! Pack definitions for KQL query execution
//!
//! A Pack is a collection of steps (queries) that can be executed against
//! Azure Log Analytics workspaces. Steps can have dependencies on other steps,
//! enabling chained execution with variable passing between steps.
//!
//! Simple packs with no dependencies execute all steps in parallel.
//! Complex packs with dependencies execute in topological order.

mod types;
mod validation;

pub use types::*;

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A pack of KQL queries with optional dependencies and orchestration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pack {
    /// Pack name
    pub name: String,

    /// Description
    #[serde(default)]
    pub description: Option<String>,

    /// Version
    #[serde(default)]
    pub version: Option<String>,

    /// User-provided inputs for variable substitution
    #[serde(default)]
    pub inputs: Vec<Input>,

    /// Execution steps
    pub steps: Vec<Step>,

    /// Output folder configuration
    #[serde(default)]
    pub output: Option<OutputConfig>,

    /// Secrets from environment variables
    #[serde(default)]
    pub secrets: Option<SecretsConfig>,

    /// Report generation configuration
    #[serde(default)]
    pub report: Option<ReportConfig>,

    /// Risk scoring configuration
    #[serde(default)]
    pub scoring: Option<ScoringConfig>,
}

impl Pack {
    /// Load a pack from a file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;

        let pack: Pack = if path.extension().map_or(false, |e| e == "json") {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };

        pack.validate()?;
        Ok(pack)
    }

    /// Get steps in execution order (topological sort)
    pub fn execution_order(&self) -> Result<Vec<&Step>> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();
        let step_map: HashMap<_, _> = self.steps.iter().map(|s| (s.name.as_str(), s)).collect();

        fn visit<'a>(
            step_name: &str,
            step_map: &HashMap<&str, &'a Step>,
            visited: &mut HashSet<String>,
            temp_visited: &mut HashSet<String>,
            result: &mut Vec<&'a Step>,
        ) -> Result<()> {
            if visited.contains(step_name) {
                return Ok(());
            }
            if temp_visited.contains(step_name) {
                return Err(Error::pack(format!(
                    "Circular dependency at step '{}'",
                    step_name
                )));
            }

            temp_visited.insert(step_name.to_string());

            if let Some(step) = step_map.get(step_name) {
                // Visit explicit dependencies
                for dep in &step.depends_on {
                    visit(dep, step_map, visited, temp_visited, result)?;
                }
                // Visit implicit foreach dependency
                if let Some(foreach) = &step.foreach {
                    if let Some(clause) = ForeachClause::parse(foreach) {
                        visit(&clause.source_step, step_map, visited, temp_visited, result)?;
                    }
                }
                visited.insert(step_name.to_string());
                result.push(step);
            }

            temp_visited.remove(step_name);
            Ok(())
        }

        for step in &self.steps {
            visit(&step.name, &step_map, &mut visited, &mut temp_visited, &mut result)?;
        }

        Ok(result)
    }

    /// Get all dependencies for a step (explicit + implicit from foreach)
    pub fn get_all_dependencies(&self, step: &Step) -> Vec<String> {
        let mut deps = step.depends_on.clone();
        if let Some(foreach) = &step.foreach {
            if let Some(clause) = ForeachClause::parse(foreach) {
                if !deps.contains(&clause.source_step) {
                    deps.push(clause.source_step);
                }
            }
        }
        deps
    }

    /// Get step by name
    pub fn get_step(&self, name: &str) -> Option<&Step> {
        self.steps.iter().find(|s| s.name == name)
    }

    /// Get required inputs without defaults
    pub fn required_inputs(&self) -> Vec<&Input> {
        self.inputs
            .iter()
            .filter(|i| i.required && i.default.is_none())
            .collect()
    }

    /// Check if pack has any dependencies between steps
    pub fn has_dependencies(&self) -> bool {
        self.steps.iter().any(|s| {
            !s.depends_on.is_empty() || s.foreach.is_some() || s.when.is_some()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_pack() {
        let yaml = r#"
name: "Security Baseline"
steps:
  - name: failed_logins
    query: "SigninLogs | where ResultType != 0"
  - name: risky_users
    query: "AADUserRiskEvents | take 100"
"#;
        let pack: Pack = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(pack.name, "Security Baseline");
        assert_eq!(pack.steps.len(), 2);
        assert!(!pack.has_dependencies());
        pack.validate().unwrap();
    }

    #[test]
    fn test_pack_with_dependencies() {
        let yaml = r#"
name: "Phishing Investigation"
inputs:
  - name: url
    required: true
steps:
  - name: clicks
    query: "UrlClickEvents | where Url == '{{inputs.url}}'"
  - name: affected_users
    depends_on: [clicks]
    query: "SigninLogs | where User in ({{clicks.*.User}})"
"#;
        let pack: Pack = serde_yaml::from_str(yaml).unwrap();
        assert!(pack.has_dependencies());
        pack.validate().unwrap();

        let order = pack.execution_order().unwrap();
        let names: Vec<_> = order.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["clicks", "affected_users"]);
    }
}
