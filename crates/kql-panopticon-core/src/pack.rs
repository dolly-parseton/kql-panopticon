//! Pack definitions for KQL query execution
//!
//! A Pack is a collection of steps (queries) that can be executed against
//! Azure Log Analytics workspaces. Steps can have dependencies on other steps,
//! enabling chained execution with variable passing between steps.
//!
//! Simple packs with no dependencies execute all steps in parallel.
//! Complex packs with dependencies execute in topological order.

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

    /// Example value for validation (used when validating queries with substitution)
    #[serde(default)]
    pub example: Option<String>,
}

fn default_true() -> bool {
    true
}

/// A single execution step
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

    /// Timespan for the query (e.g., "P7D")
    #[serde(default)]
    pub timespan: Option<String>,

    /// HTTP request configuration (for HTTP steps)
    #[serde(default)]
    pub request: Option<HttpRequest>,

    /// HTTP response field mapping (for HTTP steps)
    #[serde(default)]
    pub response: Option<HttpResponse>,

    /// Rate limiting for HTTP steps
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,

    /// Error handling behavior
    #[serde(default)]
    pub on_error: Option<OnError>,

    /// Steps this step depends on (must complete first)
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// Condition for executing this step
    #[serde(default)]
    pub when: Option<String>,

    /// Foreach iteration: "step_name as alias"
    #[serde(default)]
    pub foreach: Option<String>,

    /// Batch size for foreach iterations
    #[serde(default)]
    pub batch_size: Option<usize>,

    /// How to aggregate foreach results
    #[serde(default)]
    pub aggregate: Option<AggregateStrategy>,

    /// Behavior when foreach source is empty
    #[serde(default)]
    pub on_empty: Option<OnEmpty>,

    /// Step-level options
    #[serde(default)]
    pub options: Option<StepOptions>,

    /// Example values for validation (maps variable refs to example values)
    /// Used to substitute realistic values during KQL validation.
    /// Keys are variable references without braces: "step.*.Column" or "step.first.Column"
    #[serde(default)]
    pub examples: HashMap<String, ExampleValue>,
}

/// Example value for validation substitution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExampleValue {
    /// Single example value
    Single(String),
    /// Array of example values (for .* references)
    Array(Vec<String>),
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
    pub method: HttpMethod,

    /// URL (supports variable substitution)
    pub url: String,

    /// Query parameters
    #[serde(default)]
    pub params: HashMap<String, String>,

    /// Headers
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Request body
    #[serde(default)]
    pub body: Option<serde_json::Value>,

    /// Authentication method
    #[serde(default)]
    pub auth: Option<AuthMethod>,
}

/// HTTP method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

/// Authentication method for HTTP steps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    /// Use Azure CLI credential
    Azure,
    /// No authentication
    None,
}

/// HTTP response field mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    /// Field mappings (column_name -> JSONPath)
    #[serde(default)]
    pub fields: HashMap<String, String>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Number of requests allowed
    pub requests: u32,

    /// Time period
    pub per: RateLimitPeriod,
}

/// Time period for rate limiting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitPeriod {
    Second,
    Minute,
    Hour,
}

/// Aggregation strategy for foreach results
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AggregateStrategy {
    /// Concatenate all result rows
    #[default]
    Append,
    /// Deep merge result objects
    Merge,
    /// Keep only last iteration
    Replace,
    /// Wrap each iteration, keyed by source
    Collect,
}

/// Behavior when foreach source is empty
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnEmpty {
    /// Skip the step
    #[default]
    Skip,
    /// Fail the execution
    Error,
}

/// Error handling behavior
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnError {
    /// Fail the step
    #[default]
    Fail,
    /// Skip and continue
    Skip,
    /// Record error, continue
    Continue,
}

/// Step-level options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepOptions {
    /// Quote style for variable substitution
    #[serde(default)]
    pub quote_style: Option<QuoteStyle>,

    /// Deduplicate extracted values
    #[serde(default)]
    pub dedupe: Option<bool>,

    /// Chunk size for large arrays
    #[serde(default)]
    pub chunk_size: Option<usize>,
}

/// Quote style for value substitution
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuoteStyle {
    /// Single quotes: 'value'
    #[default]
    Single,
    /// Double quotes: "value"
    Double,
    /// KQL verbatim: @'value'
    Verbatim,
}

/// Output folder configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Folder template
    #[serde(default)]
    pub folder: Option<String>,
}

/// Secrets configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsConfig {
    /// Secret mappings (name -> env var template)
    #[serde(flatten)]
    pub secrets: HashMap<String, String>,
}

/// Report generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    /// Report format
    #[serde(default)]
    pub format: Option<ReportFormat>,

    /// Output filename
    #[serde(default)]
    pub output: Option<String>,

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

