use crate::client::{Client, QueryResponse};
use crate::error::{KqlPanopticonError, Result};
use crate::investigation_pack::{Extract, ExtractType, InvestigationPack, Step};
use crate::workspace::Workspace;
use chrono::Local;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Extracted value from a query result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExtractedValue {
    /// A single value (first row)
    Single(String),
    /// Multiple values (all rows, possibly deduped)
    Array(Vec<String>),
}

impl ExtractedValue {
    /// Check if this value is empty
    pub fn is_empty(&self) -> bool {
        match self {
            ExtractedValue::Single(s) => s.is_empty(),
            ExtractedValue::Array(arr) => arr.is_empty(),
        }
    }

    /// Get the count of values
    pub fn len(&self) -> usize {
        match self {
            ExtractedValue::Single(_) => 1,
            ExtractedValue::Array(arr) => arr.len(),
        }
    }
}

/// Per-workspace context holding extracted values and step status
#[derive(Debug, Clone)]
pub struct WorkspaceContext {
    /// The workspace being processed
    pub workspace: Workspace,
    /// Extracted values keyed by "step_name.variable_name"
    pub extractions: HashMap<String, ExtractedValue>,
    /// Status of each step
    pub step_status: HashMap<String, StepStatus>,
}

impl WorkspaceContext {
    /// Create a new workspace context
    pub fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            extractions: HashMap::new(),
            step_status: HashMap::new(),
        }
    }

    /// Get an extracted value by full key (step.variable)
    pub fn get_extraction(&self, key: &str) -> Option<&ExtractedValue> {
        self.extractions.get(key)
    }

    /// Set an extracted value
    pub fn set_extraction(&mut self, step_name: &str, var_name: &str, value: ExtractedValue) {
        let key = format!("{}.{}", step_name, var_name);
        self.extractions.insert(key, value);
    }
}

/// Status of a single step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepStatus {
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunks_executed: Option<usize>,
}

/// Execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

/// Progress update sent during investigation execution
#[derive(Debug, Clone)]
pub enum ProgressUpdate {
    /// Investigation started
    Started {
        workspace_count: usize,
        step_count: usize,
    },
    /// A step started for a workspace
    StepStarted {
        workspace_name: String,
        step_name: String,
    },
    /// A step completed for a workspace
    StepCompleted {
        workspace_name: String,
        step_name: String,
        rows: usize,
        duration: Duration,
    },
    /// A step failed for a workspace
    StepFailed {
        workspace_name: String,
        step_name: String,
        error: String,
    },
    /// Investigation completed
    Completed {
        result: InvestigationResult,
    },
}

/// Result of an investigation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestigationResult {
    pub investigation_name: String,
    pub pack_path: Option<String>,
    pub started_at: String,
    pub completed_at: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    pub output_folder: PathBuf,
    pub workspaces: HashMap<String, WorkspaceResult>,
}

/// Result for a single workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceResult {
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    pub steps: HashMap<String, StepStatus>,
}

/// Runner for executing an investigation pack
pub struct InvestigationRunner {
    client: Client,
    pack: InvestigationPack,
    workspaces: Vec<Workspace>,
    inputs: HashMap<String, String>,
    output_base: PathBuf,
    pack_path: Option<PathBuf>,
}

impl InvestigationRunner {
    /// Create a new investigation runner
    pub fn new(
        client: Client,
        pack: InvestigationPack,
        workspaces: Vec<Workspace>,
        inputs: HashMap<String, String>,
        output_base: PathBuf,
    ) -> Self {
        Self {
            client,
            pack,
            workspaces,
            inputs,
            output_base,
            pack_path: None,
        }
    }

    /// Set the source pack path for manifest tracking
    pub fn with_pack_path(mut self, path: PathBuf) -> Self {
        self.pack_path = Some(path);
        self
    }

