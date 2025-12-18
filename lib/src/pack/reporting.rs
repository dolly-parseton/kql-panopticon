//! Reporting configuration for pack execution
//!
//! Reporting defines how results are presented after acquisition and processing.
//! Multiple reports can be generated from a single pack execution.

use serde::{Deserialize, Serialize};

/// Reporting configuration
///
/// Contains multiple report definitions that are generated after execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Reporting {
    /// Report definitions
    #[serde(default)]
    pub reports: Vec<ReportDefinition>,
}

impl Reporting {
    /// Create empty reporting config
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if reporting has any reports defined
    pub fn is_empty(&self) -> bool {
        self.reports.is_empty()
    }

    /// Get report by name
    pub fn get_report(&self, name: &str) -> Option<&ReportDefinition> {
        self.reports.iter().find(|r| r.name == name)
    }
}

/// A single report definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportDefinition {
    /// Report name (unique identifier)
    pub name: String,

    /// Output format
    #[serde(default)]
    pub format: ReportFormat,

    /// Output filename template (supports Tera: `report-{{timestamp}}.md`)
    #[serde(default)]
    pub output: Option<String>,

    /// Inline template content (Tera/Markdown)
    #[serde(default)]
    pub template: Option<String>,

    /// Path to template file
    #[serde(default)]
    pub template_file: Option<String>,

    /// Condition for generating this report
    /// e.g., "processing.risk_score.score > 30"
    #[serde(default)]
    pub when: Option<String>,
}

impl ReportDefinition {
    /// Get the output filename, with default based on name
    pub fn output_filename(&self) -> String {
        self.output
            .clone()
            .unwrap_or_else(|| format!("{}.md", self.name))
    }

    /// Check if this report has a template defined
    pub fn has_template(&self) -> bool {
        self.template.is_some() || self.template_file.is_some()
    }
}

/// Report output format
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportFormat {
    /// Markdown format (default)
    #[default]
    Markdown,
    /// HTML format
    Html,
    /// JSON format
    Json,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_reporting() {
        let rep = Reporting::new();
        assert!(rep.is_empty());
    }

    #[test]
    fn test_report_definition_deserialize() {
        let yaml = r#"
name: summary
format: markdown
output: "summary-{{timestamp}}.md"
when: "processing.risk_score.score > 0"
template: |
  # Summary Report
  Score: {{ processing.risk_score.score }}
"#;
        let report: ReportDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(report.name, "summary");
        assert_eq!(report.format, ReportFormat::Markdown);
        assert!(report.has_template());
        assert!(report.when.is_some());
    }

    #[test]
    fn test_multiple_reports() {
        let yaml = r##"
reports:
  - name: executive
    format: markdown
    template: "# Executive Summary"
  - name: technical
    format: markdown
    template_file: templates/technical.md
  - name: raw_data
    format: json
    output: data.json
"##;
        let reporting: Reporting = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(reporting.reports.len(), 3);
        assert!(reporting.get_report("executive").is_some());
        assert!(reporting.get_report("technical").is_some());
        assert!(reporting.get_report("raw_data").is_some());
    }
}
