//! Investigation pack definitions
//!
//! Defines the structure of investigation packs - chained queries with
//! dependency ordering, variable passing, and report generation.
//!
//! This is a stub that will reference the existing implementation
//! in the main crate during the migration.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// An investigation pack for chained query execution
///
/// Investigation packs enable:
/// - Dependency-ordered step execution
/// - Variable extraction and substitution
/// - HTTP enrichment steps
/// - Conditional execution
/// - Foreach iteration
/// - Report generation
/// - Verdict rules and scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestigationPack {
    /// Pack kind (must be "investigation")
    pub kind: String,
    /// Pack name
    pub name: String,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
    /// Version
    #[serde(default)]
    pub version: Option<String>,
    /// User inputs
    #[serde(default)]
    pub inputs: Vec<Input>,
    /// Investigation steps
    pub steps: Vec<Step>,
    /// Output configuration
    #[serde(default)]
    pub output: Option<OutputConfig>,
    /// Secrets configuration
    #[serde(default)]
    pub secrets: Option<SecretsConfig>,
    /// Report configuration
    #[serde(default)]
    pub report: Option<ReportConfig>,
    /// Scoring configuration
    #[serde(default)]
    pub scoring: Option<ScoringConfig>,
}

/// User-provided input definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
    /// Input name (used in {{inputs.name}})
    pub name: String,
    /// Human-readable label
    #[serde(default)]
    pub label: Option<String>,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
    /// Default value
    #[serde(default)]
    pub default: Option<String>,
    /// Whether input is required
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

/// A single investigation step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Step name (unique identifier)
    pub name: String,
    /// Step type (kql or http)
    #[serde(default, rename = "type")]
    pub step_type: StepType,
    /// KQL query (for KQL steps)
    #[serde(default)]
    pub query: Option<String>,
    /// HTTP request (for HTTP steps)
    #[serde(default)]
    pub request: Option<HttpRequest>,
    /// HTTP response field mapping
    #[serde(default)]
    pub response: Option<HttpResponse>,
    /// Steps this step depends on
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Condition for execution
    #[serde(default)]
    pub when: Option<String>,
    /// Foreach iteration
    #[serde(default)]
    pub foreach: Option<String>,
    /// Batch size for foreach
    #[serde(default)]
    pub batch_size: Option<usize>,
    /// Aggregation strategy
    #[serde(default)]
    pub aggregate: Option<AggregateStrategy>,
    /// Behavior on empty foreach source
    #[serde(default)]
    pub on_empty: Option<OnEmpty>,
    /// Step options
    #[serde(default)]
    pub options: Option<StepOptions>,
}

/// Step type
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepType {
    #[default]
    Kql,
    Http,
}

/// HTTP request configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    /// HTTP method
    pub method: String,
    /// URL (supports variable substitution)
    pub url: String,
    /// Headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Request body
    #[serde(default)]
    pub body: Option<String>,
    /// Rate limit (requests per second)
    #[serde(default)]
    pub rate_limit: Option<f64>,
    /// Error handling strategy
    #[serde(default)]
    pub on_error: Option<ErrorStrategy>,
}

/// HTTP response field mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    /// Field mappings (column_name -> JSONPath)
    #[serde(default)]
    pub fields: HashMap<String, String>,
}

/// Aggregation strategy for foreach
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AggregateStrategy {
    #[default]
    Append,
    Merge,
    Replace,
    Collect,
}

/// Behavior when foreach source is empty
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnEmpty {
    #[default]
    Skip,
    Fail,
    Continue,
}

/// Error handling strategy
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorStrategy {
    #[default]
    Fail,
    Skip,
    Continue,
}

/// Step options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepOptions {
    /// Quote style for variable substitution
    #[serde(default)]
    pub quote_style: Option<String>,
    /// Deduplicate extracted values
    #[serde(default)]
    pub dedupe: Option<bool>,
    /// Chunk size for large arrays
    #[serde(default)]
    pub chunk_size: Option<usize>,
}

/// Output configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Output folder template
    #[serde(default)]
    pub folder: Option<String>,
}

/// Secrets configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsConfig {
    /// Secret mappings (name -> env var)
    #[serde(flatten)]
    pub secrets: HashMap<String, String>,
}

