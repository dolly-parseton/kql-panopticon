//! Pack definitions for KQL query execution
//!
//! A Pack is organized into three phases:
//! - **Acquisition**: Data collection via KQL queries, HTTP calls, or file reads
//! - **Processing**: Data transformation and analysis (e.g., scoring)
//! - **Reporting**: Result presentation with multiple report outputs
//!
//! Packs can be defined as:
//! - A single YAML file
//! - A folder with separate files for each phase

mod acquisition;
mod processing;
mod reporting;
mod types;
mod validation;

pub use acquisition::Acquisition;
pub use processing::{
    MatchedIndicator, Processing, ProcessingStep, ProcessingStepConfig, ScoringConfig,
    ScoringIndicator, ScoringResult, ScoringThreshold,
};
pub use reporting::{ReportDefinition, ReportFormat, Reporting};
pub use types::*;

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A pack definition with acquisition, processing, and reporting phases
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

    /// Data acquisition configuration
    #[serde(default)]
    pub acquisition: Acquisition,

    /// Data processing configuration
    #[serde(default)]
    pub processing: Option<Processing>,

    /// Report generation configuration
    #[serde(default)]
    pub reporting: Option<Reporting>,
}

impl Pack {
    /// Load a pack from a file or folder
    pub fn load(path: &Path) -> Result<Self> {
        if path.is_dir() {
            Self::load_from_folder(path)
        } else {
            Self::load_from_file(path)
        }
    }

    /// Load a pack from a single YAML/JSON file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;

        let pack: Pack = if path.extension().is_some_and(|e| e == "json") {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };

