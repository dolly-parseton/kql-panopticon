//! Template step execution handler
//!
//! Generates reports using Tera templates with lazy data materialization.

use crate::error::{Error, Result};
use crate::execution::reporting::{
    ReportingContext, ReportingStepHandler, ReportingStepOutput, ReportingStepType,
};
use crate::pack::{ReportDefinition, ReportFormat};
use async_trait::async_trait;
use std::error::Error as StdError;
use std::time::Instant;
use tera::{Context, Tera};
use tracing::{debug, error};

/// Handler for template-based report generation
pub struct TemplateStepHandler;

impl TemplateStepHandler {
    /// Create a new template handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for TemplateStepHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ReportingStepHandler for TemplateStepHandler {
    fn handles(&self) -> ReportingStepType {
        ReportingStepType::Template
    }

    async fn execute(
        &self,
        report: &ReportDefinition,
        ctx: &ReportingContext<'_>,
    ) -> Result<ReportingStepOutput> {
        let start = Instant::now();

        debug!(
            "Generating report '{}' (format: {:?})",
            report.name, report.format
        );

        // Build template data just-in-time (lazy materialization)
        let template_data = ctx.build_template_data()?;

        // Build Tera context from materialized template_data
        let mut tera_ctx = Context::new();
        for (key, value) in &template_data {
            tera_ctx.insert(key, value);
        }

        // Get template content
        let template_content = get_template_content(report, ctx)?;

        // Render template
        let rendered = render_template(&report.name, &template_content, &tera_ctx)?;

        // Determine output path
        let output_filename = resolve_output_filename(report, &tera_ctx)?;
        let output_path = ctx.output_dir().join(&output_filename);

        // Ensure parent directory exists
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                Error::execution(format!(
                    "Failed to create report directory {:?}: {}",
                    parent, e
                ))
            })?;
        }

        // Write report
        let byte_count = rendered.len();
        let line_count = rendered.lines().count();

        tokio::fs::write(&output_path, &rendered).await.map_err(|e| {
            Error::execution(format!(
                "Failed to write report '{}' to {:?}: {}",
                report.name, output_path, e
            ))
        })?;

        debug!(
            "Report '{}' generated: {} bytes, {} lines at {:?}",
            report.name, byte_count, line_count, output_path
        );

        Ok(ReportingStepOutput::new(
            &report.name,
            output_path,
            report.format,
            byte_count,
            Some(line_count),
            start.elapsed(),
        ))
    }

    fn validate(&self, report: &ReportDefinition) -> Result<()> {
        // Name must not be empty
        if report.name.trim().is_empty() {
            return Err(Error::pack("Report name cannot be empty".to_string()));
        }

        Ok(())
    }
}

/// Get template content from inline template or file
fn get_template_content(report: &ReportDefinition, ctx: &ReportingContext<'_>) -> Result<String> {
    // Inline template takes precedence
    if let Some(template) = &report.template {
        return Ok(template.clone());
    }

    // Load from template file
    if let Some(template_file) = &report.template_file {
        let template_path = ctx.resolve_template_path(template_file);

        std::fs::read_to_string(&template_path).map_err(|e| {
            Error::execution(format!(
                "Failed to read template file {:?} for report '{}': {}",
                template_path, report.name, e
            ))
        })
    } else {
        // Generate default template based on format
        Ok(default_template(report))
    }
}

/// Generate a default template based on report format
fn default_template(report: &ReportDefinition) -> String {
    match report.format {
        ReportFormat::Json => "{{ __context | json_encode(pretty=true) }}".to_string(),
        ReportFormat::Markdown => {
            format!(
                r#"# Report: {}

## Summary

Generated at: {{{{ meta.timestamp }}}}

## Data

{{% for step_name, rows in acquisition %}}
### {{{{ step_name }}}}

{{{{ rows | length }}}} row(s)
{{% endfor %}}
"#,
                report.name
            )
        }
        ReportFormat::Html => {
            format!(
                r#"<!DOCTYPE html>
<html>
<head><title>Report: {}</title></head>
<body>
<h1>Report: {}</h1>
<p>Generated at: {{{{ meta.timestamp }}}}</p>
</body>
</html>"#,
                report.name, report.name
            )
        }
    }
}

