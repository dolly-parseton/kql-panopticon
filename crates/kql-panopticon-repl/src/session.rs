//! Pack authoring session state
//!
//! Represents an in-progress investigation pack being built interactively.
//! The session maintains inputs (user parameters) and steps (query definitions)
//! that can be saved as a pack file.

use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Regex for parsing variable references like {{inputs.name}} or {{step.column}}
static REF_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{([a-zA-Z_][a-zA-Z0-9_]*(?:\.[a-zA-Z_*][a-zA-Z0-9_]*)*)\}\}").unwrap()
});

/// An interactive pack authoring session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackSession {
    /// Pack name
    pub name: Option<String>,

    /// Pack description
    pub description: Option<String>,

    /// User input parameters (ordered by definition)
    #[serde(default)]
    pub inputs: IndexMap<String, InputDef>,

    /// Query steps (ordered by definition)
    #[serde(default)]
    pub steps: IndexMap<String, StepDef>,
}

/// Definition of a user input parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDef {
    /// Input name
    pub name: String,

    /// Input type
    #[serde(default)]
    pub input_type: InputType,

    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,

    /// Whether this input is required
    #[serde(default = "default_true")]
    pub required: bool,

    /// Default value (if not required)
    #[serde(default)]
    pub default: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Supported input types
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    #[default]
    String,
    Int,
    Bool,
    Datetime,
    Timespan,
}

impl std::fmt::Display for InputType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputType::String => write!(f, "string"),
            InputType::Int => write!(f, "int"),
            InputType::Bool => write!(f, "bool"),
            InputType::Datetime => write!(f, "datetime"),
            InputType::Timespan => write!(f, "timespan"),
        }
    }
}

impl InputType {
    /// Generate a type-appropriate example value for validation
    pub fn example_value(&self, name: &str) -> String {
        match self {
            InputType::String => format!("example_{}", name),
            InputType::Int => "42".to_string(),
            InputType::Bool => "true".to_string(),
            InputType::Datetime => "datetime(2024-01-01T00:00:00Z)".to_string(),
            InputType::Timespan => "7d".to_string(),
        }
    }
}

/// Definition of a query step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDef {
    /// Step name
    pub name: String,

    /// KQL query
    pub query: String,

    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,

    /// Steps this step depends on (auto-detected from {{step.*}} references)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,

    /// Inputs referenced by this step (auto-detected from {{inputs.*}} references)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references_inputs: Vec<String>,
}

/// A parsed variable reference
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarRef {
    /// Reference to an input: {{inputs.name}}
    Input(String),
    /// Reference to a step's output: {{step_name}} or {{step_name.column}}
    Step { step: String, accessor: Option<String> },
}