    /// Run the investigation
    pub async fn run(
        &self,
        progress_tx: Option<mpsc::UnboundedSender<ProgressUpdate>>,
    ) -> Result<InvestigationResult> {
        let started_at = Local::now();
        let timestamp = started_at.format("%Y-%m-%d_%H-%M-%S").to_string();

        // Build output folder from template
        let output_folder = self.build_output_folder(&timestamp)?;
        tokio::fs::create_dir_all(&output_folder).await?;

        // Get execution order
        let execution_order = self.pack.execution_order()?;

        if let Some(tx) = &progress_tx {
            let _ = tx.send(ProgressUpdate::Started {
                workspace_count: self.workspaces.len(),
                step_count: execution_order.len(),
            });
        }

        // Initialize workspace contexts
        let mut workspace_contexts: HashMap<String, WorkspaceContext> = self
            .workspaces
            .iter()
            .map(|w| (w.workspace_id.clone(), WorkspaceContext::new(w.clone())))
            .collect();

        // Track overall success
        let mut any_failure = false;
        let mut first_failure_reason: Option<String> = None;

        // Execute steps in order
        for step in &execution_order {
            debug!("Executing step '{}' across {} workspaces", step.name, self.workspaces.len());

            // Execute step for each workspace
            for workspace in &self.workspaces {
                let context = workspace_contexts
                    .get_mut(&workspace.workspace_id)
                    .expect("Workspace context should exist");

                // Check if any dependency failed for this workspace
                let deps_ok = step.depends_on.iter().all(|dep| {
                    context
                        .step_status
                        .get(dep)
                        .map(|s| s.status == Status::Success)
                        .unwrap_or(false)
                });

                if !deps_ok && !step.depends_on.is_empty() {
                    // Skip this step - dependency failed
                    context.step_status.insert(
                        step.name.clone(),
                        StepStatus {
                            status: Status::Skipped,
                            rows: None,
                            duration_ms: None,
                            error: Some("Dependency failed".into()),
                            chunks_executed: None,
                        },
                    );
                    continue;
                }

                if let Some(tx) = &progress_tx {
                    let _ = tx.send(ProgressUpdate::StepStarted {
                        workspace_name: workspace.name.clone(),
                        step_name: step.name.clone(),
                    });
                }

                // Execute the step
                let step_start = Instant::now();
                let step_result = self
                    .execute_step(step, context, &output_folder, &timestamp)
                    .await;
                let step_duration = step_start.elapsed();

                match step_result {
                    Ok(rows) => {
                        context.step_status.insert(
                            step.name.clone(),
                            StepStatus {
                                status: Status::Success,
                                rows: Some(rows),
                                duration_ms: Some(step_duration.as_millis() as u64),
                                error: None,
                                chunks_executed: None,
                            },
                        );

                        if let Some(tx) = &progress_tx {
                            let _ = tx.send(ProgressUpdate::StepCompleted {
                                workspace_name: workspace.name.clone(),
                                step_name: step.name.clone(),
                                rows,
                                duration: step_duration,
                            });
                        }

                        info!(
                            "Step '{}' completed for workspace '{}': {} rows in {:.2}s",
                            step.name,
                            workspace.name,
                            rows,
                            step_duration.as_secs_f64()
                        );
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        context.step_status.insert(
                            step.name.clone(),
                            StepStatus {
                                status: Status::Failed,
                                rows: None,
                                duration_ms: Some(step_duration.as_millis() as u64),
                                error: Some(error_msg.clone()),
                                chunks_executed: None,
                            },
                        );

                        if let Some(tx) = &progress_tx {
                            let _ = tx.send(ProgressUpdate::StepFailed {
                                workspace_name: workspace.name.clone(),
                                step_name: step.name.clone(),
                                error: error_msg.clone(),
                            });
                        }

                        warn!(
                            "Step '{}' failed for workspace '{}': {}",
                            step.name, workspace.name, error_msg
                        );

                        any_failure = true;
                        if first_failure_reason.is_none() {
                            first_failure_reason = Some(format!(
                                "Step '{}' failed on workspace '{}': {}",
                                step.name, workspace.name, error_msg
                            ));
                        }
                    }
                }
            }
        }

        // Build workspace results
        let workspace_results: HashMap<String, WorkspaceResult> = workspace_contexts
            .into_iter()
            .map(|(id, ctx)| {
                let workspace_failed = ctx
                    .step_status
                    .values()
                    .any(|s| s.status == Status::Failed);

                let (failure_step, failure_reason) = if workspace_failed {
                    let failed = ctx
                        .step_status
                        .iter()
                        .find(|(_, s)| s.status == Status::Failed);
                    (
                        failed.map(|(name, _)| name.clone()),
                        failed.and_then(|(_, s)| s.error.clone()),
                    )
                } else {
                    (None, None)
                };

                (
                    id,
                    WorkspaceResult {
                        status: if workspace_failed {
                            Status::Failed
                        } else {
                            Status::Success
                        },
                        failure_step,
                        failure_reason,
                        steps: ctx.step_status,
                    },
                )
            })
            .collect();

        let completed_at = Local::now();

        let result = InvestigationResult {
            investigation_name: self.pack.name.clone(),
            pack_path: self.pack_path.as_ref().map(|p| p.display().to_string()),
            started_at: started_at.to_rfc3339(),
            completed_at: completed_at.to_rfc3339(),
            status: if any_failure {
                Status::Failed
            } else {
                Status::Success
            },
            failure_reason: first_failure_reason,
            output_folder: output_folder.clone(),
            workspaces: workspace_results,
        };

        // Write manifest
        self.write_manifest(&output_folder, &result).await?;

        // Write inputs
        self.write_inputs(&output_folder).await?;

        if let Some(tx) = &progress_tx {
            let _ = tx.send(ProgressUpdate::Completed {
                result: result.clone(),
            });
        }

        if any_failure {
            Err(KqlPanopticonError::InvestigationExecutionFailed(
                result.failure_reason.clone().unwrap_or_else(|| "Unknown error".into()),
            ))
        } else {
            Ok(result)
        }
    }

