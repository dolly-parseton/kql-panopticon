//! File-based tracing layer for persistent logging
//!
//! Writes trace events to JSON files in the user's config directory.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

/// A tracing layer that writes events to a JSON log file
pub struct FileLayer {
    writer: Mutex<BufWriter<File>>,
    path: PathBuf,
}

/// A single log entry written to the file
#[derive(Debug, Serialize)]
struct LogEntry {
    timestamp: DateTime<Utc>,
    level: String,
    target: String,
    message: String,
    fields: std::collections::HashMap<String, String>,
}

impl FileLayer {
    /// Create a new file layer that writes to the default location
    ///
    /// Default: `~/.kql-panopticon/traces/trace_<timestamp>.jsonl`
    pub fn new() -> std::io::Result<Self> {
        let base_dir = dirs::home_dir()
            .map(|p| p.join(".kql-panopticon").join("traces"))
            .unwrap_or_else(|| PathBuf::from(".kql-panopticon/traces"));

        Self::with_directory(base_dir)
    }

    /// Create a new file layer that writes to the specified directory
    pub fn with_directory(dir: PathBuf) -> std::io::Result<Self> {
        // Ensure directory exists
        fs::create_dir_all(&dir)?;

        // Create timestamped log file

        // Removing timestamp filenames, replacing with 'latest' to keep the volume of logging lower. TODO: rotate logs
        // let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        
        let filename = "trace_latest.jsonl".to_string();
        let path = dir.join(&filename);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        let writer = BufWriter::new(file);

        Ok(Self {
            writer: Mutex::new(writer),
            path,
        })
    }

    /// Get the path to the log file
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Write an entry to the log file
    fn write_entry(&self, entry: LogEntry) {
        if let Ok(mut writer) = self.writer.lock() {
            if let Ok(json) = serde_json::to_string(&entry) {
                let _ = writeln!(writer, "{}", json);
                let _ = writer.flush();
            }
        }
    }
}

impl<S> Layer<S> for FileLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        let entry = LogEntry {
            timestamp: Utc::now(),
            level: level_to_string(*event.metadata().level()),
            target: event.metadata().target().to_string(),
            message: visitor.message.unwrap_or_default(),
            fields: visitor.fields,
        };

        self.write_entry(entry);
    }
}

fn level_to_string(level: Level) -> String {
    match level {
        Level::TRACE => "TRACE".to_string(),
        Level::DEBUG => "DEBUG".to_string(),
        Level::INFO => "INFO".to_string(),
        Level::WARN => "WARN".to_string(),
        Level::ERROR => "ERROR".to_string(),
    }
}

/// Visitor for extracting event fields
#[derive(Default)]
struct FieldVisitor {
    message: Option<String>,
    fields: std::collections::HashMap<String, String>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let value_str = format!("{:?}", value);
        if field.name() == "message" {
            self.message = Some(value_str.trim_matches('"').to_string());
        } else {
            self.fields
                .insert(field.name().to_string(), value_str.trim_matches('"').to_string());
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(field.name().to_string(), value.to_string());
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::tempdir;
    use tracing_subscriber::prelude::*;

    #[test]
    fn test_file_layer_writes_events() {
        let dir = tempdir().unwrap();
        let layer = FileLayer::with_directory(dir.path().to_path_buf()).unwrap();
        let log_path = layer.path().clone();

        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        tracing::info!(test_field = "test_value", "Test message");

        // Read the log file
        let mut content = String::new();
        File::open(&log_path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        assert!(content.contains("Test message"));
        assert!(content.contains("test_value"));
        assert!(content.contains("INFO"));
    }
}