/// Render template with Tera
fn render_template(report_name: &str, template: &str, context: &Context) -> Result<String> {
    let mut tera = Tera::default();

    // Note: Tera already fails on undefined variables by default
    // We add detailed error logging to make issues visible

    // Register the template
    tera.add_raw_template("report", template).map_err(|e| {
        let msg = format!(
            "Invalid template syntax for report '{}': {}",
            report_name, e
        );
        error!("{}", msg);
        Error::execution(msg)
    })?;

    // Render with detailed error handling
    tera.render("report", context).map_err(|e| {
        // Build full error chain for detailed diagnostics
        let mut error_parts = vec![e.to_string()];
        let mut source: Option<&(dyn StdError + 'static)> = e.source();
        while let Some(s) = source {
            error_parts.push(s.to_string());
            source = s.source();
        }
        let error_detail = error_parts.join(" -> ");

        // Check for common issues and provide helpful messages
        let hint = if error_detail.contains("not found in context")
            || error_detail.contains("__tera_one_off")
        {
            "\n  Hint: Check that the variable exists in the template context. Available top-level keys: meta, inputs, acquisition, processing, and step names directly (e.g., signins, alerts)."
        } else if error_detail.contains("is not defined") {
            "\n  Hint: The referenced variable or filter is not available. Check spelling and ensure data was collected."
        } else {
            ""
        };

        let msg = format!(
            "Template render failed for report '{}': {}{}",
            report_name, error_detail, hint
        );
        error!("{}", msg);
        Error::execution(msg)
    })
}

/// Resolve output filename, applying any template variables
fn resolve_output_filename(report: &ReportDefinition, context: &Context) -> Result<String> {
    let filename_template = report.output.clone().unwrap_or_else(|| {
        let extension = match report.format {
            ReportFormat::Markdown => "md",
            ReportFormat::Html => "html",
            ReportFormat::Json => "json",
        };
        format!("{}.{}", report.name, extension)
    });

    // If filename contains Tera syntax, render it
    if filename_template.contains("{{") || filename_template.contains("{%") {
        let mut tera = Tera::default();
        tera.add_raw_template("filename", &filename_template)
            .map_err(|e| {
                Error::execution(format!("Invalid output filename template: {}", e))
            })?;
        tera.render("filename", context).map_err(|e| {
            Error::execution(format!("Failed to render output filename: {}", e))
        })
    } else {
        Ok(filename_template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_default_template_json() {
        let report = ReportDefinition {
            name: "test".to_string(),
            format: ReportFormat::Json,
            output: None,
            template: None,
            template_file: None,
            when: None,
        };

        let template = default_template(&report);
        assert!(template.contains("json_encode"));
    }

    #[test]
    fn test_default_template_markdown() {
        let report = ReportDefinition {
            name: "test".to_string(),
            format: ReportFormat::Markdown,
            output: None,
            template: None,
            template_file: None,
            when: None,
        };

        let template = default_template(&report);
        assert!(template.contains("# Report:"));
    }

    #[test]
    fn test_render_simple_template() {
        let template = "Hello, {{ name }}!";
        let mut context = Context::new();
        context.insert("name", "World");

        let result = render_template("test", template, &context).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_render_with_data() {
        let template = r#"Score: {{ processing.risk_score.score }}
Level: {{ processing.risk_score.level }}"#;

        let mut context = Context::new();
        let mut processing = HashMap::new();
        processing.insert(
            "risk_score".to_string(),
            serde_json::json!({
                "score": 75,
                "level": "HIGH"
            }),
        );
        context.insert("processing", &processing);

        let result = render_template("test", template, &context).unwrap();
        assert!(result.contains("Score: 75"));
        assert!(result.contains("Level: HIGH"));
    }

    #[test]
    fn test_resolve_output_filename_default() {
        let report = ReportDefinition {
            name: "summary".to_string(),
            format: ReportFormat::Markdown,
            output: None,
            template: None,
            template_file: None,
            when: None,
        };
        let context = Context::new();

        let filename = resolve_output_filename(&report, &context).unwrap();
        assert_eq!(filename, "summary.md");
    }

    #[test]
    fn test_resolve_output_filename_custom() {
        let report = ReportDefinition {
            name: "summary".to_string(),
            format: ReportFormat::Markdown,
            output: Some("reports/my-report.md".to_string()),
            template: None,
            template_file: None,
            when: None,
        };
        let context = Context::new();

        let filename = resolve_output_filename(&report, &context).unwrap();
        assert_eq!(filename, "reports/my-report.md");
    }

    #[test]
    fn test_validate_report() {
        let handler = TemplateStepHandler::new();

        // Valid report
        let valid = ReportDefinition {
            name: "test".to_string(),
            format: ReportFormat::Markdown,
            output: None,
            template: Some("# Test".to_string()),
            template_file: None,
            when: None,
        };
        assert!(handler.validate(&valid).is_ok());

        // Empty name
        let empty_name = ReportDefinition {
            name: "".to_string(),
            format: ReportFormat::Markdown,
            output: None,
            template: None,
            template_file: None,
            when: None,
        };
        assert!(handler.validate(&empty_name).is_err());
    }
}
