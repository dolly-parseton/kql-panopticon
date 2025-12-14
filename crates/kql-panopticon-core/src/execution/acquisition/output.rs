//! Acquisition step output types
//!
//! Defines the output structure for acquisition steps.

use crate::execution::result::ResultHandle;
use std::path::PathBuf;
use std::time::Duration;

/// Output from an acquisition step execution
#[derive(Debug, Clone)]
pub struct AcquisitionStepOutput {
    /// Handle to the result file
    handle: ResultHandle,

    /// Number of rows produced
    row_count: usize,

    /// Execution duration
    duration: Duration,

    /// Path to the output file
    output_path: Option<PathBuf>,
}

impl AcquisitionStepOutput {
    /// Create output from a result handle
    pub fn new(handle: ResultHandle, duration: Duration) -> Self {
        let row_count = handle.row_count().unwrap_or(0);
        let output_path = if handle.exists() {
            Some(handle.path().to_path_buf())
        } else {
            None
        };

        Self {
            handle,
            row_count,
            duration,
            output_path,
        }
    }

    /// Create an empty output (for skipped steps)
    pub fn empty(step_name: &str, duration: Duration) -> Self {
        Self {
            handle: ResultHandle::empty(step_name),
            row_count: 0,
            duration,
            output_path: None,
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

    /// Get row count
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    /// Get execution duration
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Get duration in milliseconds
    pub fn duration_ms(&self) -> u64 {
        self.duration.as_millis() as u64
    }

    /// Get output file path
    pub fn output_path(&self) -> Option<&PathBuf> {
        self.output_path.as_ref()
    }

    /// Check if output is empty
    pub fn is_empty(&self) -> bool {
        self.row_count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_output() {
        let output = AcquisitionStepOutput::empty("test", Duration::from_millis(100));
        assert!(output.is_empty());
        assert_eq!(output.row_count(), 0);
        assert_eq!(output.duration_ms(), 100);
        assert!(output.output_path().is_none());
    }
}
