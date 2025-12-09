use crate::error::{KqlPanopticonError, Result};
use crate::investigation_pack::{InvestigationPack, ScoringConfig, VerdictRule};
use crate::workspace::Workspace;
use log::info;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tera::{Context as TeraContext, Tera, Value};

use super::condition::evaluate_condition;
use super::types::InvestigationResult;

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

/// Report generator for investigation results
pub struct ReportGenerator<'a> {
    pack: &'a InvestigationPack,
    workspaces: &'a [Workspace],
    inputs: &'a HashMap<String, String>,
}

impl<'a> ReportGenerator<'a> {
    /// Create a new report generator
    pub fn new(
        pack: &'a InvestigationPack,
        workspaces: &'a [Workspace],
        inputs: &'a HashMap<String, String>,
    ) -> Self {
        Self {
            pack,
            workspaces,
            inputs,
        }
    }

    /// Generate report from investigation results
    pub async fn generate(
        &self,
        output_folder: &Path,
        result: &InvestigationResult,
        timestamp: &str,
    ) -> Result<()> {
        let report_config = match &self.pack.report {
            Some(config) => config,
            None => return Ok(()),
        };

        info!("Generating investigation report...");

        // Load all step results for template context
        let step_results = self.load_step_results(output_folder).await?;

        // Evaluate verdict rules
        let verdict = self.evaluate_verdict_rules(&report_config.verdict_rules, &step_results);

        // Evaluate scoring if configured
        let scoring = self.pack.scoring.as_ref().map(|config| {
            self.evaluate_scoring(config, &step_results)
        });

        // Build template context
        let context = self.build_template_context(result, timestamp, &step_results, &verdict, &scoring);

        // Render template
        let rendered = self.render_template(&report_config.template, &context)?;

        // Determine output filename
        let output_filename = report_config.output
            .replace("{{timestamp}}", timestamp)
            .replace("{{name}}", &Workspace::normalize_name(&self.pack.name));

        let report_path = output_folder.join(&output_filename);

        // Write report
        tokio::fs::write(&report_path, &rendered).await?;

        info!("Report generated: {}", report_path.display());

        Ok(())
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
        result: &InvestigationResult,
        timestamp: &str,
        step_results: &HashMap<String, Vec<serde_json::Value>>,
        verdict: &serde_json::Value,
        scoring: &Option<serde_json::Value>,
    ) -> TeraContext {
        let mut context = TeraContext::new();

        // Add metadata
        context.insert("meta", &serde_json::json!({
            "investigation_name": result.investigation_name,
            "timestamp": timestamp,
            "started_at": result.started_at,
            "completed_at": result.completed_at,
            "status": format!("{:?}", result.status),
            "workspace_count": self.workspaces.len(),
        }));

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
        let workspace_info: Vec<serde_json::Value> = self.workspaces
            .iter()
            .map(|w| serde_json::json!({
                "name": w.name,
                "subscription": w.subscription_name,
            }))
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
            .map_err(|e| KqlPanopticonError::InvestigationExecutionFailed(
                format!("Failed to parse report template: {}", e)
            ))?;

        tera.render("report", context)
            .map_err(|e| KqlPanopticonError::InvestigationExecutionFailed(
                format!("Failed to render report: {}", e)
            ))
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
            "recommendation": "Review all investigation findings before taking action.",
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

// Note: Condition evaluation tests are in condition.rs
