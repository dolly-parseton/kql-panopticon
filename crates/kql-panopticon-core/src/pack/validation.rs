//! Pack validation logic
//!
//! Validates pack structure, dependencies, and step configurations.

use super::types::{ForeachClause, Step, StepType};
use super::Pack;
use crate::error::{Error, Result};
use std::collections::{HashMap, HashSet};

impl Pack {
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

    pub(super) fn validate_name(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(Error::pack("Pack name cannot be empty"));
        }
        Ok(())
    }

    pub(super) fn validate_steps_not_empty(&self) -> Result<()> {
        if self.steps.is_empty() {
            return Err(Error::pack("Pack must have at least one step"));
        }
        Ok(())
    }

    pub(super) fn validate_step_names_unique(&self) -> Result<()> {
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

    pub(super) fn validate_step_types(&self) -> Result<()> {
        for step in &self.steps {
            validate_step_type(step)?;
        }
        Ok(())
    }

    pub(super) fn validate_foreach_syntax(&self) -> Result<()> {
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

    pub(super) fn validate_dependencies_exist(&self) -> Result<()> {
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

    pub(super) fn validate_no_circular_dependencies(&self) -> Result<()> {
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
            if has_cycle(&step.name, &graph, &mut visited, &mut rec_stack) {
                return Err(Error::pack(format!(
                    "Circular dependency detected involving step '{}'",
                    step.name
                )));
            }
        }
        Ok(())
    }

    pub(super) fn validate_inputs(&self) -> Result<()> {
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
}

/// Validate step type configuration
fn validate_step_type(step: &Step) -> Result<()> {
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
            if step.source.is_some() {
                return Err(Error::pack(format!(
                    "KQL step '{}' should not have 'source'",
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
            if step.source.is_some() {
                return Err(Error::pack(format!(
                    "HTTP step '{}' should not have 'source'",
                    step.name
                )));
            }
        }
        StepType::File => {
            if step.source.is_none() {
                return Err(Error::pack(format!(
                    "File step '{}' must have 'source'",
                    step.name
                )));
            }
            if let Some(source) = &step.source {
                if source.path.trim().is_empty() {
                    return Err(Error::pack(format!(
                        "File step '{}' has empty path",
                        step.name
                    )));
                }
            }
            if step.query.as_ref().map_or(false, |q| !q.is_empty()) {
                return Err(Error::pack(format!(
                    "File step '{}' should not have 'query'",
                    step.name
                )));
            }
            if step.request.is_some() {
                return Err(Error::pack(format!(
                    "File step '{}' should not have 'request'",
                    step.name
                )));
            }
        }
    }
    Ok(())
}

/// Check for cycles in dependency graph
fn has_cycle<'a>(
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
            if has_cycle(dep, graph, visited, rec_stack) {
                return true;
            }
        }
    }

    rec_stack.remove(node);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

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