/// Report format
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportFormat {
    #[default]
    Markdown,
    Html,
    Json,
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
    /// Weighted indicators
    #[serde(default)]
    pub indicators: Vec<ScoringIndicator>,

    /// Score thresholds
    #[serde(default)]
    pub thresholds: Vec<ScoringThreshold>,
}

/// Scoring indicator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringIndicator {
    /// Indicator name
    pub name: String,

    /// Condition expression
    pub condition: String,

    /// Weight (positive = risk, negative = benign)
    pub weight: i32,

    /// Description
    #[serde(default)]
    pub description: Option<String>,
}

/// Score threshold
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringThreshold {
    /// Level name
    pub level: String,

    /// Minimum score
    pub min_score: i32,

    /// Summary template
    #[serde(default)]
    pub summary: Option<String>,

    /// Recommendation template
    #[serde(default)]
    pub recommendation: Option<String>,
}

/// Parsed foreach clause
#[derive(Debug, Clone)]
pub struct ForeachClause {
    /// Source step name
    pub source_step: String,
    /// Alias for current row/batch
    pub alias: String,
}

impl ForeachClause {
    /// Parse "step_name as alias"
    pub fn parse(foreach: &str) -> Option<Self> {
        let parts: Vec<&str> = foreach.split_whitespace().collect();
        if parts.len() == 3 && parts[1].eq_ignore_ascii_case("as") {
            Some(ForeachClause {
                source_step: parts[0].to_string(),
                alias: parts[2].to_string(),
            })
        } else {
            None
        }
    }
}

impl QuoteStyle {
    /// Format a single value
    pub fn format_value(&self, value: &str) -> String {
        match self {
            QuoteStyle::Single => {
                let escaped = value.replace('\'', "''");
                format!("'{}'", escaped)
            }
            QuoteStyle::Double => {
                let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                format!("\"{}\"", escaped)
            }
            QuoteStyle::Verbatim => {
                let escaped = value.replace('\'', "''");
                format!("@'{}'", escaped)
            }
        }
    }

    /// Format an array of values
    pub fn format_array(&self, values: &[String]) -> String {
        values
            .iter()
            .map(|v| self.format_value(v))
            .collect::<Vec<_>>()
            .join(",")
    }
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

    /// Validate the pack
    pub fn validate(&self) -> Result<()> {
        self.validate_name()?;
        self.validate_steps_not_empty()?;
        self.validate_step_names_unique()?;
        self.validate_step_types()?;
        self.validate_foreach_syntax()?;
        self.validate_dependencies_exist()?;
        self.validate_no_circular_dependencies()?;
        self.validate_inputs()?;
        Ok(())
    }

