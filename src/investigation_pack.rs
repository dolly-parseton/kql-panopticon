use crate::error::{KqlPanopticonError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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

    /// User-provided input variables
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Input>,

    /// Investigation steps (queries with dependencies)
    pub steps: Vec<Step>,
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

/// A step in the investigation (a query with optional dependencies and extractions)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Unique step name (used for references like {{step_name.var}})
    pub name: String,

    /// The KQL query (may contain {{variable}} placeholders)
    pub query: String,

    /// Steps this step depends on (must complete first)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,

    /// Values to extract from query results
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extract: HashMap<String, Extract>,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum QuoteStyle {
    /// Single quotes: 'value' (escapes ' as '')
    Single,
    /// Double quotes: "value" (escapes " as \")
    Double,
    /// KQL verbatim string: @'value' (escapes ' as '')
    Verbatim,
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
        self.validate_dependencies_exist()?;
        self.validate_no_circular_dependencies()?;
        self.validate_variable_references()?;
        self.validate_inputs()?;
        Ok(())
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

    fn validate_dependencies_exist(&self) -> Result<()> {
        let step_names: HashSet<_> = self.steps.iter().map(|s| &s.name).collect();

        for step in &self.steps {
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
        }
        Ok(())
    }

    fn validate_no_circular_dependencies(&self) -> Result<()> {
        // Build adjacency list
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
        for step in &self.steps {
            graph.insert(&step.name, step.depends_on.iter().map(|s| s.as_str()).collect());
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
        // Build map of what each step provides
        let mut available_extracts: HashMap<String, HashSet<String>> = HashMap::new();
        for step in &self.steps {
            let extracts: HashSet<_> = step.extract.keys().cloned().collect();
            available_extracts.insert(step.name.clone(), extracts);
        }

        // Build input names
        let input_names: HashSet<_> = self.inputs.iter().map(|i| i.name.clone()).collect();

        // Check each step's query for variable references
        let var_pattern = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();

        for step in &self.steps {
            for cap in var_pattern.captures_iter(&step.query) {
                let var_ref = cap.get(1).unwrap().as_str().trim();

                if let Some((prefix, name)) = var_ref.split_once('.') {
                    if prefix == "inputs" {
                        // Check input exists
                        if !input_names.contains(name) {
                            return Err(KqlPanopticonError::InvalidVariableReference(
                                format!(
                                    "Step '{}' references undefined input '{}'",
                                    step.name, name
                                ),
                            ));
                        }
                    } else {
                        // Must be a step reference - check it's a dependency
                        if !step.depends_on.contains(&prefix.to_string()) {
                            return Err(KqlPanopticonError::InvalidVariableReference(
                                format!(
                                    "Step '{}' references '{}' but does not declare it in depends_on",
                                    step.name, prefix
                                ),
                            ));
                        }

                        // Check the extraction exists
                        if let Some(extracts) = available_extracts.get(prefix) {
                            if !extracts.contains(name) {
                                return Err(KqlPanopticonError::InvalidVariableReference(
                                    format!(
                                        "Step '{}' references '{{{{{}}}}}' but step '{}' does not extract '{}'",
                                        step.name, var_ref, prefix, name
                                    ),
                                ));
                            }
                        }
                    }
                } else {
                    return Err(KqlPanopticonError::InvalidVariableReference(
                        format!(
                            "Invalid variable reference '{{{{{}}}}}' in step '{}'. Use '{{{{inputs.name}}}}' or '{{{{step.name}}}}'",
                            var_ref, step.name
                        ),
                    ));
                }
            }
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
        assert!(result.unwrap_err().to_string().contains("does not declare it in depends_on"));
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
}
