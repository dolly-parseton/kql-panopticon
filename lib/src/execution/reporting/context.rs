//! Reporting phase execution context
//!
//! Provides lazy access to result data and output configuration for report generation.
//!
//! ## Design
//!
//! Unlike the previous approach that pre-materialized all data, this context holds
//! references to `ResultContext` objects and materializes data on-demand when
//! building the Tera template context. This aligns with how acquisition and
//! processing phases handle results.

use crate::error::Result;
use crate::execution::result::ResultContext;
use crate::variable::EvaluationContext;
use crate::workspace::Workspace;
use chrono::Local;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Metadata for report generation
#[derive(Debug, Clone)]
pub struct ReportMetadata {
    /// Pack name
    pub pack_name: String,
    /// Workspace name
    pub workspace_name: String,
    /// Workspace ID
    pub workspace_id: String,
    /// Subscription name
    pub subscription_name: String,
    /// Subscription ID
    pub subscription_id: String,
    /// Timestamp (RFC3339)
    pub timestamp: String,
}

impl ReportMetadata {
    /// Create metadata from workspace and pack info
    pub fn new(pack_name: &str, workspace: &Workspace) -> Self {
        Self {
            pack_name: pack_name.to_string(),
            workspace_name: workspace.name.clone(),
            workspace_id: workspace.workspace_id.clone(),
            subscription_name: workspace.subscription_name.clone(),
            subscription_id: workspace.subscription_id.clone(),
            timestamp: Local::now().to_rfc3339(),
        }
    }

    /// Convert to JSON value for templates
    pub fn to_json(&self) -> JsonValue {
        serde_json::json!({
            "timestamp": self.timestamp,
            "pack_name": self.pack_name,
            "workspace": self.workspace_name,
            "workspace_id": self.workspace_id,
            "subscription": self.subscription_name,
            "subscription_id": self.subscription_id,
        })
    }
}

/// Context for reporting phase execution
///
/// Holds references to result contexts and provides lazy materialization
/// when building template data. This defers data loading until a report
/// actually needs to render.
///
/// ## Usage
///
/// ```rust,ignore
/// let ctx = ReportingContext::new(
///     &acquisition_results,
///     processing_results.as_ref(),
///     &inputs,
///     metadata,
///     output_dir,
///     pack_path,
/// );
///
/// // Data is only materialized when needed
/// let template_data = ctx.build_template_data()?;
/// ```
pub struct ReportingContext<'a> {
    /// Acquisition phase results (lazy, file-backed)
    acquisition_results: &'a ResultContext,

    /// Processing phase results (lazy, file-backed)
    processing_results: Option<&'a ResultContext>,

    /// User-provided inputs
    inputs: &'a HashMap<String, String>,

    /// Report metadata
    metadata: ReportMetadata,

    /// Output directory for reports
    output_dir: &'a Path,

    /// Pack path for resolving template_file references
    pack_path: Option<&'a Path>,
}

impl<'a> ReportingContext<'a> {
    /// Create a new reporting context with lazy result access
    pub fn new(
        acquisition_results: &'a ResultContext,
        processing_results: Option<&'a ResultContext>,
        inputs: &'a HashMap<String, String>,
        metadata: ReportMetadata,
        output_dir: &'a Path,
        pack_path: Option<&'a Path>,
    ) -> Self {
        Self {
            acquisition_results,
            processing_results,
            inputs,
            metadata,
            output_dir,
            pack_path,
        }
    }

    /// Get the output directory
    pub fn output_dir(&self) -> &Path {
        self.output_dir
    }

    /// Get the pack path
    pub fn pack_path(&self) -> Option<&Path> {
        self.pack_path
    }

    /// Resolve a template file path relative to the pack
    pub fn resolve_template_path(&self, template_file: &str) -> PathBuf {
        if let Some(pack_path) = self.pack_path {
            if let Some(pack_dir) = pack_path.parent() {
                return pack_dir.join(template_file);
            }
        }
        PathBuf::from(template_file)
    }

    /// Get acquisition results reference
    pub fn acquisition_results(&self) -> &ResultContext {
        self.acquisition_results
    }

    /// Get processing results reference
    pub fn processing_results(&self) -> Option<&ResultContext> {
        self.processing_results
    }

    /// Get inputs reference
    pub fn inputs(&self) -> &HashMap<String, String> {
        self.inputs
    }

    /// Get metadata reference
    pub fn metadata(&self) -> &ReportMetadata {
        &self.metadata
    }

    /// Create an EvaluationContext for condition evaluation
    ///
    /// Merges acquisition and processing results into a single context
    /// for use with the variable module's `evaluate_condition` function.
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// let eval_ctx = reporting_ctx.to_evaluation_context();
    /// let should_run = evaluate_condition("{{signins | is_not_empty}}", &eval_ctx)?;
    /// ```
    pub fn to_evaluation_context(&self) -> EvaluationContext<'static> {
        // Clone acquisition results as the base
        let mut merged = self.acquisition_results.clone();