    fn validate_name(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(Error::pack("Pack name cannot be empty"));
        }
        Ok(())
    }

    fn validate_steps_not_empty(&self) -> Result<()> {
        if self.steps.is_empty() {
            return Err(Error::pack("Pack must have at least one step"));
        }
        Ok(())
    }

    fn validate_step_names_unique(&self) -> Result<()> {
        let mut seen = HashSet::new();
        for step in &self.steps {
            if step.name.is_empty() {
                return Err(Error::pack("Step name cannot be empty"));
            }
            if !seen.insert(&step.name) {
                return Err(Error::pack(format!(
                    "Duplicate step name: '{}'",
                    step.name
                )));
            }
        }
        Ok(())
    }

    fn validate_step_types(&self) -> Result<()> {
        for step in &self.steps {
            match step.step_type {
                StepType::Kql => {
                    if step.query.as_ref().map_or(true, |q| q.is_empty()) {
                        return Err(Error::pack(format!(
                            "KQL step '{}' must have a query",
                            step.name
                        )));
                    }
                    if step.request.is_some() {
                        return Err(Error::pack(format!(
                            "KQL step '{}' should not have 'request'",
                            step.name
                        )));
                    }
                }
                StepType::Http => {
                    if step.request.is_none() {
                        return Err(Error::pack(format!(
                            "HTTP step '{}' must have 'request'",
                            step.name
                        )));
                    }
                    if step.response.is_none() {
                        return Err(Error::pack(format!(
                            "HTTP step '{}' must have 'response'",
                            step.name
                        )));
                    }
                    if step.query.as_ref().map_or(false, |q| !q.is_empty()) {
                        return Err(Error::pack(format!(
                            "HTTP step '{}' should not have 'query'",
                            step.name
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_foreach_syntax(&self) -> Result<()> {
        for step in &self.steps {
            if let Some(foreach) = &step.foreach {
                if ForeachClause::parse(foreach).is_none() {
                    return Err(Error::pack(format!(
                        "Invalid foreach syntax in step '{}': '{}'. Expected 'step_name as alias'",
                        step.name, foreach
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_dependencies_exist(&self) -> Result<()> {
        let step_names: HashSet<_> = self.steps.iter().map(|s| &s.name).collect();

        for step in &self.steps {
            for dep in &step.depends_on {
                if !step_names.contains(dep) {
                    return Err(Error::pack(format!(
                        "Step '{}' depends on non-existent step '{}'",
                        step.name, dep
                    )));
                }
            }
            if let Some(foreach) = &step.foreach {
                if let Some(clause) = ForeachClause::parse(foreach) {
                    if !step_names.contains(&clause.source_step) {
                        return Err(Error::pack(format!(
                            "Step '{}' foreach references non-existent step '{}'",
                            step.name, clause.source_step
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_no_circular_dependencies(&self) -> Result<()> {
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
        for step in &self.steps {
            let mut deps: Vec<&str> = step.depends_on.iter().map(|s| s.as_str()).collect();
            if let Some(foreach) = &step.foreach {
                if let Some(clause) = ForeachClause::parse(foreach) {
                    if let Some(source) = self.steps.iter().find(|s| s.name == clause.source_step) {
                        if !deps.contains(&source.name.as_str()) {
                            deps.push(&source.name);
                        }
                    }
                }
            }
            graph.insert(&step.name, deps);
        }

        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for step in &self.steps {
            if self.has_cycle(&step.name, &graph, &mut visited, &mut rec_stack) {
                return Err(Error::pack(format!(
                    "Circular dependency detected involving step '{}'",
                    step.name
                )));
            }
        }
        Ok(())
    }

    fn has_cycle<'a>(
        &self,
        node: &'a str,
        graph: &HashMap<&str, Vec<&'a str>>,
        visited: &mut HashSet<&'a str>,
        rec_stack: &mut HashSet<&'a str>,
    ) -> bool {
        if rec_stack.contains(node) {
            return true;
        }
        if visited.contains(node) {
            return false;
        }

        visited.insert(node);
        rec_stack.insert(node);

        if let Some(deps) = graph.get(node) {
            for dep in deps {
                if self.has_cycle(dep, graph, visited, rec_stack) {
                    return true;
                }
            }
        }

        rec_stack.remove(node);
        false
    }

    fn validate_inputs(&self) -> Result<()> {
        let mut seen = HashSet::new();
        for input in &self.inputs {
            if input.name.is_empty() {
                return Err(Error::pack("Input name cannot be empty"));
            }
            if !seen.insert(&input.name) {
                return Err(Error::pack(format!(
                    "Duplicate input name: '{}'",
                    input.name
                )));
            }
        }
        Ok(())
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
            pack: &Pack,
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
                    visit(dep, step_map, pack, visited, temp_visited, result)?;
                }
                // Visit implicit foreach dependency
                if let Some(foreach) = &step.foreach {
                    if let Some(clause) = ForeachClause::parse(foreach) {
                        visit(&clause.source_step, step_map, pack, visited, temp_visited, result)?;
                    }
                }
                visited.insert(step_name.to_string());
                result.push(step);
            }

            temp_visited.remove(step_name);
            Ok(())
        }

        for step in &self.steps {
            visit(&step.name, &step_map, self, &mut visited, &mut temp_visited, &mut result)?;
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

    #[test]
    fn test_circular_dependency() {
        let yaml = r#"
name: "Test"
steps:
  - name: step1
    depends_on: [step2]
    query: "test1"
  - name: step2
    depends_on: [step1]
    query: "test2"
"#;
        let pack: Pack = serde_yaml::from_str(yaml).unwrap();
        let result = pack.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular"));
    }

    #[test]
    fn test_foreach_syntax() {
        let clause = ForeachClause::parse("step1 as item").unwrap();
        assert_eq!(clause.source_step, "step1");
        assert_eq!(clause.alias, "item");

        assert!(ForeachClause::parse("step1 item").is_none());
        assert!(ForeachClause::parse("").is_none());
    }

    #[test]
    fn test_quote_styles() {
        assert_eq!(QuoteStyle::Single.format_value("test"), "'test'");
        assert_eq!(QuoteStyle::Single.format_value("O'Brien"), "'O''Brien'");
        assert_eq!(QuoteStyle::Double.format_value("test"), "\"test\"");
        assert_eq!(QuoteStyle::Verbatim.format_value("test"), "@'test'");
    }

    #[test]
    fn test_duplicate_step_names() {
        let yaml = r#"
name: "Test"
steps:
  - name: step1
    query: "test"
  - name: step1
    query: "test"
"#;
        let pack: Pack = serde_yaml::from_str(yaml).unwrap();
        let result = pack.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Duplicate"));
    }
}