/// Report configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    /// Report format
    #[serde(default)]
    pub format: Option<String>,
    /// Template content
    #[serde(default)]
    pub template: Option<String>,
    /// Template file path
    #[serde(default)]
    pub template_file: Option<String>,
    /// Verdict rules
    #[serde(default)]
    pub verdict_rules: Vec<VerdictRule>,
}

/// Verdict rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerdictRule {
    /// Rule name
    pub name: String,
    /// Condition expression
    pub condition: String,
    /// Verdict level
    pub level: String,
    /// Summary message
    #[serde(default)]
    pub summary: Option<String>,
    /// Recommendation
    #[serde(default)]
    pub recommendation: Option<String>,
}

/// Scoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringConfig {
    /// Scoring indicators
    #[serde(default)]
    pub indicators: Vec<Indicator>,
    /// Score thresholds
    #[serde(default)]
    pub thresholds: Vec<Threshold>,
}

/// Scoring indicator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Indicator {
    /// Indicator name
    pub name: String,
    /// Condition expression
    pub condition: String,
    /// Weight (positive = risk, negative = benign)
    pub weight: i32,
}

/// Score threshold
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Threshold {
    /// Level name
    pub level: String,
    /// Minimum score for this level
    pub min_score: i32,
}

impl InvestigationPack {
    /// Load an investigation pack from a file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;

        let pack: InvestigationPack = if path.extension().map_or(false, |e| e == "json") {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };

        pack.validate()?;
        Ok(pack)
    }

    /// Validate the pack
    pub fn validate(&self) -> Result<()> {
        // TODO: Move comprehensive validation from main crate
        // For now, basic validation
        if self.kind != "investigation" {
            return Err(Error::pack(format!(
                "Invalid pack kind: '{}', expected 'investigation'",
                self.kind
            )));
        }

        if self.name.is_empty() {
            return Err(Error::pack("Pack name cannot be empty"));
        }

        if self.steps.is_empty() {
            return Err(Error::pack("Pack must contain at least one step"));
        }

        // Check for duplicate step names
        let mut seen = std::collections::HashSet::new();
        for step in &self.steps {
            if !seen.insert(&step.name) {
                return Err(Error::pack(format!("Duplicate step name: '{}'", step.name)));
            }
        }

        // Check dependencies exist
        let step_names: std::collections::HashSet<_> =
            self.steps.iter().map(|s| &s.name).collect();

        for step in &self.steps {
            for dep in &step.depends_on {
                if !step_names.contains(dep) {
                    return Err(Error::pack(format!(
                        "Step '{}' depends on unknown step '{}'",
                        step.name, dep
                    )));
                }
            }
        }

        Ok(())
    }

    /// Get execution order (topological sort)
    pub fn execution_order(&self) -> Result<Vec<&Step>> {
        // TODO: Move topological sort from main crate
        // For now, return in definition order (incorrect but compiles)
        Ok(self.steps.iter().collect())
    }

    /// Get step by name
    pub fn get_step(&self, name: &str) -> Option<&Step> {
        self.steps.iter().find(|s| s.name == name)
    }

    /// Get required inputs that don't have defaults
    pub fn required_inputs(&self) -> Vec<&Input> {
        self.inputs
            .iter()
            .filter(|i| i.required && i.default.is_none())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_duplicate_steps() {
        let pack = InvestigationPack {
            kind: "investigation".to_string(),
            name: "test".to_string(),
            description: None,
            version: None,
            inputs: vec![],
            steps: vec![
                Step {
                    name: "step1".to_string(),
                    step_type: StepType::Kql,
                    query: Some("test".to_string()),
                    request: None,
                    response: None,
                    depends_on: vec![],
                    when: None,
                    foreach: None,
                    batch_size: None,
                    aggregate: None,
                    on_empty: None,
                    options: None,
                },
                Step {
                    name: "step1".to_string(), // Duplicate!
                    step_type: StepType::Kql,
                    query: Some("test".to_string()),
                    request: None,
                    response: None,
                    depends_on: vec![],
                    when: None,
                    foreach: None,
                    batch_size: None,
                    aggregate: None,
                    on_empty: None,
                    options: None,
                },
            ],
            output: None,
            secrets: None,
            report: None,
            scoring: None,
        };

        assert!(pack.validate().is_err());
    }
}
