//! TUI tracing layer for forwarding events to a channel
//!
//! This layer intercepts tracing spans and events, converts them to typed
//! `TuiEvent` instances, and sends them through an mpsc channel for TUI consumption.

use super::events::{ExecutionPhase, LogLevel, TuiEvent};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;
use uuid::Uuid;

/// A tracing layer that forwards events to a TUI via mpsc channel
pub struct TuiLayer {
    sender: mpsc::UnboundedSender<TuiEvent>,
    /// Tracks active spans with their metadata
    spans: Arc<RwLock<HashMap<u64, SpanData>>>,
}

/// Metadata stored for active spans
#[derive(Debug, Clone)]
struct SpanData {
    name: &'static str,
    start: Instant,
    fields: SpanFields,
    parent_id: Option<u64>,
}

/// Typed fields extracted from spans
#[derive(Debug, Clone, Default)]
struct SpanFields {
    job_id: Option<Uuid>,
    pack_name: Option<String>,
    workspace: Option<String>,
    workspace_count: Option<usize>,
    step_count: Option<usize>,
    step_name: Option<String>,
    step_type: Option<String>,
    phase: Option<String>,
}

impl TuiLayer {
    /// Create a new TUI layer with the given sender
    pub fn new(sender: mpsc::UnboundedSender<TuiEvent>) -> Self {
        Self {
            sender,
            spans: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Send an event, ignoring errors (receiver may have dropped)
    fn send(&self, event: TuiEvent) {
        let _ = self.sender.send(event);
    }

    /// Look up span data by ID
    fn get_span(&self, id: u64) -> Option<SpanData> {
        self.spans.read().ok()?.get(&id).cloned()
    }

    /// Find the job_id by walking up the span tree
    fn find_job_id(&self, span_id: Option<u64>) -> Option<Uuid> {
        let spans = self.spans.read().ok()?;
        let mut current_id = span_id;

        while let Some(id) = current_id {
            if let Some(span) = spans.get(&id) {
                if let Some(job_id) = span.fields.job_id {
                    return Some(job_id);
                }
                current_id = span.parent_id;
            } else {
                break;
            }
        }
        None
    }

    /// Find the step_name by walking up the span tree
    fn find_step_name(&self, span_id: Option<u64>) -> Option<String> {
        let spans = self.spans.read().ok()?;
        let mut current_id = span_id;

        while let Some(id) = current_id {
            if let Some(span) = spans.get(&id) {
                if span.name == "step" {
                    return span.fields.step_name.clone();
                }
                current_id = span.parent_id;
            } else {
                break;
            }
        }
        None
    }

    /// Find workspace by walking up the span tree
    fn find_workspace(&self, span_id: Option<u64>) -> Option<String> {
        let spans = self.spans.read().ok()?;
        let mut current_id = span_id;

        while let Some(id) = current_id {
            if let Some(span) = spans.get(&id) {
                if let Some(ref ws) = span.fields.workspace {
                    return Some(ws.clone());
                }
                current_id = span.parent_id;
            } else {
                break;
            }
        }
        None
    }
}

impl<S> Layer<S> for TuiLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut fields = SpanFields::default();
        attrs.record(&mut FieldVisitor(&mut fields));

        let parent_id = ctx
            .span(id)
            .and_then(|s| s.parent())
            .map(|p| p.id().into_u64());

        let span_data = SpanData {
            name: attrs.metadata().name(),
            start: Instant::now(),
            fields,
            parent_id,
        };

        if let Ok(mut spans) = self.spans.write() {
            spans.insert(id.into_u64(), span_data);
        }
    }

    fn on_enter(&self, id: &Id, _ctx: Context<'_, S>) {
        let span_id = id.into_u64();
        let Some(span) = self.get_span(span_id) else {
            return;
        };

        match span.name {
            "job" => {
                if let Some(job_id) = span.fields.job_id {
                    self.send(TuiEvent::JobStarted {
                        job_id,
                        pack_name: span.fields.pack_name.clone().unwrap_or_default(),
                        workspace_count: span.fields.workspace_count.unwrap_or(0),
                        step_count: span.fields.step_count.unwrap_or(0),
                        timestamp: Utc::now(),
                    });
                }
            }
            "phase" => {
                if let (Some(job_id), Some(phase_str)) =
                    (self.find_job_id(Some(span_id)), &span.fields.phase)
                {
                    let phase = match phase_str.as_str() {
                        "acquisition" => ExecutionPhase::Acquisition,
                        "processing" => ExecutionPhase::Processing,
                        "reporting" => ExecutionPhase::Reporting,
                        _ => return,
                    };
                    self.send(TuiEvent::PhaseStarted {
                        job_id,
                        workspace: span.fields.workspace.clone().unwrap_or_default(),
                        phase,
                        step_count: span.fields.step_count.unwrap_or(0),
                        timestamp: Utc::now(),
                    });
                }
            }
            "step" => {
                if let Some(job_id) = self.find_job_id(Some(span_id)) {
                    // Determine phase from parent or default to acquisition
                    let phase = self
                        .get_span(span.parent_id.unwrap_or(0))
                        .and_then(|p| p.fields.phase.clone())
                        .map(|p| match p.as_str() {
                            "processing" => ExecutionPhase::Processing,
                            "reporting" => ExecutionPhase::Reporting,
                            _ => ExecutionPhase::Acquisition,
                        })
                        .unwrap_or(ExecutionPhase::Acquisition);

                    self.send(TuiEvent::StepStarted {
                        job_id,
                        step_name: span.fields.step_name.clone().unwrap_or_default(),
                        workspace: self.find_workspace(Some(span_id)).unwrap_or_default(),
                        phase,
                        step_type: span.fields.step_type.clone().unwrap_or_default(),
                        timestamp: Utc::now(),
                    });
                }
            }
            _ => {}
        }
    }

