//! Processing step output types
//!
//! Defines the output structure for processing steps using file-backed ResultHandle.

use crate::execution::result::ResultHandle;
use std::time::Duration;

/// Output from a processing step execution
#[derive(Debug, Clone)]
pub struct ProcessingStepOutput {
    /// Handle to the processing step's output file
    handle: ResultHandle,

    /// Execution duration
    duration: Duration,
}

impl ProcessingStepOutput {
    /// Create output from a result handle
    pub fn new(handle: ResultHandle, duration: Duration) -> Self {
        Self { handle, duration }
    }

    /// Create an empty output (for skipped steps)
    pub fn empty(step_name: &str, duration: Duration) -> Self {
        Self {
            handle: ResultHandle::empty(step_name),
            duration,
        }
    }

    /// Get the result handle
    pub fn handle(&self) -> &ResultHandle {
        &self.handle
    }

    /// Take ownership of the result handle
    pub fn into_handle(self) -> ResultHandle {
        self.handle
    }

    /// Get execution duration
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Get duration in milliseconds
    pub fn duration_ms(&self) -> u64 {
        self.duration.as_millis() as u64
    }

    /// Check if output is empty (no rows written)
    pub fn is_empty(&self) -> bool {
        self.handle.row_count().unwrap_or(0) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::result::ResultWriter;
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("kql-panopticon-test")
            .join("processing_output")
            .join(name)
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_processing_output() {
        let path = temp_path("test_output.jsonl");
        cleanup(&path);

        // Create output with a writer
        let mut writer = ResultWriter::new(&path, "test_step").unwrap();
        writer
            .write_row(&serde_json::json!({"score": 75, "level": "HIGH"}))
            .unwrap();
        let handle = writer.finish().unwrap();

        let output = ProcessingStepOutput::new(handle, Duration::from_millis(100));

        assert!(!output.is_empty());
        assert_eq!(output.duration_ms(), 100);
        assert_eq!(output.handle().row_count().unwrap(), 1);

        cleanup(&path);
    }

    #[test]
    fn test_empty_output() {
        let output = ProcessingStepOutput::empty("skipped_step", Duration::from_millis(50));
        assert!(output.is_empty());
        assert_eq!(output.duration_ms(), 50);
    }
}
