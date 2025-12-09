use crate::error::{KqlPanopticonError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Step type discriminator
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum StepType {
    /// KQL query step (default)
    #[default]
    Kql,
    /// HTTP request step for external APIs
    Http,
}

/// HTTP method for HTTP steps
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

/// Authentication method for HTTP steps
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    /// Use existing Azure CLI credential for Azure Management API calls
    Azure,
    /// No authentication (or authentication defined in headers)
    None,
}

/// HTTP request configuration for HTTP steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    /// HTTP method (GET, POST, PUT, DELETE)
    pub method: HttpMethod,

    /// URL template (supports variable substitution like {{inputs.name}}, {{step.*.Column}})
    pub url: String,

    /// Query string parameters (supports variable substitution)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>,

    /// HTTP headers (supports variable substitution, including {{secrets.name}})
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,

    /// Request body for POST/PUT (supports variable substitution)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,

    /// Authentication method
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthMethod>,
}

/// HTTP response extraction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    /// JSONPath to field name mapping for extracting data from response
    /// Key is the output field name, value is the JSONPath expression
    pub fields: HashMap<String, String>,
}

/// Rate limiting configuration for HTTP steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Number of requests allowed in the time period
    pub requests: u32,

    /// Time period for rate limiting
    pub per: RateLimitPeriod,
}

/// Time period for rate limiting
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitPeriod {
    Second,
    Minute,
    Hour,
}

/// Error handling behavior for HTTP steps
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OnError {
    /// Record error in _error column, continue with next row
    #[default]
    Continue,
    /// Skip failed row entirely (don't include in results)
    Skip,
    /// Fail the entire step on first error
    Fail,
}

/// Secrets configuration (top-level in investigation pack)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecretsConfig {
    /// Map of secret name to value template (e.g., ${ENV_VAR_NAME})
    #[serde(flatten)]
    pub secrets: HashMap<String, String>,
}

/// An investigation pack containing chained queries with variable extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestigationPack {
    /// Must be "investigation" to distinguish from query packs
    pub kind: String,

    /// Investigation name
    pub name: String,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Optional version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Output configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputConfig>,

    /// Secrets configuration for HTTP steps (API keys, tokens from environment variables)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<SecretsConfig>,

    /// User-provided input variables
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Input>,

    /// Investigation steps (queries with dependencies)
    pub steps: Vec<Step>,

    /// Report generation configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<ReportConfig>,

    /// Scoring engine configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scoring: Option<ScoringConfig>,
}

/// Output folder configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Folder template (supports {{name}}, {{timestamp}})
    pub folder: String,
}

/// A user-provided input variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
    /// Variable name (referenced as {{inputs.name}})
    pub name: String,

    /// Description shown to user when prompting
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Input type (currently only "string" supported)
    #[serde(rename = "type", default = "default_input_type")]
    pub input_type: InputType,

    /// Whether this input is required
    #[serde(default = "default_true")]
    pub required: bool,

    /// Default value if not provided
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

fn default_input_type() -> InputType {
    InputType::String
}

fn default_true() -> bool {
    true
}

/// Input variable type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    String,
}

/// A step in the investigation (a query with optional dependencies and iteration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Unique step name (used for references like {{step_name.*.Column}})
    pub name: String,

    /// Step type: kql (default) or http
    #[serde(rename = "type", default, skip_serializing_if = "is_default_step_type")]
    pub step_type: StepType,

    /// The KQL query (may contain {{variable}} placeholders)
    /// Required for KQL steps, should be empty for HTTP steps
    #[serde(default)]
    pub query: String,

    /// HTTP request configuration (required for HTTP steps)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<HttpRequest>,

    /// HTTP response extraction configuration (required for HTTP steps)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<HttpResponse>,

    /// Rate limiting configuration for HTTP steps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,

    /// Error handling behavior for HTTP steps (default: continue)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_error: Option<OnError>,

    /// Steps this step depends on (must complete first)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,

    /// Condition for executing this step (if false, step is skipped)
    /// Uses same expression syntax as verdict rules
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,

    /// Row iteration: "step_name as alias" to iterate over source step's rows
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foreach: Option<String>,

    /// How to combine results from foreach iterations
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregate: Option<AggregateStrategy>,

    /// Number of rows per foreach iteration (1 = per-row, N = batched)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<usize>,

    /// Behavior when foreach source is empty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_empty: Option<OnEmpty>,

    /// Step options (quote style, chunking, deduplication)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<StepOptions>,

    /// Values to extract from query results (deprecated: use direct access syntax)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extract: HashMap<String, Extract>,
}

/// Helper function for serde skip_serializing_if on step_type
fn is_default_step_type(step_type: &StepType) -> bool {
    *step_type == StepType::Kql
}