    fn on_close(&self, id: Id, _ctx: Context<'_, S>) {
        let span_id = id.into_u64();
        let span = {
            let mut spans = match self.spans.write() {
                Ok(s) => s,
                Err(_) => return,
            };
            spans.remove(&span_id)
        };

        let Some(span) = span else { return };
        let duration = span.start.elapsed();

        match span.name {
            "job" => {
                if let Some(job_id) = span.fields.job_id {
                    // Note: success is determined by whether there was an error event
                    // For now, assume success - the executor can emit explicit failure
                    self.send(TuiEvent::JobCompleted {
                        job_id,
                        success: true,
                        duration,
                        timestamp: Utc::now(),
                    });
                }
            }
            "phase" => {
                if let (Some(job_id), Some(phase_str)) =
                    (self.find_job_id(span.parent_id), &span.fields.phase)
                {
                    let phase = match phase_str.as_str() {
                        "acquisition" => ExecutionPhase::Acquisition,
                        "processing" => ExecutionPhase::Processing,
                        "reporting" => ExecutionPhase::Reporting,
                        _ => return,
                    };
                    self.send(TuiEvent::PhaseCompleted {
                        job_id,
                        workspace: span.fields.workspace.clone().unwrap_or_default(),
                        phase,
                        duration,
                        timestamp: Utc::now(),
                    });
                }
            }
            // Step completion is handled via explicit events, not span close
            _ => {}
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);

        let current_span_id = ctx.current_span().id().map(|id| id.into_u64());
        let job_id = self.find_job_id(current_span_id);
        let step_name = self.find_step_name(current_span_id);

        // Check for special event types based on target or message
        let target = event.metadata().target();
        let level = LogLevel::from(*event.metadata().level());

        // Handle step completion/failure events specially
        if let Some(ref msg) = visitor.message {
            if msg == "step.completed" {
                if let Some(jid) = job_id {
                    self.send(TuiEvent::StepCompleted {
                        job_id: jid,
                        step_name: step_name.unwrap_or_default(),
                        workspace: self.find_workspace(current_span_id).unwrap_or_default(),
                        row_count: visitor.row_count,
                        duration: visitor.duration.unwrap_or_default(),
                        timestamp: Utc::now(),
                    });
                    return;
                }
            } else if msg == "step.failed" {
                if let Some(jid) = job_id {
                    self.send(TuiEvent::StepFailed {
                        job_id: jid,
                        step_name: step_name.unwrap_or_default(),
                        workspace: self.find_workspace(current_span_id).unwrap_or_default(),
                        error: visitor.error.unwrap_or_default(),
                        duration: visitor.duration.unwrap_or_default(),
                        timestamp: Utc::now(),
                    });
                    return;
                }
            } else if msg == "step.skipped" {
                if let Some(jid) = job_id {
                    self.send(TuiEvent::StepSkipped {
                        job_id: jid,
                        step_name: step_name.unwrap_or_default(),
                        workspace: self.find_workspace(current_span_id).unwrap_or_default(),
                        reason: visitor.reason.unwrap_or_default(),
                        timestamp: Utc::now(),
                    });
                    return;
                }
            } else if msg == "foreach.progress" {
                if let Some(jid) = job_id {
                    self.send(TuiEvent::ForeachProgress {
                        job_id: jid,
                        step_name: step_name.unwrap_or_default(),
                        workspace: self.find_workspace(current_span_id).unwrap_or_default(),
                        current: visitor.current.unwrap_or(0),
                        total: visitor.total.unwrap_or(0),
                        timestamp: Utc::now(),
                    });
                    return;
                }
            }
        }

        // Generic log event
        self.send(TuiEvent::Log {
            level,
            target: target.to_string(),
            message: visitor.message.unwrap_or_default(),
            job_id,
            step_name,
            timestamp: Utc::now(),
        });
    }
}