        // Merge in processing results if present
        if let Some(proc_results) = self.processing_results {
            merged.merge(proc_results.clone());
        }

        EvaluationContext::new()
            .with_step_results(merged)
            .with_inputs(self.inputs.clone())
    }

    /// Build template data by materializing results
    ///
    /// This is called just-in-time when a template needs to render.
    /// Data is materialized from the file-backed ResultContext handles.
    ///
    /// ## Template Data Structure
    ///
    /// ```json
    /// {
    ///   "meta": { "timestamp": "...", "pack_name": "...", ... },
    ///   "inputs": { "target_ip": "1.2.3.4", ... },
    ///   "acquisition": { "step_name": [...rows...], ... },
    ///   "processing": { "step_name": {...}, ... },
    ///   // Step names also available at top level for convenience
    ///   "signins": [...rows...],
    ///   "risk_score": {...}
    /// }
    /// ```
    pub fn build_template_data(&self) -> Result<HashMap<String, JsonValue>> {
        let mut context = HashMap::new();

        // Add metadata
        context.insert("meta".to_string(), self.metadata.to_json());

        // Add inputs
        let inputs_json: HashMap<String, JsonValue> = self
            .inputs
            .iter()
            .map(|(k, v)| (k.clone(), JsonValue::String(v.clone())))
            .collect();
        context.insert("inputs".to_string(), serde_json::json!(inputs_json));

        // Materialize acquisition results
        // Results are available both under `acquisition.step_name` and directly as `step_name`
        // for template convenience (e.g., `{% for row in signins %}`)
        let mut acquisition = HashMap::new();
        for step_name in self.acquisition_results.step_names() {
            if let Ok(rows) = self.acquisition_results.materialize(step_name) {
                let json_rows = serde_json::json!(rows);
                acquisition.insert(step_name.to_string(), json_rows.clone());
                // Also add at top level for template convenience
                context.insert(step_name.to_string(), json_rows);
            }
        }
        context.insert("acquisition".to_string(), serde_json::json!(acquisition));

        // Materialize processing results
        // Processing results typically have a single row (e.g., scoring output),
        // so we unwrap single-row results to maintain template compatibility.
        if let Some(proc_results) = self.processing_results {
            let mut processing = HashMap::new();
            for step_name in proc_results.step_names() {
                if let Ok(rows) = proc_results.materialize(step_name) {
                    // For single-row results, unwrap to the object directly
                    // This maintains backwards compatibility with templates expecting
                    // `processing.step_name.field` instead of `processing.step_name[0].field`
                    let value = if rows.len() == 1 {
                        rows.into_iter().next().unwrap()
                    } else {
                        serde_json::json!(rows)
                    };
                    processing.insert(step_name.to_string(), value);
                }
            }
            context.insert("processing".to_string(), serde_json::json!(processing));
        }

        Ok(context)
    }

    /// Get a specific value by dotted path for condition evaluation
    ///
    /// Supports paths like:
    /// - `acquisition.signins` -> array of rows
    /// - `processing.risk_score.score` -> specific field value
    /// - `signins` -> top-level step access
    ///
    /// This materializes only the specific step needed, not all data.
    pub fn get_value(&self, path: &str) -> Option<JsonValue> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return None;
        }

        match parts[0] {
            "acquisition" if parts.len() >= 2 => {
                let step_name = parts[1];
                let rows = self.acquisition_results.materialize(step_name).ok()?;
                let value = serde_json::json!(rows);
                navigate_json(&value, &parts[2..])
            }
            "processing" if parts.len() >= 2 => {
                let proc = self.processing_results?;
                let step_name = parts[1];
                let rows = proc.materialize(step_name).ok()?;
                // Unwrap single-row for consistency
                let value = if rows.len() == 1 {
                    rows.into_iter().next()?
                } else {
                    serde_json::json!(rows)
                };
                navigate_json(&value, &parts[2..])
            }
            "meta" => {
                let meta = self.metadata.to_json();
                navigate_json(&meta, &parts[1..])
            }
            "inputs" if parts.len() >= 2 => {
                let input_name = parts[1];
                self.inputs
                    .get(input_name)
                    .map(|v| JsonValue::String(v.clone()))
            }
            // Direct step name access (e.g., "signins" or "signins.0.user")
            step_name => {
                // Try acquisition first
                if self.acquisition_results.contains(step_name) {
                    let rows = self.acquisition_results.materialize(step_name).ok()?;
                    let value = serde_json::json!(rows);
                    return navigate_json(&value, &parts[1..]);
                }
                // Try processing
                if let Some(proc) = self.processing_results {
                    if proc.contains(step_name) {
                        let rows = proc.materialize(step_name).ok()?;
                        let value = if rows.len() == 1 {
                            rows.into_iter().next()?
                        } else {
                            serde_json::json!(rows)
                        };
                        return navigate_json(&value, &parts[1..]);
                    }
                }
                None
            }
        }
    }

    /// Check if a path exists and is not empty
    ///
    /// Used for condition evaluation like `signins is not empty`
    pub fn is_empty(&self, path: &str) -> bool {
        match self.get_value(path) {
            Some(value) => match value {
                JsonValue::Null => true,
                JsonValue::Array(arr) => arr.is_empty(),
                JsonValue::Object(obj) => obj.is_empty(),
                JsonValue::String(s) => s.is_empty(),
                _ => false,
            },
            None => true,
        }
    }
}

