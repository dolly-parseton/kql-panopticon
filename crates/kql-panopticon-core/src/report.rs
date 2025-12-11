//! Report generation for pack execution results
//!
//! Generates reports from pack execution results using Tera templates.
//! Supports verdict rules, risk scoring, and custom template filters.
//!
//! ## Report Generation Flow
//!
//! 1. Load step results from output folder
//! 2. Evaluate verdict rules against results
//! 3. Evaluate scoring indicators (if configured)
//! 4. Build template context with all data
//! 5. Render template and write output
//!
//! ## Template Context Variables
//!
//! The following variables are available in templates:
//!
//! - `meta` - Execution metadata (name, timestamp, status, etc.)
//! - `inputs` - User-provided input values
//! - `verdict` - Evaluated verdict (rule_name, level, summary, recommendation)
//! - `scoring` - Risk score results (total_score, level, matched_indicators)
//! - `workspaces` - List of workspace info (name, subscription)
//! - `{step_name}` - Array of rows from each step

use crate::error::{Error, Result};
use crate::execution::PackExecutorResult;
use crate::pack::{Pack, ScoringConfig, VerdictRule};
use crate::variable::evaluate_condition;
use crate::workspace::Workspace;
use log::info;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tera::{Context as TeraContext, Tera, Value};

/// Tera filter: Replace NaN, null, or empty values with a default
/// Usage: {{ value | default_nan(value="N/A") }}
fn filter_default_nan(value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let default_val = args
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A");

    match value {
        Value::Null => Ok(Value::String(default_val.to_string())),
        Value::Number(n) => {
            // Check for NaN (JSON doesn't support NaN, but it might come through as a string)
            if let Some(f) = n.as_f64() {
                if f.is_nan() || f.is_infinite() {
                    return Ok(Value::String(default_val.to_string()));
                }
            }
            Ok(value.clone())
        }
        Value::String(s) => {
            // Handle "NaN", "null", empty strings
            let s_lower = s.to_lowercase();
            if s.is_empty() || s_lower == "nan" || s_lower == "null" || s_lower == "undefined" {
                Ok(Value::String(default_val.to_string()))
            } else {
                Ok(value.clone())
            }
        }
        _ => Ok(value.clone()),
    }
}

/// Tera filter: Deduplicate an array of objects by a specified field
/// Usage: {{ items | unique(field="NetworkMessageId") }}
fn filter_unique(value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let field = args
        .get("field")
        .and_then(|v| v.as_str())
        .ok_or_else(|| tera::Error::msg("unique filter requires 'field' argument"))?;

    let array = value
        .as_array()
        .ok_or_else(|| tera::Error::msg("unique filter can only be applied to arrays"))?;

    let mut seen: HashSet<String> = HashSet::new();
    let mut unique_items: Vec<Value> = Vec::new();

    for item in array {
        let key = item
            .get(field)
            .map(|v| match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => "null".to_string(),
                _ => v.to_string(),
            })
            .unwrap_or_default();

        if seen.insert(key) {
            unique_items.push(item.clone());
        }
    }

    Ok(Value::Array(unique_items))
}

/// Report generator for pack execution results
pub struct ReportGenerator<'a> {
    pack: &'a Pack,
    workspaces: &'a [Workspace],
    inputs: &'a HashMap<String, String>,
}

impl<'a> ReportGenerator<'a> {
    /// Create a new report generator
    pub fn new(
        pack: &'a Pack,
        workspaces: &'a [Workspace],
        inputs: &'a HashMap<String, String>,
    ) -> Self {
        Self {
            pack,
            workspaces,
            inputs,
        }
    }