/// How to aggregate results from foreach iterations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AggregateStrategy {
    /// Concatenate all result rows into single array
    #[default]
    Append,
    /// Deep merge result objects (for single-row results)
    Merge,
    /// Only keep last iteration's results
    Replace,
    /// Wrap each iteration's results, keyed by source row
    Collect,
}

/// Behavior when foreach source step has no results
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OnEmpty {
    /// Skip the step entirely
    #[default]
    Skip,
    /// Fail the investigation
    Error,
}

/// Step-level options for variable substitution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StepOptions {
    /// Quote style for value substitution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_style: Option<QuoteStyle>,

    /// Remove duplicate values before substitution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dedupe: Option<bool>,

    /// Maximum values per query chunk (splits into multiple queries)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<usize>,
}

/// Parsed foreach clause
#[derive(Debug, Clone)]
pub struct ForeachClause {
    /// Source step name to iterate over
    pub source_step: String,
    /// Alias for accessing current row/batch
    pub alias: String,
}

impl ForeachClause {
    /// Parse a foreach string like "step_name as alias"
    pub fn parse(foreach: &str) -> Option<Self> {
        let parts: Vec<&str> = foreach.split_whitespace().collect();
        if parts.len() == 3 && parts[1].to_lowercase() == "as" {
            Some(ForeachClause {
                source_step: parts[0].to_string(),
                alias: parts[2].to_string(),
            })
        } else {
            None
        }
    }
}

/// Configuration for extracting a value from query results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extract {
    /// Column name to extract from results
    pub column: String,

    /// Whether to extract a single value or array
    #[serde(rename = "type", default = "default_extract_type")]
    pub extract_type: ExtractType,

    /// How to quote the value in substitution
    #[serde(default = "default_quote_style")]
    pub quote_style: QuoteStyle,

    /// Max items per query chunk (for arrays)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<usize>,

    /// Remove duplicates (default true for arrays)
    #[serde(default = "default_true")]
    pub dedupe: bool,
}

fn default_extract_type() -> ExtractType {
    ExtractType::Array
}

fn default_quote_style() -> QuoteStyle {
    QuoteStyle::Single
}

/// Type of value extraction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExtractType {
    /// Extract first value only
    Single,
    /// Extract all values as array
    Array,
}

/// Quote style for value substitution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum QuoteStyle {
    /// Single quotes: 'value' (escapes ' as '')
    #[default]
    Single,
    /// Double quotes: "value" (escapes " as \")
    Double,
    /// KQL verbatim string: @'value' (escapes ' as '')
    Verbatim,
}

/// Report generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    /// Output format: markdown, html, or json
    #[serde(default = "default_report_format")]
    pub format: ReportFormat,

    /// Output filename template (supports variable substitution)
    #[serde(default = "default_report_filename")]
    pub output: String,

    /// Report template (Tera/Jinja2-style syntax)
    pub template: String,

    /// Verdict rules evaluated in order; first match wins
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verdict_rules: Vec<VerdictRule>,
}

fn default_report_format() -> ReportFormat {
    ReportFormat::Markdown
}

fn default_report_filename() -> String {
    "report.md".to_string()
}

/// Report output format
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReportFormat {
    Markdown,
    Html,
    Json,
}

/// A verdict rule that evaluates conditions to determine investigation outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerdictRule {
    /// Rule name for identification
    pub name: String,

    /// Condition expression (evaluated against step results)
    /// Supports: step_name.field == value, step_name.field > value, and/or/not
    pub condition: String,

    /// Verdict level if condition matches
    pub level: String,

    /// Summary text explaining the verdict
    pub summary: String,

    /// Recommended action for the analyst
    pub recommendation: String,
}

/// Scoring engine configuration for weighted risk assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringConfig {
    /// Weighted indicators (positive = risk, negative = benign)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indicators: Vec<ScoringIndicator>,

    /// Score thresholds for verdict levels
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub thresholds: Vec<ScoringThreshold>,
}

/// A weighted indicator for the scoring engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringIndicator {
    /// Indicator name for display
    pub name: String,

    /// Condition expression (same syntax as verdict rules)
    pub condition: String,

    /// Weight value (positive = increases risk, negative = decreases risk)
    pub weight: i32,

    /// Optional description of what this indicator means
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Score threshold mapping to verdict level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringThreshold {
    /// Verdict level name
    pub level: String,

    /// Minimum score for this level (inclusive)
    pub min_score: i32,

    /// Summary template (can reference {{score}})
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,

    /// Recommendation template
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommendation: Option<String>,
}

impl QuoteStyle {
    /// Format a single value with this quote style
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