/// Navigate a JSON value by path parts
fn navigate_json(value: &JsonValue, parts: &[&str]) -> Option<JsonValue> {
    if parts.is_empty() {
        return Some(value.clone());
    }

    let mut current = value;
    for part in parts {
        // Try as object key first
        if let Some(obj_value) = current.get(part) {
            current = obj_value;
            continue;
        }
        // Try as array index
        if let Ok(index) = part.parse::<usize>() {
            if let Some(arr_value) = current.get(index) {
                current = arr_value;
                continue;
            }
        }
        return None;
    }

    Some(current.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::result::ResultWriter;
    use std::path::PathBuf;

    fn create_test_workspace() -> Workspace {
        Workspace {
            name: "test-workspace".to_string(),
            workspace_id: "ws-123".to_string(),
            resource_id: "/subscriptions/sub-456/resourceGroups/rg-test/providers/Microsoft.OperationalInsights/workspaces/test-workspace".to_string(),
            location: "eastus".to_string(),
            subscription_name: "test-sub".to_string(),
            subscription_id: "sub-456".to_string(),
            resource_group: "rg-test".to_string(),
            tenant_id: "tenant-789".to_string(),
        }
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("kql-panopticon-test")
            .join("reporting-context")
            .join(name)
    }

    #[test]
    fn test_metadata_to_json() {
        let workspace = create_test_workspace();
        let meta = ReportMetadata::new("test-pack", &workspace);
        let json = meta.to_json();

        assert_eq!(json["pack_name"], "test-pack");
        assert_eq!(json["workspace"], "test-workspace");
        assert_eq!(json["workspace_id"], "ws-123");
    }

    #[test]
    fn test_get_value_acquisition() {
        let path = temp_path("get_value_acq.jsonl");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::create_dir_all(path.parent().unwrap());

        let mut writer = ResultWriter::new(&path, "signins").unwrap();
        writer
            .write_rows(&[
                serde_json::json!({"user": "alice", "status": "success"}),
                serde_json::json!({"user": "bob", "status": "failed"}),
            ])
            .unwrap();
        let handle = writer.finish().unwrap();

        let mut acq_results = ResultContext::new();
        acq_results.insert("signins", handle);

        let workspace = create_test_workspace();
        let meta = ReportMetadata::new("test", &workspace);
        let inputs = HashMap::new();
        let output_dir = PathBuf::from("/tmp");

        let ctx = ReportingContext::new(
            &acq_results,
            None,
            &inputs,
            meta,
            &output_dir,
            None,
        );

        // Test direct step access
        let value = ctx.get_value("signins").unwrap();
        assert!(value.is_array());
        assert_eq!(value.as_array().unwrap().len(), 2);

        // Test nested access
        let first_user = ctx.get_value("signins.0.user").unwrap();
        assert_eq!(first_user, "alice");

        // Test acquisition prefix
        let via_acq = ctx.get_value("acquisition.signins").unwrap();
        assert!(via_acq.is_array());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_is_empty() {
        let path = temp_path("is_empty.jsonl");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::create_dir_all(path.parent().unwrap());

        // Create empty result
        let writer = ResultWriter::new(&path, "empty_step").unwrap();
        let handle = writer.finish().unwrap();

        let mut acq_results = ResultContext::new();
        acq_results.insert("empty_step", handle);

        let workspace = create_test_workspace();
        let meta = ReportMetadata::new("test", &workspace);
        let inputs = HashMap::new();
        let output_dir = PathBuf::from("/tmp");

        let ctx = ReportingContext::new(
            &acq_results,
            None,
            &inputs,
            meta,
            &output_dir,
            None,
        );

        assert!(ctx.is_empty("empty_step"));
        assert!(ctx.is_empty("nonexistent"));

        let _ = std::fs::remove_file(&path);
    }
}