    /// Generate report from pack execution results
    pub async fn generate(
        &self,
        output_folder: &Path,
        result: &PackExecutorResult,
        timestamp: &str,
    ) -> Result<std::path::PathBuf> {
        let report_config = match &self.pack.report {
            Some(config) => config,
            None => return Err(Error::config("Pack has no report configuration")),
        };

        info!("Generating pack report...");

        // Load all step results for template context
        let step_results = self.load_step_results(output_folder).await?;

        // Evaluate verdict rules
        let verdict = self.evaluate_verdict_rules(&report_config.verdict_rules, &step_results);

        // Evaluate scoring if configured
        let scoring = self
            .pack
            .scoring
            .as_ref()
            .map(|config| self.evaluate_scoring(config, &step_results));

        // Build template context
        let context = self.build_template_context(result, timestamp, &step_results, &verdict, &scoring);

        // Get template content (inline or from file)
        let template = self.resolve_template(report_config, output_folder).await?;

        // Render template
        let rendered = self.render_template(&template, &context)?;

        // Determine output filename
        let output_filename = report_config
            .output
            .as_deref()
            .unwrap_or("report_{{timestamp}}.md")
            .replace("{{timestamp}}", timestamp)
            .replace("{{name}}", &Workspace::normalize_name(&self.pack.name));

        let report_path = output_folder.join(&output_filename);

        // Write report
        tokio::fs::write(&report_path, &rendered).await?;

        info!("Report generated: {}", report_path.display());

        Ok(report_path)
    }

    /// Resolve template content (inline template or from file)
    async fn resolve_template(
        &self,
        config: &crate::pack::ReportConfig,
        output_folder: &Path,
    ) -> Result<String> {
        // Prefer inline template
        if let Some(template) = &config.template {
            return Ok(template.clone());
        }

        // Try template file
        if let Some(template_file) = &config.template_file {
            // Try relative to output folder first, then absolute
            let paths = [
                output_folder.join(template_file),
                std::path::PathBuf::from(template_file),
            ];

            for path in &paths {
                if path.exists() {
                    return tokio::fs::read_to_string(path).await.map_err(|e| {
                        Error::io(format!("Failed to read template file '{}': {}", path.display(), e))
                    });
                }
            }

            return Err(Error::config(format!(
                "Template file not found: {}",
                template_file
            )));
        }

        Err(Error::config(
            "Report configuration must have either 'template' or 'template_file'",
        ))
    }

    /// Load step results from output folder
    async fn load_step_results(
        &self,
        output_folder: &Path,
    ) -> Result<HashMap<String, Vec<serde_json::Value>>> {
        let mut step_results: HashMap<String, Vec<serde_json::Value>> = HashMap::new();

        for workspace in self.workspaces {
            let normalized_subscription = Workspace::normalize_name(&workspace.subscription_name);
            let normalized_workspace = Workspace::normalize_name(&workspace.name);

            for step in &self.pack.steps {
                let results_path = output_folder
                    .join(&normalized_subscription)
                    .join(&normalized_workspace)
                    .join(&step.name)
                    .join("results.json");

                if results_path.exists() {
                    if let Ok(content) = tokio::fs::read_to_string(&results_path).await {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(rows) = json.get("rows").and_then(|r| r.as_array()) {
                                step_results
                                    .entry(step.name.clone())
                                    .or_default()
                                    .extend(rows.clone());
                            }
                        }
                    }
                }
            }
        }

