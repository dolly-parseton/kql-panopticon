//! Scoring step execution handler
//!
//! Evaluates scoring indicators against acquisition data to produce risk scores.

use crate::error::{Error, Result};
use crate::execution::processing::{
    ProcessingContext, ProcessingStepHandler, ProcessingStepOutput, ProcessingStepType,
};
use crate::pack::{
    MatchedIndicator, ProcessingStep, ProcessingStepConfig, ScoringConfig, ScoringResult,
};
use crate::variable::evaluate_condition;
use async_trait::async_trait;
use serde_json::json;
use std::time::Instant;
use tracing::debug;

/// Handler for scoring processing steps
pub struct ScoringStepHandler;

impl ScoringStepHandler {
    /// Create a new scoring handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for ScoringStepHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProcessingStepHandler for ScoringStepHandler {
    fn handles(&self) -> ProcessingStepType {
        ProcessingStepType::Scoring
    }

    async fn execute(
        &self,
        step: &ProcessingStep,
        ctx: &ProcessingContext<'_>,
    ) -> Result<ProcessingStepOutput> {
        let start = Instant::now();

        // Extract scoring config
        let ProcessingStepConfig::Scoring(scoring_config) = &step.config;

        debug!(
            "Executing scoring step '{}' with {} indicators",
            step.name,
            scoring_config.indicators.len()
        );

        // Evaluate scoring against acquisition results
        let result = evaluate_scoring(scoring_config, ctx)?;

        debug!(
            "Scoring step '{}' completed: score={}, level={}, {} matched indicators",
            step.name,
            result.score,
            result.level,
            result.matched_indicators.len()
        );

        // Write result to JSONL
        let mut writer = ctx.writer(&step.name)?;
        writer.write_row(&json!({
            "score": result.score,
            "level": result.level,
            "matched_indicators": result.matched_indicators,
            "summary": result.summary,
            "recommendation": result.recommendation,
        }))?;

        let handle = writer.finish()?;
        Ok(ProcessingStepOutput::new(handle, start.elapsed()))
    }

    fn validate(&self, step: &ProcessingStep) -> Result<()> {
        let ProcessingStepConfig::Scoring(config) = &step.config;

        // Must have at least one indicator
        if config.indicators.is_empty() {
            return Err(Error::pack(format!(
                "Scoring step '{}' must have at least one indicator",
                step.name
            )));
        }

        // Validate indicator conditions (basic syntax check)
        for indicator in &config.indicators {
            if indicator.condition.trim().is_empty() {
                return Err(Error::pack(format!(
                    "Indicator '{}' in step '{}' has empty condition",
                    indicator.name, step.name
                )));
            }
        }

        // Should have at least one threshold
        if config.thresholds.is_empty() {
            return Err(Error::pack(format!(
                "Scoring step '{}' should have at least one threshold",
                step.name
            )));
        }

        Ok(())
    }
}

