//! Reporting step output types
//!
//! Defines the output structure for reporting steps.

use crate::pack::ReportFormat;
use std::path::PathBuf;
use std::time::Duration;

/// Output from a reporting step execution
#[derive(Debug, Clone)]
pub struct ReportingStepOutput {
    /// Report name
    name: String,

    /// Path where report was written
    path: PathBuf,

    /// Report format
    format: ReportFormat,

    /// Byte count
    byte_count: usize,

    /// Line count (for text formats)
    line_count: Option<usize>,

    /// Execution duration
    duration: Duration,
}

impl ReportingStepOutput {
    /// Create a new reporting output
    pub fn new(
        name: impl Into<String>,
        path: PathBuf,
        format: ReportFormat,
        byte_count: usize,
        line_count: Option<usize>,
        duration: Duration,
    ) -> Self {
        Self {
            name: name.into(),
            path,
            format,
            byte_count,
            line_count,
            duration,
        }
    }

    /// Get report name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get output path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Get report format
    pub fn format(&self) -> ReportFormat {
        self.format
    }

    /// Get byte count
    pub fn byte_count(&self) -> usize {
        self.byte_count
    }

    /// Get line count
    pub fn line_count(&self) -> Option<usize> {
        self.line_count
    }

    /// Get execution duration
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Get duration in milliseconds
    pub fn duration_ms(&self) -> u64 {
        self.duration.as_millis() as u64
    }
}

/// Summary of a generated report
#[derive(Debug, Clone)]
pub struct GeneratedReport {
    pub name: String,
    pub path: PathBuf,
    pub format: ReportFormat,
    pub byte_count: usize,
}

impl From<ReportingStepOutput> for GeneratedReport {
    fn from(output: ReportingStepOutput) -> Self {
        Self {
            name: output.name,
            path: output.path,
            format: output.format,
            byte_count: output.byte_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reporting_output() {
        let output = ReportingStepOutput::new(
            "summary",
            PathBuf::from("/tmp/summary.md"),
            ReportFormat::Markdown,
            1024,
            Some(50),
            Duration::from_millis(100),
        );

        assert_eq!(output.name(), "summary");
        assert_eq!(output.byte_count(), 1024);
        assert_eq!(output.line_count(), Some(50));
        assert_eq!(output.duration_ms(), 100);
    }

    #[test]
    fn test_generated_report_conversion() {
        let output = ReportingStepOutput::new(
            "report",
            PathBuf::from("/tmp/report.md"),
            ReportFormat::Markdown,
            512,
            None,
            Duration::from_millis(50),
        );

        let report: GeneratedReport = output.into();
        assert_eq!(report.name, "report");
        assert_eq!(report.byte_count, 512);
    }
}