        Ok(step_results)
    }

    /// Build template context for rendering
    fn build_template_context(
        &self,
        result: &PackExecutorResult,
        timestamp: &str,
        step_results: &HashMap<String, Vec<serde_json::Value>>,
        verdict: &serde_json::Value,
        scoring: &Option<serde_json::Value>,
    ) -> TeraContext {
        let mut context = TeraContext::new();

        // Add metadata
        context.insert(
            "meta",
            &serde_json::json!({
                "pack_name": result.pack_name,
                "timestamp": timestamp,
                "job_id": result.job_id.to_string(),
                "status": format!("{:?}", result.status),
                "duration_ms": result.duration_ms,
                "workspace_count": self.workspaces.len(),
            }),
        );

        // Add inputs
        context.insert("inputs", self.inputs);

        // Add step results
        for (step_name, rows) in step_results {
            context.insert(step_name, rows);
        }

        // Add verdict
        context.insert("verdict", verdict);

        // Add scoring if available
        if let Some(score) = scoring {
            context.insert("scoring", score);
        }

        // Add workspaces info
        let workspace_info: Vec<serde_json::Value> = self
            .workspaces
            .iter()
            .map(|w| {
                serde_json::json!({
                    "name": w.name,
                    "subscription": w.subscription_name,
                    "workspace_id": w.workspace_id,
                    "location": w.location,
                })
            })
            .collect();
        context.insert("workspaces", &workspace_info);

        context
    }

    /// Render template with context
    fn render_template(&self, template: &str, context: &TeraContext) -> Result<String> {
        let mut tera = Tera::default();

        // Register custom filters
        tera.register_filter("default_nan", filter_default_nan);
        tera.register_filter("unique", filter_unique);

        tera.add_raw_template("report", template)
            .map_err(|e| Error::config(format!("Failed to parse report template: {}", e)))?;

        tera.render("report", context)
            .map_err(|e| Error::config(format!("Failed to render report: {}", e)))
    }

    /// Evaluate verdict rules against step results
    fn evaluate_verdict_rules(
        &self,
        rules: &[VerdictRule],
        step_results: &HashMap<String, Vec<serde_json::Value>>,
    ) -> serde_json::Value {
        for rule in rules {
            if evaluate_condition(&rule.condition, step_results) {
                return serde_json::json!({
                    "rule_name": rule.name,
                    "level": rule.level,
                    "summary": rule.summary,
                    "recommendation": rule.recommendation,
                });
            }
        }

        // Default verdict if no rules match
        serde_json::json!({
            "rule_name": "default",
            "level": "REQUIRES REVIEW",
            "summary": "No verdict rules matched - manual review required",
            "recommendation": "Review all findings before taking action.",
        })
    }

    /// Evaluate scoring indicators and calculate total score
    fn evaluate_scoring(
        &self,
        scoring_config: &ScoringConfig,
        step_results: &HashMap<String, Vec<serde_json::Value>>,
    ) -> serde_json::Value {
        let mut total_score: i32 = 0;
        let mut matched_indicators: Vec<serde_json::Value> = Vec::new();

        for indicator in &scoring_config.indicators {
            if evaluate_condition(&indicator.condition, step_results) {
                total_score += indicator.weight;
                matched_indicators.push(serde_json::json!({
                    "name": indicator.name,
                    "weight": indicator.weight,
                    "description": indicator.description,
                }));
            }
        }

        // Find matching threshold (thresholds should be sorted descending by min_score)
        let mut level = "UNKNOWN".to_string();
        let mut summary = format!("Risk score: {}", total_score);
        let mut recommendation = "Review the matched indicators.".to_string();

        for threshold in &scoring_config.thresholds {
            if total_score >= threshold.min_score {
                level = threshold.level.clone();
                if let Some(s) = &threshold.summary {
                    summary = s.replace("{{score}}", &total_score.to_string());
                }
                if let Some(r) = &threshold.recommendation {
                    recommendation = r.clone();
                }
                break;
            }
        }

        serde_json::json!({
            "total_score": total_score,
            "level": level,
            "summary": summary,
            "recommendation": recommendation,
            "matched_indicators": matched_indicators,
        })
    }
}