/// Evaluate scoring indicators against step data
fn evaluate_scoring(
    config: &ScoringConfig,
    ctx: &ProcessingContext<'_>,
) -> Result<ScoringResult> {
    let mut total_score: i32 = 0;
    let mut matched_indicators = Vec::new();

    // Evaluate each indicator against acquisition results
    for indicator in &config.indicators {
        let matched = evaluate_condition(&indicator.condition, ctx.acquisition_results());

        debug!(
            "Indicator '{}': condition='{}' matched={}",
            indicator.name, indicator.condition, matched
        );

        if matched {
            total_score += indicator.weight;
            matched_indicators.push(MatchedIndicator {
                name: indicator.name.clone(),
                weight: indicator.weight,
                description: indicator.description.clone(),
            });
        }
    }

    // Determine level based on thresholds (sorted descending by min_score)
    let mut thresholds: Vec<_> = config.thresholds.iter().collect();
    thresholds.sort_by(|a, b| b.min_score.cmp(&a.min_score));

    let (level, summary, recommendation) = thresholds
        .iter()
        .find(|t| total_score >= t.min_score)
        .map(|t| {
            // Replace {{score}} placeholder in summary if present
            let summary = t
                .summary
                .as_ref()
                .map(|s| s.replace("{{score}}", &total_score.to_string()));
            (t.level.clone(), summary, t.recommendation.clone())
        })
        .unwrap_or_else(|| ("NONE".to_string(), None, None));

    Ok(ScoringResult {
        score: total_score,
        level,
        matched_indicators,
        summary,
        recommendation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::result::{ResultContext, ResultWriter};
    use crate::pack::{ScoringIndicator, ScoringThreshold};
    use serde_json::Value as JsonValue;

    fn create_test_handle(
        dir: &std::path::Path,
        step_name: &str,
        rows: &[JsonValue],
    ) -> crate::execution::result::ResultHandle {
        let path = dir.join(format!("{}.jsonl", step_name));
        let mut writer = ResultWriter::new(&path, step_name).unwrap();
        writer.write_rows(rows).unwrap();
        writer.finish().unwrap()
    }

    fn make_result_context(temp_dir: &std::path::Path) -> ResultContext {
        let mut results = ResultContext::new();

        let signins_handle = create_test_handle(
            temp_dir,
            "signins",
            &[
                json!({"user": "alice", "failed": 15, "country": "US"}),
                json!({"user": "bob", "failed": 2, "country": "UK"}),
            ],
        );
        results.insert("signins", signins_handle);

        let alerts_handle = create_test_handle(
            temp_dir,
            "alerts",
            &[json!({"severity": "High", "count": 3})],
        );
        results.insert("alerts", alerts_handle);

        results
    }

    fn make_scoring_config() -> ScoringConfig {
        ScoringConfig {
            indicators: vec![
                ScoringIndicator {
                    name: "high_failures".to_string(),
                    condition: "signins.any(failed > 10)".to_string(),
                    weight: 25,
                    description: Some("High number of failed logins".to_string()),
                },
                ScoringIndicator {
                    name: "high_alerts".to_string(),
                    condition: "alerts.any(severity == 'High')".to_string(),
                    weight: 30,
                    description: Some("High severity alerts present".to_string()),
                },
            ],
            thresholds: vec![
                ScoringThreshold {
                    level: "HIGH".to_string(),
                    min_score: 50,
                    summary: Some("High risk detected".to_string()),
                    recommendation: Some("Review recommended".to_string()),
                },
                ScoringThreshold {
                    level: "MEDIUM".to_string(),
                    min_score: 25,
                    summary: Some("Moderate risk".to_string()),
                    recommendation: None,
                },
                ScoringThreshold {
                    level: "LOW".to_string(),
                    min_score: 0,
                    summary: None,
                    recommendation: None,
                },
            ],
        }
    }

    #[tokio::test]
    async fn test_scoring_execution() {
        let temp_dir = tempfile::tempdir().unwrap();
        let handler = ScoringStepHandler::new();
        let acquisition_results = make_result_context(temp_dir.path());

        // Create output directory for processing results
        let output_dir = temp_dir.path().join("processing");
        std::fs::create_dir_all(&output_dir).unwrap();

        let ctx = ProcessingContext::new(&acquisition_results, &output_dir);

        let step = ProcessingStep {
            name: "risk_score".to_string(),
            config: ProcessingStepConfig::Scoring(make_scoring_config()),
            when: None,
        };

        let output = handler.execute(&step, &ctx).await.unwrap();

        // Verify output has a result handle
        assert!(!output.is_empty());
        assert_eq!(output.handle().row_count().unwrap(), 1);

        // Read back the result
        let rows = output.handle().materialize().unwrap();
        assert_eq!(rows.len(), 1);

        let result = &rows[0];
        // Should match high_failures (25) and high_alerts (30) = 55
        assert_eq!(result["score"], json!(55));
        assert_eq!(result["level"], json!("HIGH"));
    }

    #[tokio::test]
    async fn test_scoring_empty_data() {
        let temp_dir = tempfile::tempdir().unwrap();
        let handler = ScoringStepHandler::new();
        let acquisition_results = ResultContext::new();

        let output_dir = temp_dir.path().join("processing");
        std::fs::create_dir_all(&output_dir).unwrap();

        let ctx = ProcessingContext::new(&acquisition_results, &output_dir);

        let step = ProcessingStep {
            name: "risk_score".to_string(),
            config: ProcessingStepConfig::Scoring(make_scoring_config()),
            when: None,
        };

        let output = handler.execute(&step, &ctx).await.unwrap();

        // Read back the result
        let rows = output.handle().materialize().unwrap();
        assert_eq!(rows.len(), 1);

        let result = &rows[0];
        // No matches with empty data
        assert_eq!(result["score"], json!(0));
        assert_eq!(result["level"], json!("LOW"));
    }

    #[test]
    fn test_scoring_validation() {
        let handler = ScoringStepHandler::new();

        // Valid step
        let valid_step = ProcessingStep {
            name: "test".to_string(),
            config: ProcessingStepConfig::Scoring(make_scoring_config()),
            when: None,
        };
        assert!(handler.validate(&valid_step).is_ok());

        // Empty indicators
        let empty_indicators = ProcessingStep {
            name: "test".to_string(),
            config: ProcessingStepConfig::Scoring(ScoringConfig {
                indicators: vec![],
                thresholds: vec![ScoringThreshold {
                    level: "LOW".to_string(),
                    min_score: 0,
                    summary: None,
                    recommendation: None,
                }],
            }),
            when: None,
        };
        assert!(handler.validate(&empty_indicators).is_err());
    }
}
