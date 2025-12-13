//! Event log with in-memory storage and file persistence
//!
//! Stores timestamped events during a session and can flush to disk on shutdown.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use super::ContextEvent;

/// Configuration for the event log
#[derive(Debug, Clone)]
pub struct EventLogConfig {
    /// Maximum number of events to keep in memory
    pub max_memory_events: usize,
    /// Whether to auto-flush to file when memory limit reached
    pub auto_flush: bool,
    /// Directory for log files (if auto_flush enabled)
    pub log_directory: Option<String>,
}

impl Default for EventLogConfig {
    fn default() -> Self {
        Self {
            max_memory_events: 10000,
            auto_flush: false,
            log_directory: None,
        }
    }
}

/// A timestamped event entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampedEvent {
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// The event data
    pub event: ContextEvent,
}

impl TimestampedEvent {
    /// Create a new timestamped event with current time
    pub fn now(event: ContextEvent) -> Self {
        Self {
            timestamp: Utc::now(),
            event,
        }
    }
}

/// In-memory event log with optional file persistence
#[derive(Debug)]
pub struct EventLog {
    /// Events stored in memory
    events: Vec<TimestampedEvent>,
    /// Configuration
    config: EventLogConfig,
    /// Number of events flushed to file (for continuation)
    flushed_count: usize,
}

impl EventLog {
    /// Create a new event log with default configuration
    pub fn new() -> Self {
        Self::with_config(EventLogConfig::default())
    }

    /// Create a new event log with custom configuration
    pub fn with_config(config: EventLogConfig) -> Self {
        Self {
            events: Vec::new(),
            config,
            flushed_count: 0,
        }
    }

    /// Log an event
    pub fn log(&mut self, event: ContextEvent) {
        let timestamped = TimestampedEvent::now(event);

        // Check if we need to auto-flush before adding
        if self.config.auto_flush
            && self.events.len() >= self.config.max_memory_events
            && self.config.log_directory.is_some()
        {
            // Auto-flush to temporary file
            if let Some(ref dir) = self.config.log_directory {
                let filename = format!("{}/events_{}.jsonl", dir, Utc::now().format("%Y%m%d_%H%M%S"));
                let _ = self.flush_to_file(&filename);
            }
        }

        self.events.push(timestamped);

        // Trim if over limit and auto-flush is disabled
        if !self.config.auto_flush && self.events.len() > self.config.max_memory_events {
            // Remove oldest events
            let overflow = self.events.len() - self.config.max_memory_events;
            self.events.drain(0..overflow);
            self.flushed_count += overflow;
        }
    }

    /// Get all events in memory
    pub fn events(&self) -> &[TimestampedEvent] {
        &self.events
    }

    /// Get the total event count (including flushed)
    pub fn total_count(&self) -> usize {
        self.flushed_count + self.events.len()
    }

    /// Get events for a specific job
    pub fn events_for_job(&self, job_id: uuid::Uuid) -> Vec<&TimestampedEvent> {
        self.events
            .iter()
            .filter(|e| e.event.job_id() == Some(job_id))
            .collect()
    }

    /// Get recent events (last N)
    pub fn recent(&self, count: usize) -> &[TimestampedEvent] {
        let start = self.events.len().saturating_sub(count);
        &self.events[start..]
    }

    /// Get error events
    pub fn errors(&self) -> Vec<&TimestampedEvent> {
        self.events.iter().filter(|e| e.event.is_error()).collect()
    }

    /// Clear all events from memory
    pub fn clear(&mut self) {
        self.flushed_count += self.events.len();
        self.events.clear();
    }

    /// Flush events to a file in JSONL format
    pub fn flush_to_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<usize> {
        let path = path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        let count = self.events.len();

        for event in &self.events {
            let json = serde_json::to_string(event)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            writeln!(writer, "{}", json)?;
        }

        writer.flush()?;

        // Clear after successful flush
        self.flushed_count += count;
        self.events.clear();

        Ok(count)
    }

    /// Append events to a file (without clearing memory)
    pub fn append_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<usize> {
        let path = path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        let mut writer = BufWriter::new(file);
        let count = self.events.len();

        for event in &self.events {
            let json = serde_json::to_string(event)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            writeln!(writer, "{}", json)?;
        }

        writer.flush()?;
        Ok(count)
    }

    /// Load events from a JSONL file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<TimestampedEvent>> {
        let content = fs::read_to_string(path)?;
        let mut events = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let event: TimestampedEvent = serde_json::from_str(line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            events.push(event);
        }

        Ok(events)
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_log_and_retrieve() {
        let mut log = EventLog::new();

        log.log(ContextEvent::SessionStarted {
            session_id: Uuid::new_v4(),
        });

        log.log(ContextEvent::PackLoaded {
            pack_name: "test-pack".to_string(),
            step_count: 3,
            workspace_count: 1,
        });

        assert_eq!(log.events().len(), 2);
        assert_eq!(log.total_count(), 2);
    }

    #[test]
    fn test_job_filtering() {
        let mut log = EventLog::new();
        let job_id = Uuid::new_v4();

        log.log(ContextEvent::ExecutionStarted {
            job_id,
            pack_name: "test".to_string(),
            workspace: "ws".to_string(),
            step_count: 2,
        });

        log.log(ContextEvent::StepStarted {
            job_id,
            step_name: "step1".to_string(),
            step_index: 0,
        });

        // Unrelated event
        log.log(ContextEvent::InputSet {
            name: "foo".to_string(),
            has_value: true,
        });

        let job_events = log.events_for_job(job_id);
        assert_eq!(job_events.len(), 2);
    }

    #[test]
    fn test_memory_limit() {
        let config = EventLogConfig {
            max_memory_events: 5,
            auto_flush: false,
            log_directory: None,
        };

        let mut log = EventLog::with_config(config);

        for i in 0..10 {
            log.log(ContextEvent::InputSet {
                name: format!("input_{}", i),
                has_value: true,
            });
        }

        // Should have trimmed to max
        assert_eq!(log.events().len(), 5);
        // Total should reflect all events
        assert_eq!(log.total_count(), 10);
    }
}