    /// Build output folder path from template
    fn build_output_folder(&self, timestamp: &str) -> Result<PathBuf> {
        let folder_template = self
            .pack
            .output
            .as_ref()
            .map(|o| o.folder.as_str())
            .unwrap_or("./investigations/{{name}}/{{timestamp}}");

        let normalized_name = Workspace::normalize_name(&self.pack.name);
        let folder = folder_template
            .replace("{{name}}", &normalized_name)
            .replace("{{timestamp}}", timestamp);

        // If relative path, join with output_base
        let path = PathBuf::from(&folder);
        if path.is_relative() {
            Ok(self.output_base.join(path))
        } else {
            Ok(path)
        }
    }

    /// Execute a single step for a workspace
    async fn execute_step(
        &self,
        step: &Step,
        context: &mut WorkspaceContext,
        output_folder: &Path,
        timestamp: &str,
    ) -> Result<usize> {
        // Build workspace-specific output folder
        let normalized_subscription = Workspace::normalize_name(&context.workspace.subscription_name);
        let normalized_workspace = Workspace::normalize_name(&context.workspace.name);
        let step_output_dir = output_folder
            .join(&normalized_subscription)
            .join(&normalized_workspace)
            .join(&step.name);

        tokio::fs::create_dir_all(&step_output_dir).await?;

        // Substitute variables and get queries (may be multiple if chunking)
        let queries = self.substitute_variables(&step.query, context, step)?;

        if queries.is_empty() {
            return Err(KqlPanopticonError::InvestigationExecutionFailed(
                "Variable substitution produced no queries (empty array?)".into(),
            ));
        }

        // Execute queries (may be chunked)
        let mut all_rows: Vec<serde_json::Value> = Vec::new();
        let mut columns: Option<Vec<crate::client::Column>> = None;

        for (chunk_idx, query) in queries.iter().enumerate() {
            debug!(
                "Executing query chunk {}/{} for step '{}' on workspace '{}'",
                chunk_idx + 1,
                queries.len(),
                step.name,
                context.workspace.name
            );

            let response = self.execute_query_with_retry(&context.workspace, query).await?;

            if response.tables.is_empty() {
                continue;
            }

            let table = &response.tables[0];

            // Store columns from first response
            if columns.is_none() {
                columns = Some(table.columns.clone());
            }

            // Collect all rows
            all_rows.extend(table.rows.clone());

            // Handle pagination for this chunk
            let mut next_response = response;
            while let Some(ref next_link) = next_response.next_link {
                debug!("Fetching next page for chunk {}", chunk_idx + 1);
                next_response = self.client.query_next_page(next_link).await?;
                if !next_response.tables.is_empty() {
                    all_rows.extend(next_response.tables[0].rows.clone());
                }
            }
        }

        let row_count = all_rows.len();

        // Write results
        let columns = columns.unwrap_or_default();
        self.write_step_results(&step_output_dir, &step.name, &columns, &all_rows, &context.workspace, timestamp, &step.query)
            .await?;

        // Extract values if configured
        if !step.extract.is_empty() {
            let extracts = self.extract_values(&columns, &all_rows, &step.extract)?;

            // Write extracts to file
            self.write_extracts(&step_output_dir, &extracts).await?;

            // Store in context
            for (var_name, value) in extracts {
                context.set_extraction(&step.name, &var_name, value);
            }
        }

        Ok(row_count)
    }