        pack.validate()?;
        Ok(pack)
    }

    /// Load a pack from a folder structure
    ///
    /// Expected structure:
    /// ```text
    /// pack-folder/
    /// ├── pack.yaml           # metadata (name, version, description)
    /// ├── acquisition.yaml    # or acquisition/ folder
    /// ├── processing.yaml     # optional, or processing/ folder
    /// └── reporting.yaml      # optional, or reporting/ folder
    /// ```
    pub fn load_from_folder(dir: &Path) -> Result<Self> {
        // Load metadata from pack.yaml
        let metadata_path = dir.join("pack.yaml");
        if !metadata_path.exists() {
            return Err(Error::pack(format!(
                "Pack folder missing pack.yaml: {}",
                dir.display()
            )));
        }

        let metadata_content = std::fs::read_to_string(&metadata_path)?;

        #[derive(Deserialize)]
        struct PackMetadata {
            name: String,
            #[serde(default)]
            description: Option<String>,
            #[serde(default)]
            version: Option<String>,
        }

        let metadata: PackMetadata = serde_yaml::from_str(&metadata_content)?;

        // Load acquisition
        let acquisition = Self::load_acquisition_from_folder(dir)?;

        // Load processing (optional)
        let processing = Self::load_processing_from_folder(dir)?;

        // Load reporting (optional)
        let reporting = Self::load_reporting_from_folder(dir)?;

        let pack = Pack {
            name: metadata.name,
            description: metadata.description,
            version: metadata.version,
            acquisition,
            processing,
            reporting,
        };

        pack.validate()?;
        Ok(pack)
    }

    /// Load acquisition from folder
    fn load_acquisition_from_folder(dir: &Path) -> Result<Acquisition> {
        let file_path = dir.join("acquisition.yaml");
        let folder_path = dir.join("acquisition");

        if file_path.exists() {
            let content = std::fs::read_to_string(&file_path)?;
            Ok(serde_yaml::from_str(&content)?)
        } else if folder_path.is_dir() {
            Self::load_acquisition_from_parts(&folder_path)
        } else {
            // Acquisition is required
            Err(Error::pack(format!(
                "Pack folder missing acquisition.yaml or acquisition/: {}",
                dir.display()
            )))
        }
    }

    /// Load acquisition from parts (inputs.yaml, secrets.yaml, steps/*.yaml)
    fn load_acquisition_from_parts(dir: &Path) -> Result<Acquisition> {
        let mut acquisition = Acquisition::new();

        // Load inputs.yaml
        let inputs_path = dir.join("inputs.yaml");
        if inputs_path.exists() {
            let content = std::fs::read_to_string(&inputs_path)?;
            acquisition.inputs = serde_yaml::from_str(&content)?;
        }

        // Load secrets.yaml
        let secrets_path = dir.join("secrets.yaml");
        if secrets_path.exists() {
            let content = std::fs::read_to_string(&secrets_path)?;
            acquisition.secrets = Some(serde_yaml::from_str(&content)?);
        }

        // Load output.yaml
        let output_path = dir.join("output.yaml");
        if output_path.exists() {
            let content = std::fs::read_to_string(&output_path)?;
            acquisition.output = Some(serde_yaml::from_str(&content)?);
        }

        // Load steps from steps/ folder
        let steps_dir = dir.join("steps");
        if steps_dir.is_dir() {
            let mut steps = Vec::new();
            let mut entries: Vec<_> = std::fs::read_dir(&steps_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|ext| ext == "yaml" || ext == "yml")
                })
                .collect();

            // Sort by filename for deterministic order
            entries.sort_by_key(|e| e.path());

            for entry in entries {
                let content = std::fs::read_to_string(entry.path())?;
                let step: Step = serde_yaml::from_str(&content)?;
                steps.push(step);
            }

            acquisition.steps = steps;
        }

        Ok(acquisition)
    }

    /// Load processing from folder (optional)
    fn load_processing_from_folder(dir: &Path) -> Result<Option<Processing>> {
        let file_path = dir.join("processing.yaml");
        let folder_path = dir.join("processing");

        if file_path.exists() {
            let content = std::fs::read_to_string(&file_path)?;
            // Processing file can be either a Processing struct or a Vec<ProcessingStep>
            let processing: Processing = serde_yaml::from_str(&content)?;
            Ok(Some(processing))
        } else if folder_path.is_dir() {
            // Load individual processing step files
            let mut steps = Vec::new();
            let mut entries: Vec<_> = std::fs::read_dir(&folder_path)?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|ext| ext == "yaml" || ext == "yml")
                })
                .collect();

            entries.sort_by_key(|e| e.path());

            for entry in entries {
                let content = std::fs::read_to_string(entry.path())?;
                let step: ProcessingStep = serde_yaml::from_str(&content)?;
                steps.push(step);
            }

            if steps.is_empty() {
                Ok(None)
            } else {
                Ok(Some(Processing { steps }))
            }
        } else {
            Ok(None)
        }
    }

    /// Load reporting from folder (optional)
    fn load_reporting_from_folder(dir: &Path) -> Result<Option<Reporting>> {
        let file_path = dir.join("reporting.yaml");
        let folder_path = dir.join("reporting");

        if file_path.exists() {
            let content = std::fs::read_to_string(&file_path)?;
            let reporting: Reporting = serde_yaml::from_str(&content)?;
            Ok(Some(reporting))
        } else if folder_path.is_dir() {
            // Load individual report definition files
            let mut reports = Vec::new();
            let mut entries: Vec<_> = std::fs::read_dir(&folder_path)?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|ext| ext == "yaml" || ext == "yml")
                })
                .collect();

            entries.sort_by_key(|e| e.path());

            for entry in entries {
                let content = std::fs::read_to_string(entry.path())?;
                let report: ReportDefinition = serde_yaml::from_str(&content)?;
                reports.push(report);
            }

            if reports.is_empty() {
                Ok(None)
            } else {
                Ok(Some(Reporting { reports }))
            }
        } else {
            Ok(None)
        }
    }

    /// Get acquisition steps in execution order (topological sort)
    pub fn execution_order(&self) -> Result<Vec<&Step>> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();
        let step_map: HashMap<_, _> = self
            .acquisition
            .steps
            .iter()
            .map(|s| (s.name.as_str(), s))
            .collect();

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
                return Err(Error::pack(format!(
                    "Circular dependency at step '{}'",
                    step_name
                )));
            }

            temp_visited.insert(step_name.to_string());

            if let Some(step) = step_map.get(step_name) {
                // Visit explicit dependencies
                for dep in &step.depends_on {
                    visit(dep, step_map, visited, temp_visited, result)?;
                }
                // Visit implicit foreach dependency
                if let Some(foreach) = &step.foreach {
                    if let Some(clause) = ForeachClause::parse(foreach) {
                        visit(&clause.source_step, step_map, visited, temp_visited, result)?;
                    }
                }
                visited.insert(step_name.to_string());
                result.push(step);
            }

            temp_visited.remove(step_name);
            Ok(())
        }

        for step in &self.acquisition.steps {
            visit(
                &step.name,
                &step_map,
                &mut visited,
                &mut temp_visited,
                &mut result,
            )?;
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

    /// Get acquisition step by name
    pub fn get_step(&self, name: &str) -> Option<&Step> {
        self.acquisition.get_step(name)
    }

    /// Get required inputs without defaults
    pub fn required_inputs(&self) -> Vec<&Input> {
        self.acquisition.required_inputs()
    }

    /// Check if pack has any dependencies between acquisition steps
    pub fn has_dependencies(&self) -> bool {
        self.acquisition.has_dependencies()
    }

    /// Check if pack has processing steps
    pub fn has_processing(&self) -> bool {
        self.processing
            .as_ref()
            .is_some_and(|p| !p.is_empty())
    }

    /// Check if pack has reporting definitions
    pub fn has_reporting(&self) -> bool {
        self.reporting
            .as_ref()
            .is_some_and(|r| !r.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_format_pack() {
        let yaml = r#"
name: "Security Check"
description: "Basic security analysis"
version: "1.0"

acquisition:
  inputs:
    - name: target_ip
      type: array
      required: true
  steps:
    - name: signins
      query: "SigninLogs | where IPAddress in ({{inputs.target_ip}})"

processing:
  steps:
    - name: risk_score
      type: scoring
      indicators:
        - name: high_count
          condition: "signins.length > 100"
          weight: 25
      thresholds:
        - level: HIGH
          min_score: 50

reporting:
  reports:
    - name: summary
      format: markdown
      template: |
        # Summary
        Score: {{ processing.risk_score.score }}
"#;
        let pack: Pack = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(pack.name, "Security Check");
        assert_eq!(pack.acquisition.inputs.len(), 1);
        assert_eq!(pack.acquisition.steps.len(), 1);
        assert!(pack.has_processing());
        assert!(pack.has_reporting());
    }

    #[test]
    fn test_minimal_pack() {
        let yaml = r#"
name: "Simple Query"
acquisition:
  steps:
    - name: data
      query: "Usage | take 10"
"#;
        let pack: Pack = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(pack.name, "Simple Query");
        assert!(!pack.has_processing());
        assert!(!pack.has_reporting());
    }
}