/// Convenience function to generate a report from pack execution
pub async fn generate_report(
    pack: &Pack,
    workspaces: &[Workspace],
    inputs: &HashMap<String, String>,
    output_folder: &Path,
    result: &PackExecutorResult,
    timestamp: &str,
) -> Result<std::path::PathBuf> {
    let generator = ReportGenerator::new(pack, workspaces, inputs);
    generator.generate(output_folder, result, timestamp).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_default_nan_null() {
        let result = filter_default_nan(&Value::Null, &HashMap::new()).unwrap();
        assert_eq!(result, Value::String("N/A".to_string()));
    }

    #[test]
    fn test_filter_default_nan_empty_string() {
        let args = HashMap::new();
        let result = filter_default_nan(&Value::String("".to_string()), &args).unwrap();
        assert_eq!(result, Value::String("N/A".to_string()));
    }

    #[test]
    fn test_filter_default_nan_custom_default() {
        let mut args = HashMap::new();
        args.insert("value".to_string(), Value::String("-".to_string()));
        let result = filter_default_nan(&Value::Null, &args).unwrap();
        assert_eq!(result, Value::String("-".to_string()));
    }

    #[test]
    fn test_filter_default_nan_valid_value() {
        let args = HashMap::new();
        let result = filter_default_nan(&Value::String("valid".to_string()), &args).unwrap();
        assert_eq!(result, Value::String("valid".to_string()));
    }

    #[test]
    fn test_filter_unique() {
        let items = vec![
            serde_json::json!({"id": "a", "value": 1}),
            serde_json::json!({"id": "b", "value": 2}),
            serde_json::json!({"id": "a", "value": 3}), // duplicate by id
        ];

        let mut args = HashMap::new();
        args.insert("field".to_string(), Value::String("id".to_string()));

        let result = filter_unique(&Value::Array(items.into_iter().map(Value::from).collect()), &args).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_evaluate_verdict_rules() {
        let pack = Pack {
            name: "test".to_string(),
            description: None,
            version: None,
            inputs: vec![],
            steps: vec![],
            output: None,
            secrets: None,
            report: None,
            scoring: None,
        };

        let workspaces = vec![];
        let inputs = HashMap::new();

        let generator = ReportGenerator::new(&pack, &workspaces, &inputs);

        let rules = vec![
            VerdictRule {
                name: "high_risk".to_string(),
                condition: "threats is not empty".to_string(),
                level: "HIGH".to_string(),
                summary: Some("High risk detected".to_string()),
                recommendation: Some("Immediate action required".to_string()),
            },
            VerdictRule {
                name: "low_risk".to_string(),
                condition: "true".to_string(),
                level: "LOW".to_string(),
                summary: Some("No issues".to_string()),
                recommendation: None,
            },
        ];

        // Empty results - should match "low_risk" (true condition)
        let step_results = HashMap::new();
        let verdict = generator.evaluate_verdict_rules(&rules, &step_results);
        assert_eq!(verdict["rule_name"], "low_risk");
        assert_eq!(verdict["level"], "LOW");
    }

    #[test]
    fn test_evaluate_scoring() {
        let pack = Pack {
            name: "test".to_string(),
            description: None,
            version: None,
            inputs: vec![],
            steps: vec![],
            output: None,
            secrets: None,
            report: None,
            scoring: None,
        };

        let workspaces = vec![];
        let inputs = HashMap::new();

        let generator = ReportGenerator::new(&pack, &workspaces, &inputs);

        let scoring_config = ScoringConfig {
            indicators: vec![
                crate::pack::ScoringIndicator {
                    name: "indicator1".to_string(),
                    condition: "true".to_string(),
                    weight: 10,
                    description: Some("Always matches".to_string()),
                },
                crate::pack::ScoringIndicator {
                    name: "indicator2".to_string(),
                    condition: "false".to_string(),
                    weight: 20,
                    description: Some("Never matches".to_string()),
                },
            ],
            thresholds: vec![
                crate::pack::ScoringThreshold {
                    level: "HIGH".to_string(),
                    min_score: 20,
                    summary: Some("High risk (score: {{score}})".to_string()),
                    recommendation: None,
                },
                crate::pack::ScoringThreshold {
                    level: "LOW".to_string(),
                    min_score: 0,
                    summary: Some("Low risk (score: {{score}})".to_string()),
                    recommendation: None,
                },
            ],
        };

        let step_results = HashMap::new();
        let scoring = generator.evaluate_scoring(&scoring_config, &step_results);

        // Only indicator1 matches (weight 10), so score should be 10
        // This matches the LOW threshold (min_score: 0)
        assert_eq!(scoring["total_score"], 10);
        assert_eq!(scoring["level"], "LOW");
        assert_eq!(scoring["summary"], "Low risk (score: 10)");
        assert_eq!(scoring["matched_indicators"].as_array().unwrap().len(), 1);
    }
}