impl PackSession {
    /// Create a new empty session
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new session with a name
    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::default()
        }
    }

    /// Check if the session is empty (no inputs or steps)
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty() && self.steps.is_empty()
    }

    /// Add or update an input
    pub fn add_input(&mut self, input: InputDef) {
        self.inputs.insert(input.name.clone(), input);
    }

    /// Add or update a step, auto-detecting dependencies
    pub fn add_step(&mut self, mut step: StepDef) {
        // Parse references from the query
        let refs = parse_references(&step.query);

        // Extract input references
        step.references_inputs = refs
            .iter()
            .filter_map(|r| match r {
                VarRef::Input(name) => Some(name.clone()),
                _ => None,
            })
            .collect();

        // Extract step dependencies
        step.depends_on = refs
            .iter()
            .filter_map(|r| match r {
                VarRef::Step { step, .. } => Some(step.clone()),
                _ => None,
            })
            .collect();

        self.steps.insert(step.name.clone(), step);
    }

    /// Remove an input by name
    pub fn remove_input(&mut self, name: &str) -> Option<InputDef> {
        self.inputs.shift_remove(name)
    }

    /// Remove a step by name
    pub fn remove_step(&mut self, name: &str) -> Option<StepDef> {
        self.steps.shift_remove(name)
    }

    /// Get an input by name
    #[allow(dead_code)]
    pub fn get_input(&self, name: &str) -> Option<&InputDef> {
        self.inputs.get(name)
    }

    /// Get a step by name
    pub fn get_step(&self, name: &str) -> Option<&StepDef> {
        self.steps.get(name)
    }

    /// Check if a name exists (as input or step)
    #[allow(dead_code)]
    pub fn name_exists(&self, name: &str) -> bool {
        self.inputs.contains_key(name) || self.steps.contains_key(name)
    }

    /// Validate references in a query against defined inputs and steps
    pub fn validate_references(&self, query: &str) -> Result<(), Vec<String>> {
        let refs = parse_references(query);
        let mut errors = Vec::new();

        for r in refs {
            match r {
                VarRef::Input(name) => {
                    if !self.inputs.contains_key(&name) {
                        errors.push(format!("Unknown input: {}", name));
                    }
                }
                VarRef::Step { step, .. } => {
                    if !self.steps.contains_key(&step) {
                        errors.push(format!("Unknown step: {}", step));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Prepare a query for KQL validation by substituting references with example values
    ///
    /// This allows the KQL validator to check syntax without needing actual runtime values.
    /// - Input references are replaced with type-appropriate example values
    /// - Step references are replaced with placeholder subqueries
    pub fn prepare_query_for_validation(&self, query: &str) -> String {
        let refs = parse_references(query);
        let mut result = query.to_string();

        for var_ref in refs {
            let (full_match, replacement) = match &var_ref {
                VarRef::Input(name) => {
                    let example = self
                        .inputs
                        .get(name)
                        .map(|input| {
                            // Use default value if available, otherwise generate example
                            input.default.clone().unwrap_or_else(|| {
                                input.input_type.example_value(name)
                            })
                        })
                        .unwrap_or_else(|| format!("placeholder_{}", name));
                    (format!("{{{{inputs.{}}}}}", name), example)
                }
                VarRef::Step { step, accessor } => {
                    // Generate a placeholder subquery that's syntactically valid KQL
                    let placeholder = match accessor.as_deref() {
                        // {{step}} - entire step result (used as subquery)
                        None => format!("(print placeholder_{} = 'value')", step),
                        // {{step.first.column}} or {{step.*.column}} - scalar or array value
                        Some(acc) if acc.starts_with("first.") || acc.contains('*') => {
                            "'placeholder_value'".to_string()
                        }
                        // {{step.column}} - column access (likely in foreach or similar)
                        Some(_) => "'placeholder_value'".to_string(),
                    };
                    let full = match accessor {
                        Some(acc) => format!("{{{{{}.{}}}}}", step, acc),
                        None => format!("{{{{{}}}}}", step),
                    };
                    (full, placeholder)
                }
            };
            result = result.replace(&full_match, &replacement);
        }

        result
    }

    /// Clear the session
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.name = None;
        self.description = None;
        self.inputs.clear();
        self.steps.clear();
    }
}

/// Parse variable references from a query string
pub fn parse_references(query: &str) -> Vec<VarRef> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();

    for cap in REF_PATTERN.captures_iter(query) {
        let full_ref = &cap[1];

        // Skip duplicates
        if seen.contains(full_ref) {
            continue;
        }
        seen.insert(full_ref.to_string());

        let parts: Vec<&str> = full_ref.split('.').collect();

        match parts.as_slice() {
            ["inputs", name] => {
                refs.push(VarRef::Input((*name).to_string()));
            }
            [step_name] => {
                // Just {{step_name}} - reference to entire step output
                refs.push(VarRef::Step {
                    step: (*step_name).to_string(),
                    accessor: None,
                });
            }
            [step_name, _accessor, ..] => {
                // {{step_name.column}} or {{step_name.first.column}}
                refs.push(VarRef::Step {
                    step: (*step_name).to_string(),
                    accessor: Some(parts[1..].join(".")),
                });
            }
            _ => {
                // Ignore malformed references
            }
        }
    }

    refs
}

impl InputDef {
    /// Create a new string input with just a value
    #[allow(dead_code)]
    pub fn new_string(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            input_type: InputType::String,
            description: None,
            required: false,
            default: Some(value.into()),
        }
    }

    /// Create a new required input
    #[allow(dead_code)]
    pub fn new_required(name: impl Into<String>, input_type: InputType) -> Self {
        Self {
            name: name.into(),
            input_type,
            description: None,
            required: true,
            default: None,
        }
    }
}

impl StepDef {
    /// Create a new step with just a query
    pub fn new(name: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            query: query.into(),
            description: None,
            depends_on: Vec::new(),
            references_inputs: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_input_reference() {
        let refs = parse_references("SecurityEvent | where IP == '{{inputs.threat_ip}}'");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0], VarRef::Input("threat_ip".to_string()));
    }

    #[test]
    fn test_parse_step_reference() {
        let refs = parse_references("{{events}} | summarize count()");
        assert_eq!(refs.len(), 1);
        assert_eq!(
            refs[0],
            VarRef::Step {
                step: "events".to_string(),
                accessor: None
            }
        );
    }

    #[test]
    fn test_parse_step_with_accessor() {
        let refs = parse_references("T | where IP in ({{events.first.IpAddress}})");
        assert_eq!(refs.len(), 1);
        assert_eq!(
            refs[0],
            VarRef::Step {
                step: "events".to_string(),
                accessor: Some("first.IpAddress".to_string())
            }
        );
    }

    #[test]
    fn test_parse_multiple_references() {
        let refs = parse_references(
            "{{base_events}} | where IP == '{{inputs.ip}}' | join ({{other_step}})",
        );
        assert_eq!(refs.len(), 3);
    }

    #[test]
    fn test_add_step_auto_detects_deps() {
        let mut session = PackSession::new();

        // Add an input first
        session.add_input(InputDef::new_string("ip", "10.0.0.1"));

        // Add first step
        session.add_step(StepDef::new("events", "SecurityEvent | take 10"));

        // Add step that depends on both
        session.add_step(StepDef::new(
            "filtered",
            "{{events}} | where IP == '{{inputs.ip}}'",
        ));

        let step = session.get_step("filtered").unwrap();
        assert_eq!(step.depends_on, vec!["events"]);
        assert_eq!(step.references_inputs, vec!["ip"]);
    }

    #[test]
    fn test_validate_references() {
        let mut session = PackSession::new();
        session.add_input(InputDef::new_string("ip", "10.0.0.1"));
        session.add_step(StepDef::new("events", "T | take 10"));

        // Valid references
        assert!(session
            .validate_references("{{events}} | where IP == '{{inputs.ip}}'")
            .is_ok());

        // Invalid step reference
        let err = session.validate_references("{{nonexistent}}").unwrap_err();
        assert!(err[0].contains("Unknown step"));

        // Invalid input reference
        let err = session
            .validate_references("{{inputs.missing}}")
            .unwrap_err();
        assert!(err[0].contains("Unknown input"));
    }

    #[test]
    fn test_session_name_exists() {
        let mut session = PackSession::new();
        session.add_input(InputDef::new_string("my_input", "value"));
        session.add_step(StepDef::new("my_step", "T"));

        assert!(session.name_exists("my_input"));
        assert!(session.name_exists("my_step"));
        assert!(!session.name_exists("other"));
    }

    #[test]
    fn test_input_type_example_values() {
        assert_eq!(InputType::String.example_value("foo"), "example_foo");
        assert_eq!(InputType::Int.example_value("count"), "42");
        assert_eq!(InputType::Bool.example_value("flag"), "true");
        assert!(InputType::Datetime.example_value("ts").contains("datetime"));
        assert_eq!(InputType::Timespan.example_value("dur"), "7d");
    }

    #[test]
    fn test_prepare_query_for_validation_with_input() {
        let mut session = PackSession::new();
        session.add_input(InputDef::new_string("ip", "10.0.0.1"));

        let query = "SecurityEvent | where IpAddress == '{{inputs.ip}}'";
        let prepared = session.prepare_query_for_validation(query);

        // Should substitute with the default value
        assert!(prepared.contains("10.0.0.1"));
        assert!(!prepared.contains("{{inputs.ip}}"));
    }

    #[test]
    fn test_prepare_query_for_validation_with_step() {
        let mut session = PackSession::new();
        session.add_step(StepDef::new("events", "SecurityEvent | take 10"));

        let query = "{{events}} | summarize count()";
        let prepared = session.prepare_query_for_validation(query);

        // Should substitute with placeholder subquery
        assert!(prepared.contains("print placeholder_events"));
        assert!(!prepared.contains("{{events}}"));
    }

    #[test]
    fn test_prepare_query_for_validation_with_step_accessor() {
        let mut session = PackSession::new();
        session.add_step(StepDef::new("events", "SecurityEvent | take 10"));

        let query = "T | where IP == '{{events.first.IpAddress}}'";
        let prepared = session.prepare_query_for_validation(query);

        // Should substitute with placeholder value
        assert!(prepared.contains("placeholder_value"));
        assert!(!prepared.contains("{{events.first.IpAddress}}"));
    }

    #[test]
    fn test_prepare_query_for_validation_no_refs() {
        let session = PackSession::new();
        let query = "SecurityEvent | take 10";
        let prepared = session.prepare_query_for_validation(query);

        assert_eq!(prepared, query);
    }
}