/// Visitor for extracting span fields
struct FieldVisitor<'a>(&'a mut SpanFields);

impl<'a> Visit for FieldVisitor<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let value_str = format!("{:?}", value);
        match field.name() {
            "job_id" => {
                if let Ok(id) = value_str.trim_matches('"').parse() {
                    self.0.job_id = Some(id);
                }
            }
            "pack" | "pack_name" => {
                self.0.pack_name = Some(value_str.trim_matches('"').to_string());
            }
            "workspace" => {
                self.0.workspace = Some(value_str.trim_matches('"').to_string());
            }
            "workspace_count" | "workspaces" => {
                if let Ok(n) = value_str.parse() {
                    self.0.workspace_count = Some(n);
                }
            }
            "step_count" | "steps" => {
                if let Ok(n) = value_str.parse() {
                    self.0.step_count = Some(n);
                }
            }
            "step" | "step_name" | "name" => {
                self.0.step_name = Some(value_str.trim_matches('"').to_string());
            }
            "step_type" => {
                self.0.step_type = Some(value_str.trim_matches('"').to_string());
            }
            "phase" => {
                self.0.phase = Some(value_str.trim_matches('"').to_string());
            }
            _ => {}
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "job_id" => {
                if let Ok(id) = value.parse() {
                    self.0.job_id = Some(id);
                }
            }
            "pack" | "pack_name" => self.0.pack_name = Some(value.to_string()),
            "workspace" => self.0.workspace = Some(value.to_string()),
            "step" | "step_name" | "name" => self.0.step_name = Some(value.to_string()),
            "step_type" => self.0.step_type = Some(value.to_string()),
            "phase" => self.0.phase = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "workspace_count" | "workspaces" => self.0.workspace_count = Some(value as usize),
            "step_count" | "steps" => self.0.step_count = Some(value as usize),
            _ => {}
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        match field.name() {
            "workspace_count" | "workspaces" => self.0.workspace_count = Some(value as usize),
            "step_count" | "steps" => self.0.step_count = Some(value as usize),
            _ => {}
        }
    }
}

/// Visitor for extracting event fields
#[derive(Default)]
struct EventVisitor {
    message: Option<String>,
    error: Option<String>,
    reason: Option<String>,
    row_count: Option<usize>,
    duration: Option<std::time::Duration>,
    current: Option<usize>,
    total: Option<usize>,
}

impl Visit for EventVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let value_str = format!("{:?}", value);
        match field.name() {
            "message" => self.message = Some(value_str.trim_matches('"').to_string()),
            "error" => self.error = Some(value_str.trim_matches('"').to_string()),
            "reason" => self.reason = Some(value_str.trim_matches('"').to_string()),
            "rows" | "row_count" => {
                if let Ok(n) = value_str.parse() {
                    self.row_count = Some(n);
                }
            }
            "duration_ms" => {
                if let Ok(ms) = value_str.parse::<u64>() {
                    self.duration = Some(std::time::Duration::from_millis(ms));
                }
            }
            "current" => {
                if let Ok(n) = value_str.parse() {
                    self.current = Some(n);
                }
            }
            "total" => {
                if let Ok(n) = value_str.parse() {
                    self.total = Some(n);
                }
            }
            _ => {}
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message = Some(value.to_string()),
            "error" => self.error = Some(value.to_string()),
            "reason" => self.reason = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "rows" | "row_count" => self.row_count = Some(value as usize),
            "duration_ms" => self.duration = Some(std::time::Duration::from_millis(value)),
            "current" => self.current = Some(value as usize),
            "total" => self.total = Some(value as usize),
            _ => {}
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        match field.name() {
            "rows" | "row_count" => self.row_count = Some(value as usize),
            "duration_ms" => {
                self.duration = Some(std::time::Duration::from_millis(value as u64))
            }
            "current" => self.current = Some(value as usize),
            "total" => self.total = Some(value as usize),
            _ => {}
        }
    }
}

/// Create a TUI event channel
pub fn tui_channel() -> (mpsc::UnboundedSender<TuiEvent>, mpsc::UnboundedReceiver<TuiEvent>) {
    mpsc::unbounded_channel()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::prelude::*;

    #[tokio::test]
    async fn test_tui_layer_receives_events() {
        let (tx, mut rx) = tui_channel();
        let layer = TuiLayer::new(tx);

        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        // Emit a simple event
        tracing::info!(target: "test", "hello world");

        // Check we received it
        let event = rx.try_recv().expect("should receive event");
        match event {
            TuiEvent::Log { message, level, .. } => {
                assert_eq!(message, "hello world");
                assert_eq!(level, LogLevel::Info);
            }
            _ => panic!("expected Log event"),
        }
    }
}