    /// Substitute variables in a query, returning multiple queries if chunking is needed
    fn substitute_variables(
        &self,
        query: &str,
        context: &WorkspaceContext,
        _step: &Step,
    ) -> Result<Vec<String>> {
        let var_pattern = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();

        // First pass: identify all variables and check for array chunking needs
        let mut array_substitutions: Vec<(String, &Extract, Vec<String>)> = Vec::new();
        let mut single_substitutions: HashMap<String, String> = HashMap::new();

        for cap in var_pattern.captures_iter(query) {
            let var_ref = cap.get(1).unwrap().as_str().trim();

            if let Some((prefix, name)) = var_ref.split_once('.') {
                if prefix == "inputs" {
                    // Input variable
                    let value = self.inputs.get(name).ok_or_else(|| {
                        KqlPanopticonError::InvalidVariableReference(format!(
                            "Input '{}' not provided",
                            name
                        ))
                    })?;
                    single_substitutions.insert(var_ref.to_string(), value.clone());
                } else {
                    // Step extraction reference
                    let key = format!("{}.{}", prefix, name);
                    let extracted = context.get_extraction(&key).ok_or_else(|| {
                        KqlPanopticonError::InvalidVariableReference(format!(
                            "Extraction '{}' not found in context",
                            key
                        ))
                    })?;

                    // Find the extract config from the dependency step
                    let extract_config = self.find_extract_config(prefix, name)?;

                    match extracted {
                        ExtractedValue::Single(s) => {
                            let formatted = extract_config.quote_style.format_value(s);
                            single_substitutions.insert(var_ref.to_string(), formatted);
                        }
                        ExtractedValue::Array(arr) => {
                            if arr.is_empty() {
                                return Err(KqlPanopticonError::InvestigationExecutionFailed(
                                    format!("Extraction '{}' is empty, cannot substitute", key),
                                ));
                            }
                            array_substitutions.push((var_ref.to_string(), extract_config, arr.clone()));
                        }
                    }
                }
            }
        }

        // If no array substitutions or all arrays fit in one chunk, simple case
        if array_substitutions.is_empty() {
            let result = self.apply_substitutions(query, &single_substitutions, &HashMap::new());
            return Ok(vec![result]);
        }

        // Calculate chunking
        // For simplicity, we chunk based on the first array that needs chunking
        // More complex scenarios (multiple arrays) would need cartesian product handling
        if array_substitutions.len() > 1 {
            warn!("Multiple array substitutions in one query - using first array for chunking");
        }

        let (var_ref, extract_config, values) = &array_substitutions[0];
        let chunk_size = extract_config.chunk_size.unwrap_or(500);

        let chunks: Vec<Vec<String>> = values
            .chunks(chunk_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        let mut queries = Vec::new();
        for chunk in chunks {
            let formatted = extract_config.quote_style.format_array(&chunk);
            let mut chunk_subs = single_substitutions.clone();
            chunk_subs.insert(var_ref.clone(), formatted);

            // Handle other array substitutions (use full array - not ideal but functional)
            for (other_ref, other_config, other_values) in &array_substitutions[1..] {
                let formatted = other_config.quote_style.format_array(other_values);
                chunk_subs.insert(other_ref.clone(), formatted);
            }

            let result = self.apply_substitutions(query, &chunk_subs, &HashMap::new());
            queries.push(result);
        }

        Ok(queries)
    }

    /// Apply substitutions to a query string
    fn apply_substitutions(
        &self,
        query: &str,
        single_subs: &HashMap<String, String>,
        _array_subs: &HashMap<String, String>,
    ) -> String {
        let var_pattern = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();

        var_pattern
            .replace_all(query, |caps: &regex::Captures| {
                let var_ref = caps.get(1).unwrap().as_str().trim();
                single_subs
                    .get(var_ref)
                    .cloned()
                    .unwrap_or_else(|| format!("{{{{{}}}}}", var_ref))
            })
            .to_string()
    }

    /// Find extract configuration for a variable from a step
    fn find_extract_config(&self, step_name: &str, var_name: &str) -> Result<&Extract> {
        let step = self
            .pack
            .steps
            .iter()
            .find(|s| s.name == step_name)
            .ok_or_else(|| {
                KqlPanopticonError::InvalidVariableReference(format!(
                    "Step '{}' not found",
                    step_name
                ))
            })?;

        step.extract.get(var_name).ok_or_else(|| {
            KqlPanopticonError::InvalidVariableReference(format!(
                "Extraction '{}' not found in step '{}'",
                var_name, step_name
            ))
        })
    }

    /// Execute query with retry logic
    async fn execute_query_with_retry(
        &self,
        workspace: &Workspace,
        query: &str,
    ) -> Result<QueryResponse> {
        let timeout = self.client.query_timeout();
        let retry_count = self.client.retry_count();
        let mut last_error = None;
        let max_attempts = retry_count + 1;

        for attempt in 0..max_attempts {
            if attempt > 0 {
                let backoff = match &last_error {
                    Some(KqlPanopticonError::RateLimitExceeded { retry_after }) => {
                        info!(
                            "Rate limited on workspace '{}'. Waiting {} seconds",
                            workspace.name, retry_after
                        );
                        Duration::from_secs(*retry_after)
                    }
                    _ => Duration::from_secs(2u64.pow(attempt - 1)),
                };
                tokio::time::sleep(backoff).await;
            }

            let query_future = self
                .client
                .query_workspace(&workspace.workspace_id, query, None);

            match tokio::time::timeout(timeout, query_future).await {
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(e)) => {
                    last_error = Some(e);
                }
                Err(_) => {
                    last_error = Some(KqlPanopticonError::QueryExecutionFailed(format!(
                        "Query timed out after {} seconds",
                        timeout.as_secs()
                    )));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            KqlPanopticonError::QueryExecutionFailed("Query failed after all retries".into())
        }))
    }

