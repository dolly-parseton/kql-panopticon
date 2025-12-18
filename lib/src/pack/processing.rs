//! Processing configuration for pack execution
//!
//! Processing runs after acquisition completes, transforming and analyzing
//! the collected data. Currently supports scoring; future versions may add
//! Polars-based transformations.

use serde::{Deserialize, Serialize};

/// Processing configuration
///
/// Contains processing steps that run after acquisition.
/// Results are available in reporting as `{{ processing.step_name.* }}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Processing {
    /// Processing steps
    #[serde(default)]
    pub steps: Vec<ProcessingStep>,
}

impl Processing {
    /// Create empty processing config
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if processing has any steps
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Get step by name
    pub fn get_step(&self, name: &str) -> Option<&ProcessingStep> {
        self.steps.iter().find(|s| s.name == name)
    }
}

/// A single processing step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingStep {
    /// Step name (unique identifier, used as `processing.{name}` in templates)
    pub name: String,

    /// Processing step type and configuration
    #[serde(flatten)]
    pub config: ProcessingStepConfig,

    /// Condition for executing this step
    #[serde(default)]
    pub when: Option<String>,
}

/// Processing step type configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProcessingStepConfig {
    /// Risk scoring based on indicators and thresholds
    Scoring(ScoringConfig),
    // Future: Polars, Aggregate, etc.
}

/// Scoring configuration
///
/// Evaluates indicators against step data to produce a risk score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringConfig {
    /// Weighted indicators that contribute to the score
    #[serde(default)]
    pub indicators: Vec<ScoringIndicator>,

    /// Score thresholds that determine risk level
    #[serde(default)]
    pub thresholds: Vec<ScoringThreshold>,
}

/// A scoring indicator
///
/// When the condition matches, the weight is added to the total score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringIndicator {
    /// Indicator name
    pub name: String,

    /// Condition expression (e.g., "step_name.any(column > 10)")
    pub condition: String,

    /// Weight (positive = risk, negative = benign)
    pub weight: i32,

    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,
}

/// A score threshold
///
/// Defines risk levels based on accumulated score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringThreshold {
    /// Level name (e.g., "CRITICAL", "HIGH", "MEDIUM", "LOW")
    pub level: String,

    /// Minimum score for this level
    pub min_score: i32,

    /// Summary message template (supports Tera)
    #[serde(default)]
    pub summary: Option<String>,

    /// Recommendation template (supports Tera)
    #[serde(default)]
    pub recommendation: Option<String>,
}

/// Result of scoring execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoringResult {
    /// Total accumulated score
    pub score: i32,

    /// Matched risk level
    pub level: String,

    /// Indicators that matched
    pub matched_indicators: Vec<MatchedIndicator>,

    /// Summary from threshold (rendered)
    pub summary: Option<String>,

    /// Recommendation from threshold (rendered)
    pub recommendation: Option<String>,
}

/// An indicator that matched during scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedIndicator {
    /// Indicator name
    pub name: String,

    /// Weight that was added
    pub weight: i32,

    /// Description
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_processing() {
        let proc = Processing::new();
        assert!(proc.is_empty());
    }

    #[test]
    fn test_scoring_config_deserialize() {
        let yaml = r#"
name: risk_score
type: scoring
indicators:
  - name: high_failures
    condition: "signins.any(FailedCount > 10)"
    weight: 25
thresholds:
  - level: HIGH
    min_score: 50
"#;
        let step: ProcessingStep = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(step.name, "risk_score");

        match step.config {
            ProcessingStepConfig::Scoring(cfg) => {
                assert_eq!(cfg.indicators.len(), 1);
                assert_eq!(cfg.thresholds.len(), 1);
            }
        }
    }
}