    /// Format an array of values with this quote style (comma-separated)
    pub fn format_array(&self, values: &[String]) -> String {
        values
            .iter()
            .map(|v| self.format_value(v))
            .collect::<Vec<_>>()
            .join(",")
    }
}

impl InvestigationPack {
    /// Load an investigation pack from a file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(KqlPanopticonError::InvestigationPackNotFound(
                path.display().to_string(),
            ));
        }

        let content = std::fs::read_to_string(path)?;

        let pack: Self = if path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };

        Ok(pack)
    }

    /// Save an investigation pack to a file
    #[allow(dead_code)]
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let content = if path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::to_string_pretty(self)?
        } else {
            serde_yaml::to_string(self)?
        };

        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate the investigation pack
    pub fn validate(&self) -> Result<()> {
        self.validate_kind()?;
        self.validate_steps_not_empty()?;
        self.validate_step_names_unique()?;
        self.validate_step_types()?;
        self.validate_foreach_syntax()?;
        self.validate_dependencies_exist()?;
        self.validate_no_circular_dependencies()?;
        self.validate_variable_references()?;
        self.validate_inputs()?;
        Ok(())
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

    fn validate_kind(&self) -> Result<()> {
        if self.kind != "investigation" {
            return Err(KqlPanopticonError::InvestigationPackValidation(
                format!("Invalid kind '{}', expected 'investigation'", self.kind),
            ));
        }
        Ok(())
    }

    fn validate_steps_not_empty(&self) -> Result<()> {
        if self.steps.is_empty() {
            return Err(KqlPanopticonError::InvestigationPackValidation(
                "Investigation pack must have at least one step".into(),
            ));
        }
        Ok(())
    }

    fn validate_step_names_unique(&self) -> Result<()> {
        let mut seen = HashSet::new();
        for step in &self.steps {
            if step.name.is_empty() {
                return Err(KqlPanopticonError::InvestigationPackValidation(
                    "Step name cannot be empty".into(),
                ));
            }
            if !seen.insert(&step.name) {
                return Err(KqlPanopticonError::InvestigationPackValidation(
                    format!("Duplicate step name: '{}'", step.name),
                ));
            }
        }
        Ok(())
    }

    /// Validate step type-specific requirements (KQL vs HTTP)
    fn validate_step_types(&self) -> Result<()> {
        for step in &self.steps {
            match step.step_type {
                StepType::Kql => {
                    // KQL steps must have a query
                    if step.query.is_empty() {
                        return Err(KqlPanopticonError::InvestigationPackValidation(
                            format!("KQL step '{}' must have a 'query' field", step.name),
                        ));
                    }
                    // KQL steps should not have request/response config
                    if step.request.is_some() {
                        return Err(KqlPanopticonError::InvestigationPackValidation(
                            format!("KQL step '{}' should not have 'request' configuration", step.name),
                        ));
                    }
                    if step.response.is_some() {
                        return Err(KqlPanopticonError::InvestigationPackValidation(
                            format!("KQL step '{}' should not have 'response' configuration", step.name),
                        ));
                    }
                }
                StepType::Http => {
                    // HTTP steps must have request config
                    if step.request.is_none() {
                        return Err(KqlPanopticonError::InvestigationPackValidation(
                            format!("HTTP step '{}' must have 'request' configuration", step.name),
                        ));
                    }
                    // HTTP steps must have response config
                    if step.response.is_none() {
                        return Err(KqlPanopticonError::InvestigationPackValidation(
                            format!("HTTP step '{}' must have 'response' configuration", step.name),
                        ));
                    }
                    // HTTP steps should not have a query
                    if !step.query.is_empty() {
                        return Err(KqlPanopticonError::InvestigationPackValidation(
                            format!("HTTP step '{}' should not have 'query' field", step.name),
                        ));
                    }
                    // Validate response fields are not empty
                    if let Some(response) = &step.response {
                        if response.fields.is_empty() {
                            return Err(KqlPanopticonError::InvestigationPackValidation(
                                format!("HTTP step '{}' response must have at least one field mapping", step.name),
                            ));
                        }
                    }
                    // Validate secrets references in headers if any
                    if let Some(request) = &step.request {
                        self.validate_http_secrets_references(step, request)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Validate that secrets referenced in HTTP headers exist in the secrets config
    fn validate_http_secrets_references(&self, step: &Step, request: &HttpRequest) -> Result<()> {
        let secrets_pattern = regex::Regex::new(r"\{\{secrets\.([^}]+)\}\}").unwrap();
        let available_secrets: HashSet<_> = self.secrets
            .as_ref()
            .map(|s| s.secrets.keys().cloned().collect())
            .unwrap_or_default();

        // Check headers for secrets references
        for (header_name, header_value) in &request.headers {
            for cap in secrets_pattern.captures_iter(header_value) {
                let secret_name = cap.get(1).unwrap().as_str();
                if !available_secrets.contains(secret_name) {
                    return Err(KqlPanopticonError::InvestigationPackValidation(
                        format!(
                            "HTTP step '{}' header '{}' references undefined secret '{}'",
                            step.name, header_name, secret_name
                        ),
                    ));
                }
            }
        }

        // Check URL for secrets references
        for cap in secrets_pattern.captures_iter(&request.url) {
            let secret_name = cap.get(1).unwrap().as_str();
            if !available_secrets.contains(secret_name) {
                return Err(KqlPanopticonError::InvestigationPackValidation(
                    format!(
                        "HTTP step '{}' URL references undefined secret '{}'",
                        step.name, secret_name
                    ),
                ));
            }
        }

        // Check params for secrets references
        for (param_name, param_value) in &request.params {
            for cap in secrets_pattern.captures_iter(param_value) {
                let secret_name = cap.get(1).unwrap().as_str();
                if !available_secrets.contains(secret_name) {
                    return Err(KqlPanopticonError::InvestigationPackValidation(
                        format!(
                            "HTTP step '{}' param '{}' references undefined secret '{}'",
                            step.name, param_name, secret_name
                        ),
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_foreach_syntax(&self) -> Result<()> {
        for step in &self.steps {
            if let Some(foreach) = &step.foreach {
                if ForeachClause::parse(foreach).is_none() {
                    return Err(KqlPanopticonError::InvestigationPackValidation(
                        format!(
                            "Invalid foreach syntax in step '{}': '{}'. Expected 'step_name as alias'",
                            step.name, foreach
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_dependencies_exist(&self) -> Result<()> {
        let step_names: HashSet<_> = self.steps.iter().map(|s| &s.name).collect();

        for step in &self.steps {
            // Check explicit dependencies
            for dep in &step.depends_on {
                if !step_names.contains(dep) {
                    return Err(KqlPanopticonError::InvestigationPackValidation(
                        format!(
                            "Step '{}' depends on non-existent step '{}'",
                            step.name, dep
                        ),
                    ));
                }
            }
            // Check foreach source step exists
            if let Some(foreach) = &step.foreach {
                if let Some(clause) = ForeachClause::parse(foreach) {
                    if !self.steps.iter().any(|s| s.name == clause.source_step) {
                        return Err(KqlPanopticonError::InvestigationPackValidation(
                            format!(
                                "Step '{}' foreach references non-existent step '{}'",
                                step.name, clause.source_step
                            ),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_no_circular_dependencies(&self) -> Result<()> {
        // Build adjacency list (including implicit foreach dependencies)
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
        for step in &self.steps {
            let mut deps: Vec<&str> = step.depends_on.iter().map(|s| s.as_str()).collect();
            // Add foreach source as implicit dependency
            if let Some(foreach) = &step.foreach {
                if let Some(clause) = ForeachClause::parse(foreach) {
                    // Find the source step name in self.steps to get a stable reference
                    if let Some(source) = self.steps.iter().find(|s| s.name == clause.source_step) {
                        if !deps.contains(&source.name.as_str()) {
                            deps.push(&source.name);
                        }
                    }
                }
            }
            graph.insert(&step.name, deps);
        }

        // DFS-based cycle detection
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for step in &self.steps {
            if self.has_cycle(&step.name, &graph, &mut visited, &mut rec_stack)? {
                return Err(KqlPanopticonError::CircularDependency(format!(
                    "Circular dependency detected involving step '{}'",
                    step.name
                )));
            }
        }

        Ok(())
    }

    #[allow(clippy::only_used_in_recursion)]
    fn has_cycle<'a>(
        &self,
        node: &'a str,
        graph: &HashMap<&str, Vec<&'a str>>,
        visited: &mut HashSet<&'a str>,
        rec_stack: &mut HashSet<&'a str>,
    ) -> Result<bool> {
        if rec_stack.contains(node) {
            return Ok(true);
        }
        if visited.contains(node) {
            return Ok(false);
        }

        visited.insert(node);
        rec_stack.insert(node);

        if let Some(deps) = graph.get(node) {
            for dep in deps {
                if self.has_cycle(dep, graph, visited, rec_stack)? {
                    return Ok(true);
                }
            }
        }

        rec_stack.remove(node);
        Ok(false)
    }

    fn validate_variable_references(&self) -> Result<()> {
        // Build set of step names
        let step_names: HashSet<_> = self.steps.iter().map(|s| s.name.as_str()).collect();

        // Build map of legacy extracts for backward compatibility
        let mut available_extracts: HashMap<String, HashSet<String>> = HashMap::new();
        for step in &self.steps {
            let extracts: HashSet<_> = step.extract.keys().cloned().collect();
            available_extracts.insert(step.name.clone(), extracts);
        }

        // Build input names
        let input_names: HashSet<_> = self.inputs.iter().map(|i| i.name.clone()).collect();

        // Variable patterns
        let var_pattern = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();

        for step in &self.steps {
            // Get foreach alias if present
            let foreach_alias = step.foreach.as_ref()
                .and_then(|f| ForeachClause::parse(f))
                .map(|c| c.alias);

            // Get all dependencies (explicit + implicit from foreach)
            let all_deps = self.get_all_dependencies(step);

            for cap in var_pattern.captures_iter(&step.query) {
                let var_ref = cap.get(1).unwrap().as_str().trim();

                // Parse the variable reference
                self.validate_single_reference(
                    var_ref,
                    &step.name,
                    &input_names,
                    &step_names,
                    &all_deps,
                    &available_extracts,
                    &foreach_alias,
                )?;
            }
        }

        Ok(())
    }

    /// Validate a single variable reference
    #[allow(clippy::too_many_arguments)]
    fn validate_single_reference(
        &self,
        var_ref: &str,
        step_name: &str,
        input_names: &HashSet<String>,
        step_names: &HashSet<&str>,
        all_deps: &[String],
        available_extracts: &HashMap<String, HashSet<String>>,
        foreach_alias: &Option<String>,
    ) -> Result<()> {
        // Pattern: inputs.name
        if let Some(input_name) = var_ref.strip_prefix("inputs.") {
            if !input_names.contains(input_name) {
                return Err(KqlPanopticonError::InvalidVariableReference(
                    format!("Step '{}' references undefined input '{}'", step_name, input_name),
                ));
            }
            return Ok(());
        }

        // Extract prefix (step name) from various patterns
        // Pattern: step.*.Column (array access)
        if let Some((prefix, _rest)) = var_ref.split_once(".*.") {
            return self.validate_step_reference(prefix, step_name, step_names, all_deps);
        }

        // Pattern: step.first.Column (first row access)
        if let Some((prefix, _rest)) = var_ref.split_once(".first.") {
            return self.validate_step_reference(prefix, step_name, step_names, all_deps);
        }

        // Pattern: step[N].Column (indexed access)
        if let Some(bracket_pos) = var_ref.find('[') {
            let prefix = &var_ref[..bracket_pos];
            // Verify it has closing bracket and dot
            if var_ref.contains("].") {
                return self.validate_step_reference(prefix, step_name, step_names, all_deps);
            }
        }

        // Check if this matches foreach alias: alias.Column
        if let Some(alias) = foreach_alias {
            if var_ref.starts_with(&format!("{}.", alias)) {
                // Valid foreach alias reference
                return Ok(());
            }
        }

        // Try parsing as step.something (legacy extract or direct column access)
        if let Some((prefix, rest)) = var_ref.split_once('.') {
            // Check if prefix is foreach alias
            if let Some(alias) = foreach_alias {
                if prefix == alias {
                    return Ok(());
                }
            }

            // Check if prefix looks like a step reference (alphanumeric/underscore)
            let looks_like_step = prefix.chars().all(|c| c.is_alphanumeric() || c == '_');

            if looks_like_step {
                // Check if step exists
                if !step_names.contains(prefix) {
                    return Err(KqlPanopticonError::InvalidVariableReference(
                        format!(
                            "Step '{}' references non-existent step '{}'",
                            step_name, prefix
                        ),
                    ));
                }

                // Validate it's a dependency
                if !all_deps.iter().any(|d| d == prefix) {
                    return Err(KqlPanopticonError::InvalidVariableReference(
                        format!(
                            "Step '{}' references '{}' but does not declare it in depends_on",
                            step_name, prefix
                        ),
                    ));
                }

                // Check for legacy extract syntax: step.extract_name
                if let Some(extracts) = available_extracts.get(prefix) {
                    if extracts.contains(rest) {
                        // Valid legacy extract reference
                        return Ok(());
                    }
                }

                // For new syntax, we assume column name is valid (can't validate at parse time)
                return Ok(());
            }
        }

        Err(KqlPanopticonError::InvalidVariableReference(
            format!(
                "Invalid variable reference '{{{{{}}}}}' in step '{}'. Use '{{{{inputs.name}}}}', '{{{{step.*.Column}}}}', or '{{{{alias.Column}}}}'",
                var_ref, step_name
            ),
        ))
    }

    /// Validate that a step reference is a valid dependency
    fn validate_step_reference(
        &self,
        step_ref: &str,
        current_step: &str,
        step_names: &HashSet<&str>,
        all_deps: &[String],
    ) -> Result<()> {
        if !step_names.contains(step_ref) {
            return Err(KqlPanopticonError::InvalidVariableReference(
                format!("Step '{}' references non-existent step '{}'", current_step, step_ref),
            ));
        }
        if !all_deps.iter().any(|d| d == step_ref) {
            return Err(KqlPanopticonError::InvalidVariableReference(
                format!(
                    "Step '{}' references '{}' but does not declare it in depends_on",
                    current_step, step_ref
                ),
            ));
        }
        Ok(())
    }

    fn validate_inputs(&self) -> Result<()> {
        let mut seen = HashSet::new();
        for input in &self.inputs {
            if input.name.is_empty() {
                return Err(KqlPanopticonError::InvestigationPackValidation(
                    "Input name cannot be empty".into(),
                ));
            }
            if !seen.insert(&input.name) {
                return Err(KqlPanopticonError::InvestigationPackValidation(
                    format!("Duplicate input name: '{}'", input.name),
                ));
            }
        }
        Ok(())
    }

    /// Get steps in execution order (topological sort)
    pub fn execution_order(&self) -> Result<Vec<&Step>> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();

        // Build step lookup
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
                return Err(KqlPanopticonError::CircularDependency(format!(
                    "Circular dependency at step '{}'",
                    step_name
                )));
            }

            temp_visited.insert(step_name.to_string());

            if let Some(step) = step_map.get(step_name) {
                for dep in &step.depends_on {
                    visit(dep, step_map, visited, temp_visited, result)?;
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

    /// Get the library path for investigation packs
    pub fn get_library_path(relative_path: &str) -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or(KqlPanopticonError::HomeDirectoryNotFound)?;

        Ok(home.join(".kql-panopticon/investigations").join(relative_path))
    }

    /// List all investigation packs in the library
    pub fn list_library_packs() -> Result<Vec<PathBuf>> {
        let packs_dir = Self::get_library_path("")?;

        if !packs_dir.exists() {
            std::fs::create_dir_all(&packs_dir)?;
            return Ok(vec![]);
        }

        let mut packs = Vec::new();

        for entry in walkdir::WalkDir::new(&packs_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                    if ext == "yaml" || ext == "yml" || ext == "json" {
                        // Verify it's actually an investigation pack by checking kind
                        if let Ok(content) = std::fs::read_to_string(entry.path()) {
                            let is_investigation = if ext == "json" {
                                serde_json::from_str::<serde_json::Value>(&content)
                                    .ok()
                                    .and_then(|v| v.get("kind")?.as_str().map(|s| s == "investigation"))
                                    .unwrap_or(false)
                            } else {
                                serde_yaml::from_str::<serde_yaml::Value>(&content)
                                    .ok()
                                    .and_then(|v| v.get("kind")?.as_str().map(|s| s == "investigation"))
                                    .unwrap_or(false)
                            };

                            if is_investigation {
                                packs.push(entry.path().to_path_buf());
                            }
                        }
                    }
                }
            }
        }

        Ok(packs)
    }

    /// Get summary info for display (without full validation)
    #[allow(dead_code)]
    pub fn summary(&self) -> InvestigationPackSummary {
        InvestigationPackSummary {
            name: self.name.clone(),
            description: self.description.clone(),
            version: self.version.clone(),
            step_count: self.steps.len(),
            input_count: self.inputs.len(),
        }
    }
}

/// Summary info for listing investigation packs
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InvestigationPackSummary {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub step_count: usize,
    pub input_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_minimal_investigation() {
        let yaml = r#"
kind: investigation
name: "Test Investigation"
steps:
  - name: first_step
    query: "SecurityEvent | limit 10"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(pack.name, "Test Investigation");
        assert_eq!(pack.steps.len(), 1);
        pack.validate().unwrap();
    }

    #[test]
    fn test_load_full_investigation() {
        let yaml = r#"
kind: investigation
name: "Malicious URL Investigation"
description: "Investigate phishing URL clicks"
version: "1.0"
output:
  folder: "./investigations/{{name}}/{{timestamp}}"
inputs:
  - name: malicious_url
    description: "The malicious URL to investigate"
    type: string
    required: true
  - name: lookback_days
    type: string
    default: "7"
steps:
  - name: url_clicks
    query: |
      UrlClickEvents
      | where Url contains "{{inputs.malicious_url}}"
    extract:
      users:
        column: UserPrincipalName
        type: array
        quote_style: single
        chunk_size: 500
        dedupe: true
  - name: related_emails
    depends_on:
      - url_clicks
    query: |
      EmailEvents
      | where RecipientEmailAddress in ({{url_clicks.users}})
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(pack.steps.len(), 2);
        assert_eq!(pack.inputs.len(), 2);
        pack.validate().unwrap();
    }

    #[test]
    fn test_invalid_kind() {
        let yaml = r#"
kind: query_pack
name: "Test"
steps:
  - name: step1
    query: "test"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        let result = pack.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid kind"));
    }

    #[test]
    fn test_duplicate_step_names() {
        let yaml = r#"
kind: investigation
name: "Test"
steps:
  - name: step1
    query: "test1"
  - name: step1
    query: "test2"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        let result = pack.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Duplicate step name"));
    }

    #[test]
    fn test_missing_dependency() {
        let yaml = r#"
kind: investigation
name: "Test"
steps:
  - name: step1
    depends_on:
      - nonexistent
    query: "test"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        let result = pack.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("non-existent step"));
    }

    #[test]
    fn test_circular_dependency() {
        let yaml = r#"
kind: investigation
name: "Test"
steps:
  - name: step1
    depends_on:
      - step2
    query: "test1"
  - name: step2
    depends_on:
      - step1
    query: "test2"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        let result = pack.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular dependency"));
    }

    #[test]
    fn test_invalid_variable_reference() {
        // Test referencing a non-existent step
        let yaml = r#"
kind: investigation
name: "Test"
steps:
  - name: step1
    query: "test | where x == {{step2.value}}"
    extract:
      value:
        column: col
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        let result = pack.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("non-existent step"));

        // Test referencing a step without declaring dependency
        let yaml2 = r#"
kind: investigation
name: "Test"
steps:
  - name: step1
    query: "test"
    extract:
      value:
        column: col
  - name: step2
    query: "test | where x == {{step1.value}}"
"#;
        let pack2: InvestigationPack = serde_yaml::from_str(yaml2).unwrap();
        let result2 = pack2.validate();
        assert!(result2.is_err());
        assert!(result2.unwrap_err().to_string().contains("does not declare it in depends_on"));
    }

    #[test]
    fn test_undefined_input_reference() {
        let yaml = r#"
kind: investigation
name: "Test"
steps:
  - name: step1
    query: "test | where x == {{inputs.undefined}}"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        let result = pack.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("undefined input"));
    }

    #[test]
    fn test_execution_order() {
        let yaml = r#"
kind: investigation
name: "Test"
steps:
  - name: step3
    depends_on:
      - step1
      - step2
    query: "test3"
  - name: step1
    query: "test1"
  - name: step2
    depends_on:
      - step1
    query: "test2"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        pack.validate().unwrap();

        let order = pack.execution_order().unwrap();
        let names: Vec<_> = order.iter().map(|s| s.name.as_str()).collect();

        // step1 must come before step2 and step3
        // step2 must come before step3
        let step1_idx = names.iter().position(|&n| n == "step1").unwrap();
        let step2_idx = names.iter().position(|&n| n == "step2").unwrap();
        let step3_idx = names.iter().position(|&n| n == "step3").unwrap();

        assert!(step1_idx < step2_idx);
        assert!(step1_idx < step3_idx);
        assert!(step2_idx < step3_idx);
    }

    #[test]
    fn test_quote_style_single() {
        let style = QuoteStyle::Single;
        assert_eq!(style.format_value("test"), "'test'");
        assert_eq!(style.format_value("O'Brien"), "'O''Brien'");
        assert_eq!(style.format_array(&["a".into(), "b".into()]), "'a','b'");
    }

    #[test]
    fn test_quote_style_double() {
        let style = QuoteStyle::Double;
        assert_eq!(style.format_value("test"), "\"test\"");
        assert_eq!(style.format_value("say \"hello\""), "\"say \\\"hello\\\"\"");
    }

    #[test]
    fn test_quote_style_verbatim() {
        let style = QuoteStyle::Verbatim;
        assert_eq!(style.format_value("test"), "@'test'");
        assert_eq!(style.format_value("path\\to\\file"), "@'path\\to\\file'");
        assert_eq!(style.format_value("it's"), "@'it''s'");
    }

    #[test]
    fn test_empty_steps_invalid() {
        let yaml = r#"
kind: investigation
name: "Test"
steps: []
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        let result = pack.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least one step"));
    }

    #[test]
    fn test_invalid_bare_variable() {
        let yaml = r#"
kind: investigation
name: "Test"
steps:
  - name: step1
    query: "test | where x == {{value}}"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        let result = pack.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid variable reference"));
    }

    #[test]
    fn test_foreach_syntax_valid() {
        let yaml = r#"
kind: investigation
name: "Test Foreach"
steps:
  - name: get_users
    query: "SigninLogs | distinct UserPrincipalName"
  - name: user_details
    foreach: "get_users as user"
    query: "SigninLogs | where UserPrincipalName == {{user.UserPrincipalName}}"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        pack.validate().unwrap();
    }

    #[test]
    fn test_foreach_syntax_invalid() {
        // Missing 'as' keyword
        let yaml = r#"
kind: investigation
name: "Test"
steps:
  - name: step1
    query: "test"
  - name: step2
    foreach: "step1 user"
    query: "test"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        let result = pack.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid foreach syntax"));
    }

    #[test]
    fn test_foreach_nonexistent_source() {
        let yaml = r#"
kind: investigation
name: "Test"
steps:
  - name: step1
    foreach: "nonexistent as item"
    query: "test"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        let result = pack.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("non-existent step"));
    }

    #[test]
    fn test_foreach_implicit_dependency() {
        // Foreach should imply depends_on
        let yaml = r#"
kind: investigation
name: "Test"
steps:
  - name: step1
    query: "test1"
  - name: step2
    foreach: "step1 as item"
    query: "test2 | where x == {{item.col}}"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        pack.validate().unwrap();

        // Check that execution order respects implicit dependency
        let order = pack.execution_order().unwrap();
        let names: Vec<_> = order.iter().map(|s| s.name.as_str()).collect();
        let step1_idx = names.iter().position(|&n| n == "step1").unwrap();
        let step2_idx = names.iter().position(|&n| n == "step2").unwrap();
        assert!(step1_idx < step2_idx);
    }

    #[test]
    fn test_new_variable_syntax_array() {
        let yaml = r#"
kind: investigation
name: "Test Array Syntax"
steps:
  - name: get_users
    query: "SigninLogs | distinct UserPrincipalName"
  - name: filter_users
    depends_on:
      - get_users
    query: "SigninLogs | where UserPrincipalName in ({{get_users.*.UserPrincipalName}})"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        pack.validate().unwrap();
    }

    #[test]
    fn test_new_variable_syntax_first() {
        let yaml = r#"
kind: investigation
name: "Test First Syntax"
steps:
  - name: get_config
    query: "ConfigTable | limit 1"
  - name: use_config
    depends_on:
      - get_config
    query: "SigninLogs | where setting == {{get_config.first.Value}}"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        pack.validate().unwrap();
    }

    #[test]
    fn test_new_variable_syntax_indexed() {
        let yaml = r#"
kind: investigation
name: "Test Indexed Syntax"
steps:
  - name: get_items
    query: "ItemTable | limit 5"
  - name: use_second
    depends_on:
      - get_items
    query: "DetailTable | where id == {{get_items[1].Id}}"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        pack.validate().unwrap();
    }

    #[test]
    fn test_foreach_with_options() {
        let yaml = r#"
kind: investigation
name: "Test Foreach Options"
steps:
  - name: get_users
    query: "SigninLogs | distinct UserPrincipalName"
  - name: user_details
    foreach: "get_users as user"
    batch_size: 10
    aggregate: append
    on_empty: skip
    options:
      quote_style: single
      dedupe: true
    query: "AuditLogs | where Actor == {{user.UserPrincipalName}}"
"#;
        let pack: InvestigationPack = serde_yaml::from_str(yaml).unwrap();
        pack.validate().unwrap();

        let step = &pack.steps[1];
        assert_eq!(step.batch_size, Some(10));
        assert_eq!(step.aggregate, Some(AggregateStrategy::Append));
        assert_eq!(step.on_empty, Some(OnEmpty::Skip));
        assert!(step.options.is_some());
    }

    #[test]
    fn test_foreach_clause_parse() {
        // Valid syntax
        let clause = ForeachClause::parse("step1 as item").unwrap();
        assert_eq!(clause.source_step, "step1");
        assert_eq!(clause.alias, "item");

        // Case insensitive 'as'
        let clause2 = ForeachClause::parse("step1 AS item").unwrap();
        assert_eq!(clause2.source_step, "step1");
        assert_eq!(clause2.alias, "item");

        // Invalid - missing 'as'
        assert!(ForeachClause::parse("step1 item").is_none());

        // Invalid - extra parts
        assert!(ForeachClause::parse("step1 as item extra").is_none());

        // Invalid - empty
        assert!(ForeachClause::parse("").is_none());
    }

    #[test]
    fn test_aggregate_strategies() {
        assert_eq!(AggregateStrategy::default(), AggregateStrategy::Append);

        // Test deserialization
        let yaml = "append";
        let agg: AggregateStrategy = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(agg, AggregateStrategy::Append);

        let yaml = "merge";
        let agg: AggregateStrategy = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(agg, AggregateStrategy::Merge);

        let yaml = "replace";
        let agg: AggregateStrategy = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(agg, AggregateStrategy::Replace);

        let yaml = "collect";
        let agg: AggregateStrategy = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(agg, AggregateStrategy::Collect);
    }
}