    /// Extract values from query results
    fn extract_values(
        &self,
        columns: &[crate::client::Column],
        rows: &[serde_json::Value],
        extract_config: &HashMap<String, Extract>,
    ) -> Result<HashMap<String, ExtractedValue>> {
        let mut results = HashMap::new();

        for (var_name, config) in extract_config {
            // Find column index
            let col_idx = columns
                .iter()
                .position(|c| c.name == config.column)
                .ok_or_else(|| {
                    KqlPanopticonError::InvestigationExecutionFailed(format!(
                        "Column '{}' not found in results for extraction '{}'",
                        config.column, var_name
                    ))
                })?;

            match config.extract_type {
                ExtractType::Single => {
                    // Take first value
                    let value = rows
                        .first()
                        .and_then(|row| row.as_array())
                        .and_then(|arr| arr.get(col_idx))
                        .map(|v| self.value_to_string(v))
                        .unwrap_or_default();

                    results.insert(var_name.clone(), ExtractedValue::Single(value));
                }
                ExtractType::Array => {
                    // Collect all values
                    let mut values: Vec<String> = rows
                        .iter()
                        .filter_map(|row| row.as_array())
                        .filter_map(|arr| arr.get(col_idx))
                        .map(|v| self.value_to_string(v))
                        .filter(|s| !s.is_empty())
                        .collect();

                    // Dedupe if configured
                    if config.dedupe {
                        let mut seen = std::collections::HashSet::new();
                        values.retain(|v| seen.insert(v.clone()));
                    }

                    results.insert(var_name.clone(), ExtractedValue::Array(values));
                }
            }
        }

        Ok(results)
    }

    /// Convert a JSON value to string for extraction
    fn value_to_string(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => String::new(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => value.to_string(),
        }
    }

    /// Write step results to files
    async fn write_step_results(
        &self,
        output_dir: &Path,
        step_name: &str,
        columns: &[crate::client::Column],
        rows: &[serde_json::Value],
        workspace: &Workspace,
        timestamp: &str,
        query: &str,
    ) -> Result<()> {
        // Write CSV
        let csv_path = output_dir.join("results.csv");
        self.write_csv(&csv_path, columns, rows).await?;

        // Write JSON
        let json_path = output_dir.join("results.json");
        self.write_json(&json_path, columns, rows, workspace, timestamp, step_name, query)
            .await?;

        Ok(())
    }

    /// Write results as CSV
    async fn write_csv(
        &self,
        path: &Path,
        columns: &[crate::client::Column],
        rows: &[serde_json::Value],
    ) -> Result<()> {
        let mut content = String::new();

        // Header
        let headers: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
        content.push_str(&headers.join(","));
        content.push('\n');

        // Rows
        for row in rows {
            if let Some(arr) = row.as_array() {
                let values: Vec<String> = arr.iter().map(|v| self.format_csv_value(v)).collect();
                content.push_str(&values.join(","));
                content.push('\n');
            }
        }

        tokio::fs::write(path, content).await?;
        Ok(())
    }

    /// Format a value for CSV
    fn format_csv_value(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => String::new(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => {
                if s.contains(',') || s.contains('"') || s.contains('\n') {
                    format!("\"{}\"", s.replace('"', "\"\""))
                } else {
                    s.clone()
                }
            }
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                let json_str = value.to_string();
                format!("\"{}\"", json_str.replace('"', "\"\""))
            }
        }
    }

    /// Write results as JSON with metadata
    async fn write_json(
        &self,
        path: &Path,
        columns: &[crate::client::Column],
        rows: &[serde_json::Value],
        workspace: &Workspace,
        timestamp: &str,
        step_name: &str,
        query: &str,
    ) -> Result<()> {
        // Convert rows to objects
        let row_objects: Vec<serde_json::Value> = rows
            .iter()
            .filter_map(|row| {
                row.as_array().map(|arr| {
                    let mut obj = serde_json::Map::new();
                    for (idx, value) in arr.iter().enumerate() {
                        if let Some(col) = columns.get(idx) {
                            obj.insert(col.name.clone(), value.clone());
                        }
                    }
                    serde_json::Value::Object(obj)
                })
            })
            .collect();

        let output = serde_json::json!({
            "metadata": {
                "step": step_name,
                "workspace": workspace.name,
                "workspace_id": workspace.workspace_id,
                "subscription": workspace.subscription_name,
                "timestamp": timestamp,
                "query": query,
                "row_count": rows.len(),
            },
            "columns": columns.iter().map(|col| {
                serde_json::json!({
                    "name": col.name,
                    "type": col.column_type,
                })
            }).collect::<Vec<_>>(),
            "rows": row_objects,
        });

        let content = serde_json::to_string_pretty(&output)?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    /// Write extracts to JSON file
    async fn write_extracts(
        &self,
        output_dir: &Path,
        extracts: &HashMap<String, ExtractedValue>,
    ) -> Result<()> {
        let path = output_dir.join("extracts.json");
        let content = serde_json::to_string_pretty(extracts)?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    /// Write investigation manifest
    async fn write_manifest(&self, output_folder: &Path, result: &InvestigationResult) -> Result<()> {
        let path = output_folder.join("manifest.json");
        let content = serde_json::to_string_pretty(result)?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    /// Write inputs to file
    async fn write_inputs(&self, output_folder: &Path) -> Result<()> {
        let path = output_folder.join("inputs.json");
        let content = serde_json::to_string_pretty(&self.inputs)?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extracted_value_len() {
        let single = ExtractedValue::Single("test".into());
        assert_eq!(single.len(), 1);

        let array = ExtractedValue::Array(vec!["a".into(), "b".into(), "c".into()]);
        assert_eq!(array.len(), 3);

        let empty = ExtractedValue::Array(vec![]);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_workspace_context() {
        let workspace = Workspace {
            workspace_id: "test-id".into(),
            resource_id: "/subscriptions/test".into(),
            name: "test-workspace".into(),
            location: "eastus".into(),
            subscription_id: "sub-id".into(),
            resource_group: "rg".into(),
            tenant_id: "tenant".into(),
            subscription_name: "Test Sub".into(),
        };

        let mut ctx = WorkspaceContext::new(workspace);

        ctx.set_extraction("step1", "users", ExtractedValue::Array(vec!["user1".into(), "user2".into()]));

        let extracted = ctx.get_extraction("step1.users");
        assert!(extracted.is_some());

        match extracted.unwrap() {
            ExtractedValue::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr[0], "user1");
            }
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn test_step_status_serialization() {
        let status = StepStatus {
            status: Status::Success,
            rows: Some(42),
            duration_ms: Some(1234),
            error: None,
            chunks_executed: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"success\""));
        assert!(json.contains("42"));
        assert!(!json.contains("error")); // skip_serializing_if works
    }
}
