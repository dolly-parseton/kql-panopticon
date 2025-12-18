use crate::client::{Client, Column, QueryResponse};
use crate::error::{KqlPanopticonError, Result};
use crate::investigation_pack::{
    Extract, ExtractType, ForeachClause, InvestigationPack, Step, StepType,
};
use crate::workspace::Workspace;
use chrono::Local;
use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use super::condition::evaluate_condition;
use super::http::{handle_http_error, HttpExecutor, RateLimiter, ResolvedSecrets};
use super::report::ReportGenerator;
use super::types::{
    ExtractedValue, InvestigationResult, ProgressUpdate, Status, StepStatus, WorkspaceContext,
    WorkspaceResult,
};

/// Parsed variable reference from query
#[derive(Debug, Clone)]
enum VarRef {
    /// Input variable: {{inputs.name}}
    Input { name: String },
    /// Array access: {{step.*.Column}}
    ArrayColumn { step: String, column: String },
    /// First row access: {{step.first.Column}}
    FirstColumn { step: String, column: String },
    /// Indexed access: {{step[N].Column}}
    IndexedColumn { step: String, index: usize, column: String },
    /// Legacy extract: {{step.extract_name}}
    LegacyExtract { step: String, extract: String },
    /// Foreach alias: {{alias.Column}}
    ForeachAlias { alias: String, column: String },
}

impl VarRef {
    /// Parse a variable reference string
    fn parse(var_ref: &str, foreach_alias: Option<&str>) -> Option<Self> {
        let var_ref = var_ref.trim();

        // Input variable: inputs.name
        if let Some(name) = var_ref.strip_prefix("inputs.") {
            return Some(VarRef::Input { name: name.to_string() });
        }

        // Check for foreach alias first: alias.Column
        if let Some(alias) = foreach_alias {
            if let Some(column) = var_ref.strip_prefix(&format!("{}.", alias)) {
                return Some(VarRef::ForeachAlias {
                    alias: alias.to_string(),
                    column: column.to_string()
                });
            }
        }

        // Array access: step.*.Column
        if let Some((step, rest)) = var_ref.split_once(".*.") {
            return Some(VarRef::ArrayColumn {
                step: step.to_string(),
                column: rest.to_string()
            });
        }

        // First row access: step.first.Column
        if let Some((step, rest)) = var_ref.split_once(".first.") {
            return Some(VarRef::FirstColumn {
                step: step.to_string(),
                column: rest.to_string()
            });
        }

        // Indexed access: step[N].Column
        if let Some(bracket_pos) = var_ref.find('[') {
            if let Some(close_pos) = var_ref.find("].") {
                let step = &var_ref[..bracket_pos];
                let index_str = &var_ref[bracket_pos + 1..close_pos];
                let column = &var_ref[close_pos + 2..];
                if let Ok(index) = index_str.parse::<usize>() {
                    return Some(VarRef::IndexedColumn {
                        step: step.to_string(),
                        index,
                        column: column.to_string(),
                    });
                }
            }
        }

        // Legacy or direct column access: step.something
        if let Some((step, rest)) = var_ref.split_once('.') {
            // Could be legacy extract or direct column access
            return Some(VarRef::LegacyExtract {
                step: step.to_string(),
                extract: rest.to_string()
            });
        }

        None
    }
}

/// Runner for executing an investigation pack
pub struct InvestigationRunner {
    client: Client,
    pack: InvestigationPack,
    workspaces: Vec<Workspace>,
    inputs: HashMap<String, String>,
    output_base: PathBuf,
    pack_path: Option<PathBuf>,
    #[allow(dead_code)]
    secrets: ResolvedSecrets,
}

impl InvestigationRunner {
    /// Create a new investigation runner
    pub fn new(
        client: Client,
        pack: InvestigationPack,
        workspaces: Vec<Workspace>,
        inputs: HashMap<String, String>,
        output_base: PathBuf,
    ) -> Result<Self> {
        // Resolve secrets from environment variables
        let secrets = ResolvedSecrets::resolve(pack.secrets.as_ref())?;

        Ok(Self {
            client,
            pack,
            workspaces,
            inputs,
            output_base,
            pack_path: None,
            secrets,
        })
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

        // Track step results for condition evaluation (aggregated across workspaces)
        let mut step_results: HashMap<String, Vec<serde_json::Value>> = HashMap::new();

        // Track overall success
        let mut any_failure = false;
        let mut first_failure_reason: Option<String> = None;

        // Execute steps in order
        for step in &execution_order {
            debug!(
                "Executing step '{}' across {} workspaces",
                step.name,
                self.workspaces.len()
            );

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

                // Check `when` condition if specified
                if let Some(when_condition) = &step.when {
                    if !evaluate_condition(when_condition, &step_results) {
                        info!(
                            "Step '{}' skipped for workspace '{}': condition '{}' not met",
                            step.name, workspace.name, when_condition
                        );
                        context.step_status.insert(
                            step.name.clone(),
                            StepStatus {
                                status: Status::Skipped,
                                rows: None,
                                duration_ms: None,
                                error: Some(format!("Condition not met: {}", when_condition)),
                                chunks_executed: None,
                            },
                        );
                        continue;
                    }
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

                        // Load step results for condition evaluation
                        let normalized_subscription = Workspace::normalize_name(&workspace.subscription_name);
                        let normalized_workspace = Workspace::normalize_name(&workspace.name);
                        let results_path = output_folder
                            .join(&normalized_subscription)
                            .join(&normalized_workspace)
                            .join(&step.name)
                            .join("results.json");

                        if let Ok(content) = tokio::fs::read_to_string(&results_path).await {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(rows) = json.get("rows").and_then(|r| r.as_array()) {
                                    step_results
                                        .entry(step.name.clone())
                                        .or_default()
                                        .extend(rows.clone());
                                }
                            }
                        }
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

        // Generate report if configured
        if self.pack.report.is_some() {
            let report_generator = ReportGenerator::new(&self.pack, &self.workspaces, &self.inputs);
            report_generator
                .generate(&output_folder, &result, &timestamp)
                .await?;
        }

        if let Some(tx) = &progress_tx {
            let _ = tx.send(ProgressUpdate::Completed {
                result: result.clone(),
            });
        }

        if any_failure {
            Err(KqlPanopticonError::InvestigationExecutionFailed(
                result
                    .failure_reason
                    .clone()
                    .unwrap_or_else(|| "Unknown error".into()),
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
        let normalized_subscription =
            Workspace::normalize_name(&context.workspace.subscription_name);
        let normalized_workspace = Workspace::normalize_name(&context.workspace.name);
        let step_output_dir = output_folder
            .join(&normalized_subscription)
            .join(&normalized_workspace)
            .join(&step.name);

        tokio::fs::create_dir_all(&step_output_dir).await?;

        // Route based on step type
        match step.step_type {
            StepType::Kql => {
                // Check if this step uses foreach
                if let Some(foreach_str) = &step.foreach {
                    return self
                        .execute_step_foreach(step, context, &step_output_dir, timestamp, foreach_str)
                        .await;
                }
                // Standard KQL execution (no foreach)
                self.execute_step_simple(step, context, &step_output_dir, timestamp)
                    .await
            }
            StepType::Http => {
                // HTTP step execution
                if let Some(foreach_str) = &step.foreach {
                    self.execute_http_step_foreach(step, context, &step_output_dir, timestamp, foreach_str)
                        .await
                } else {
                    self.execute_http_step_simple(step, context, &step_output_dir, timestamp)
                        .await
                }
            }
        }
    }

    /// Execute a step without foreach (standard execution)
    async fn execute_step_simple(
        &self,
        step: &Step,
        context: &mut WorkspaceContext,
        step_output_dir: &Path,
        timestamp: &str,
    ) -> Result<usize> {
        // Substitute variables and get queries (may be multiple if chunking)
        let queries = self.substitute_variables(&step.query, step, context, None, None)?;

        if queries.is_empty() {
            return Err(KqlPanopticonError::InvestigationExecutionFailed(
                "Variable substitution produced no queries (empty array?)".into(),
            ));
        }

        // Execute queries (may be chunked)
        let (columns, all_rows) = self
            .execute_queries(&queries, step, context)
            .await?;

        let row_count = all_rows.len();

        // Write and store results
        self.finalize_step_results(step, context, step_output_dir, timestamp, columns, all_rows)
            .await?;

        Ok(row_count)
    }

    /// Execute a step with foreach iteration
    async fn execute_step_foreach(
        &self,
        step: &Step,
        context: &mut WorkspaceContext,
        step_output_dir: &Path,
        timestamp: &str,
        foreach_str: &str,
    ) -> Result<usize> {
        // Parse foreach clause
        let foreach_clause = ForeachClause::parse(foreach_str).ok_or_else(|| {
            KqlPanopticonError::InvestigationExecutionFailed(format!(
                "Invalid foreach syntax: '{}'. Expected 'step_name as alias'",
                foreach_str
            ))
        })?;

        // Get source step results
        let source_rows = context
            .get_step_results(&foreach_clause.source_step)
            .cloned()
            .unwrap_or_default();

        // Handle empty source
        if source_rows.is_empty() {
            let on_empty = step.on_empty.clone().unwrap_or_default();
            match on_empty {
                crate::investigation_pack::OnEmpty::Skip => {
                    info!(
                        "Step '{}' foreach source '{}' is empty, skipping",
                        step.name, foreach_clause.source_step
                    );
                    // Store empty results
                    context.set_step_results(&step.name, vec![], vec![]);
                    return Ok(0);
                }
                crate::investigation_pack::OnEmpty::Error => {
                    return Err(KqlPanopticonError::InvestigationExecutionFailed(format!(
                        "Foreach source '{}' is empty",
                        foreach_clause.source_step
                    )));
                }
            }
        }

        let batch_size = step.batch_size.unwrap_or(1);
        let aggregate = step.aggregate.clone().unwrap_or_default();

        let mut all_columns: Option<Vec<crate::client::Column>> = None;
        let mut all_rows: Vec<serde_json::Value> = Vec::new();
        let mut iteration_results: Vec<Vec<serde_json::Value>> = Vec::new();

        // Process rows in batches
        for (batch_idx, batch) in source_rows.chunks(batch_size).enumerate() {
            debug!(
                "Step '{}' foreach batch {}/{} ({} rows)",
                step.name,
                batch_idx + 1,
                source_rows.len().div_ceil(batch_size),
                batch.len()
            );

            // For batch_size=1, use single row context; otherwise use first row of batch
            // (for batch processing, user should use {{step.*.Column}} syntax)
            let foreach_row = batch.first();

            // Substitute variables with foreach context
            let queries = self.substitute_variables(
                &step.query,
                step,
                context,
                Some(&foreach_clause.alias),
                foreach_row,
            )?;

            if queries.is_empty() {
                continue;
            }

            // Execute queries for this batch
            let (columns, rows) = self.execute_queries(&queries, step, context).await?;

            // Store columns from first result
            if all_columns.is_none() && !columns.is_empty() {
                all_columns = Some(columns);
            }

            // Aggregate based on strategy
            match aggregate {
                crate::investigation_pack::AggregateStrategy::Append => {
                    all_rows.extend(rows);
                }
                crate::investigation_pack::AggregateStrategy::Replace => {
                    all_rows = rows;
                }
                crate::investigation_pack::AggregateStrategy::Merge => {
                    // Merge: dedupe rows by all columns
                    for row in rows {
                        if !all_rows.contains(&row) {
                            all_rows.push(row);
                        }
                    }
                }
                crate::investigation_pack::AggregateStrategy::Collect => {
                    // Collect: keep each iteration's results separate (for report access)
                    iteration_results.push(rows);
                }
            }
        }

        // For collect strategy, flatten for storage but keep structure in a special field
        if matches!(aggregate, crate::investigation_pack::AggregateStrategy::Collect) {
            all_rows = iteration_results.into_iter().flatten().collect();
        }

        let row_count = all_rows.len();

        // Write and store results
        let columns = all_columns.unwrap_or_default();
        self.finalize_step_results(step, context, step_output_dir, timestamp, columns, all_rows)
            .await?;

        Ok(row_count)
    }

    /// Execute a set of queries and return columns and rows
    async fn execute_queries(
        &self,
        queries: &[String],
        step: &Step,
        context: &WorkspaceContext,
    ) -> Result<(Vec<crate::client::Column>, Vec<serde_json::Value>)> {
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

            let response = self
                .execute_query_with_retry(&context.workspace, query)
                .await?;

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

        Ok((columns.unwrap_or_default(), all_rows))
    }

    /// Finalize step results: write to files and store in context
    async fn finalize_step_results(
        &self,
        step: &Step,
        context: &mut WorkspaceContext,
        step_output_dir: &Path,
        timestamp: &str,
        columns: Vec<crate::client::Column>,
        all_rows: Vec<serde_json::Value>,
    ) -> Result<()> {
        // Write results
        self.write_step_results(
            step_output_dir,
            &step.name,
            &columns,
            &all_rows,
            &context.workspace,
            timestamp,
            &step.query,
        )
        .await?;

        // Store step results in context for variable substitution
        // Convert rows (arrays) to JSON objects with column names as keys
        let column_names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();
        let row_objects: Vec<serde_json::Value> = all_rows
            .iter()
            .filter_map(|row| {
                row.as_array().map(|arr| {
                    let mut obj = serde_json::Map::new();
                    for (idx, value) in arr.iter().enumerate() {
                        if let Some(col_name) = column_names.get(idx) {
                            obj.insert(col_name.clone(), value.clone());
                        }
                    }
                    serde_json::Value::Object(obj)
                })
            })
            .collect();
        context.set_step_results(&step.name, column_names, row_objects);

        // Extract values if configured (legacy extract system)
        if !step.extract.is_empty() {
            let extracts = self.extract_values(&columns, &all_rows, &step.extract)?;

            // Write extracts to file
            self.write_extracts(step_output_dir, &extracts).await?;

            // Store in context
            for (var_name, value) in extracts {
                context.set_extraction(&step.name, &var_name, value);
            }
        }

        Ok(())
    }

    /// Substitute variables in a query, returning multiple queries if chunking is needed.
    ///
    /// Supports multiple variable reference syntaxes:
    /// - `{{inputs.name}}` - Input variable
    /// - `{{step.*.Column}}` - All values from column (array)
    /// - `{{step.first.Column}}` - First row's column value
    /// - `{{step[N].Column}}` - Nth row's column value
    /// - `{{alias.Column}}` - Foreach alias column (requires foreach_row)
    /// - `{{step.extract}}` - Legacy extract reference
    fn substitute_variables(
        &self,
        query: &str,
        step: &Step,
        context: &WorkspaceContext,
        foreach_alias: Option<&str>,
        foreach_row: Option<&serde_json::Value>,
    ) -> Result<Vec<String>> {
        let var_pattern = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();

        // Get step options for formatting
        let quote_style = step.options.as_ref()
            .and_then(|o| o.quote_style.clone())
            .unwrap_or_default();
        let dedupe = step.options.as_ref()
            .and_then(|o| o.dedupe)
            .unwrap_or(false);
        let chunk_size = step.options.as_ref()
            .and_then(|o| o.chunk_size)
            .unwrap_or(500);

        // First pass: identify all variables and resolve values
        let mut array_substitutions: Vec<(String, Vec<String>)> = Vec::new();
        let mut single_substitutions: HashMap<String, String> = HashMap::new();
        let mut seen_refs: HashSet<String> = HashSet::new();

        for cap in var_pattern.captures_iter(query) {
            let var_ref_str = cap.get(1).unwrap().as_str().trim();

            // Skip if already processed
            if seen_refs.contains(var_ref_str) {
                continue;
            }
            seen_refs.insert(var_ref_str.to_string());

            // Parse the variable reference
            let var_ref = VarRef::parse(var_ref_str, foreach_alias);

            match var_ref {
                Some(VarRef::Input { name }) => {
                    let value = self.inputs.get(&name).ok_or_else(|| {
                        KqlPanopticonError::InvalidVariableReference(format!(
                            "Input '{}' not provided",
                            name
                        ))
                    })?;
                    single_substitutions.insert(var_ref_str.to_string(), value.clone());
                }

                Some(VarRef::ArrayColumn { step: step_name, column }) => {
                    let mut values = context.get_column_values(&step_name, &column);
                    if values.is_empty() {
                        return Err(KqlPanopticonError::InvestigationExecutionFailed(
                            format!("Step '{}' column '{}' has no values", step_name, column),
                        ));
                    }
                    if dedupe {
                        let mut seen = HashSet::new();
                        values.retain(|v| seen.insert(v.clone()));
                    }
                    array_substitutions.push((var_ref_str.to_string(), values));
                }

                Some(VarRef::FirstColumn { step: step_name, column }) => {
                    let value = context.get_first_column_value(&step_name, &column)
                        .ok_or_else(|| {
                            KqlPanopticonError::InvalidVariableReference(format!(
                                "Step '{}' has no results or column '{}' not found",
                                step_name, column
                            ))
                        })?;
                    let formatted = quote_style.format_value(&value);
                    single_substitutions.insert(var_ref_str.to_string(), formatted);
                }

                Some(VarRef::IndexedColumn { step: step_name, index, column }) => {
                    let value = context.get_indexed_column_value(&step_name, index, &column)
                        .ok_or_else(|| {
                            KqlPanopticonError::InvalidVariableReference(format!(
                                "Step '{}' row {} or column '{}' not found",
                                step_name, index, column
                            ))
                        })?;
                    let formatted = quote_style.format_value(&value);
                    single_substitutions.insert(var_ref_str.to_string(), formatted);
                }

                Some(VarRef::ForeachAlias { alias, column }) => {
                    let row = foreach_row.ok_or_else(|| {
                        KqlPanopticonError::InvalidVariableReference(format!(
                            "Foreach alias '{}' used but no foreach row context",
                            alias
                        ))
                    })?;
                    let value = row.get(&column)
                        .map(|v| match v {
                            serde_json::Value::Null => String::new(),
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .ok_or_else(|| {
                            KqlPanopticonError::InvalidVariableReference(format!(
                                "Column '{}' not found in foreach row",
                                column
                            ))
                        })?;
                    let formatted = quote_style.format_value(&value);
                    single_substitutions.insert(var_ref_str.to_string(), formatted);
                }

                Some(VarRef::LegacyExtract { step: step_name, extract }) => {
                    // Try new column access first (step.Column), then legacy extract
                    if let Some(value) = context.get_first_column_value(&step_name, &extract) {
                        let formatted = quote_style.format_value(&value);
                        single_substitutions.insert(var_ref_str.to_string(), formatted);
                    } else {
                        // Fall back to legacy extract system
                        let key = format!("{}.{}", step_name, extract);
                        let extracted = context.get_extraction(&key).ok_or_else(|| {
                            KqlPanopticonError::InvalidVariableReference(format!(
                                "Step '{}' has no column or extraction named '{}'",
                                step_name, extract
                            ))
                        })?;

                        match extracted {
                            ExtractedValue::Single(s) => {
                                // Try to find legacy extract config for quote style
                                let qs = self.find_extract_config(&step_name, &extract)
                                    .map(|e| e.quote_style.clone())
                                    .unwrap_or(quote_style.clone());
                                let formatted = qs.format_value(s);
                                single_substitutions.insert(var_ref_str.to_string(), formatted);
                            }
                            ExtractedValue::Array(arr) => {
                                if arr.is_empty() {
                                    return Err(KqlPanopticonError::InvestigationExecutionFailed(
                                        format!("Extraction '{}' is empty", key),
                                    ));
                                }
                                array_substitutions.push((var_ref_str.to_string(), arr.clone()));
                            }
                        }
                    }
                }

                None => {
                    return Err(KqlPanopticonError::InvalidVariableReference(format!(
                        "Could not parse variable reference: {}",
                        var_ref_str
                    )));
                }
            }
        }

        // If no array substitutions, simple case
        if array_substitutions.is_empty() {
            let result = self.apply_substitutions(query, &single_substitutions);
            return Ok(vec![result]);
        }

        // Handle chunking for array substitutions
        if array_substitutions.len() > 1 {
            warn!("Multiple array substitutions in one query - using first array for chunking");
        }

        let (var_ref, values) = &array_substitutions[0];

        let chunks: Vec<Vec<String>> = values
            .chunks(chunk_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        let mut queries = Vec::new();
        for chunk in chunks {
            let formatted = quote_style.format_array(&chunk);
            let mut chunk_subs = single_substitutions.clone();
            chunk_subs.insert(var_ref.clone(), formatted);

            // Handle other array substitutions (use full array)
            for (other_ref, other_values) in &array_substitutions[1..] {
                let formatted = quote_style.format_array(other_values);
                chunk_subs.insert(other_ref.clone(), formatted);
            }

            let result = self.apply_substitutions(query, &chunk_subs);
            queries.push(result);
        }

        Ok(queries)
    }

    /// Apply variable substitutions to a query string.
    fn apply_substitutions(
        &self,
        query: &str,
        substitutions: &HashMap<String, String>,
    ) -> String {
        let var_pattern = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();

        var_pattern
            .replace_all(query, |caps: &regex::Captures| {
                let var_ref = caps.get(1).unwrap().as_str().trim();
                substitutions
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
    #[allow(clippy::too_many_arguments)]
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
        self.write_json(
            &json_path, columns, rows, workspace, timestamp, step_name, query,
        )
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
    #[allow(clippy::too_many_arguments)]
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
    async fn write_manifest(
        &self,
        output_folder: &Path,
        result: &InvestigationResult,
    ) -> Result<()> {
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

    // ============================================================
    // HTTP Step Execution Methods
    // ============================================================

    /// Execute a simple HTTP step (no foreach)
    async fn execute_http_step_simple(
        &self,
        step: &Step,
        context: &mut WorkspaceContext,
        step_output_dir: &Path,
        timestamp: &str,
    ) -> Result<usize> {
        // Build substitutions from inputs and prior step results
        let substitutions = self.build_http_substitutions(step, context, None, None)?;

        // Create rate limiter for this step
        let mut rate_limiter = RateLimiter::new(step.rate_limit.clone());

        // Create HTTP executor
        let http_executor = HttpExecutor::new(
            Some(self.client.clone()),
            ResolvedSecrets::resolve(self.pack.secrets.as_ref())?,
            self.client.query_timeout(),
            self.client.retry_count(),
        )?;

        // Execute HTTP request
        let result = http_executor
            .execute(step, &substitutions, &mut rate_limiter)
            .await?;

        let row_count = result.rows.len();

        // Convert to column format and store results
        let columns = self.extract_columns_from_rows(&result.rows);
        self.finalize_http_step_results(
            step,
            context,
            step_output_dir,
            timestamp,
            columns,
            result.rows,
        )
        .await?;

        Ok(row_count)
    }

    /// Execute an HTTP step with foreach iteration
    async fn execute_http_step_foreach(
        &self,
        step: &Step,
        context: &mut WorkspaceContext,
        step_output_dir: &Path,
        timestamp: &str,
        foreach_str: &str,
    ) -> Result<usize> {
        // Parse foreach clause
        let foreach_clause = ForeachClause::parse(foreach_str).ok_or_else(|| {
            KqlPanopticonError::InvestigationExecutionFailed(format!(
                "Invalid foreach syntax: '{}'. Expected 'step_name as alias'",
                foreach_str
            ))
        })?;

        // Get source step results
        let source_rows = context
            .get_step_results(&foreach_clause.source_step)
            .cloned()
            .unwrap_or_default();

        // Handle empty source
        if source_rows.is_empty() {
            let on_empty = step.on_empty.clone().unwrap_or_default();
            match on_empty {
                crate::investigation_pack::OnEmpty::Skip => {
                    info!(
                        "HTTP step '{}' foreach source '{}' is empty, skipping",
                        step.name, foreach_clause.source_step
                    );
                    context.set_step_results(&step.name, vec![], vec![]);
                    return Ok(0);
                }
                crate::investigation_pack::OnEmpty::Error => {
                    return Err(KqlPanopticonError::InvestigationExecutionFailed(format!(
                        "Foreach source '{}' is empty",
                        foreach_clause.source_step
                    )));
                }
            }
        }

        let batch_size = step.batch_size.unwrap_or(1);
        let aggregate = step.aggregate.clone().unwrap_or_default();
        let on_error = step.on_error.clone().unwrap_or_default();

        // Create rate limiter and HTTP executor
        let mut rate_limiter = RateLimiter::new(step.rate_limit.clone());
        let http_executor = HttpExecutor::new(
            Some(self.client.clone()),
            ResolvedSecrets::resolve(self.pack.secrets.as_ref())?,
            self.client.query_timeout(),
            self.client.retry_count(),
        )?;

        let mut all_rows: Vec<serde_json::Value> = Vec::new();

        // Get field names for error handling
        let field_names: Vec<String> = step
            .response
            .as_ref()
            .map(|r| r.fields.keys().cloned().collect())
            .unwrap_or_default();

        // Process rows in batches
        for (batch_idx, batch) in source_rows.chunks(batch_size).enumerate() {
            debug!(
                "HTTP step '{}' foreach batch {}/{} ({} rows)",
                step.name,
                batch_idx + 1,
                source_rows.len().div_ceil(batch_size),
                batch.len()
            );

            // For batch_size=1, use single row context
            let foreach_row = batch.first();

            // Build substitutions with foreach context
            let substitutions = self.build_http_substitutions(
                step,
                context,
                Some(&foreach_clause.alias),
                foreach_row,
            )?;

            // Execute HTTP request for this batch
            let result = http_executor
                .execute(step, &substitutions, &mut rate_limiter)
                .await;

            match result {
                Ok(http_result) => {
                    // Aggregate based on strategy
                    match aggregate {
                        crate::investigation_pack::AggregateStrategy::Append => {
                            all_rows.extend(http_result.rows);
                        }
                        crate::investigation_pack::AggregateStrategy::Replace => {
                            all_rows = http_result.rows;
                        }
                        crate::investigation_pack::AggregateStrategy::Merge => {
                            for row in http_result.rows {
                                if !all_rows.contains(&row) {
                                    all_rows.push(row);
                                }
                            }
                        }
                        crate::investigation_pack::AggregateStrategy::Collect => {
                            all_rows.extend(http_result.rows);
                        }
                    }
                }
                Err(e) => {
                    // Handle error based on on_error strategy
                    if let Some(error_row) = handle_http_error(e, &on_error, &field_names)? {
                        all_rows.push(error_row);
                    }
                    // None means skip - don't add anything
                }
            }
        }

        let row_count = all_rows.len();

        // Convert to column format and store results
        let columns = self.extract_columns_from_rows(&all_rows);
        self.finalize_http_step_results(
            step,
            context,
            step_output_dir,
            timestamp,
            columns,
            all_rows,
        )
        .await?;

        Ok(row_count)
    }

    /// Build variable substitutions for HTTP requests
    fn build_http_substitutions(
        &self,
        step: &Step,
        context: &WorkspaceContext,
        foreach_alias: Option<&str>,
        foreach_row: Option<&serde_json::Value>,
    ) -> Result<HashMap<String, String>> {
        let mut substitutions = HashMap::new();

        // Add all inputs
        for (name, value) in &self.inputs {
            substitutions.insert(format!("inputs.{}", name), value.clone());
        }

        // Add step results for dependencies
        for dep_name in &step.depends_on {
            if let Some(results) = context.get_step_results(dep_name) {
                // Add first row values: {{step.first.Column}}
                if let Some(first_row) = results.first() {
                    if let Some(obj) = first_row.as_object() {
                        for (col, val) in obj {
                            let key = format!("{}.first.{}", dep_name, col);
                            substitutions.insert(key, self.json_value_to_string(val));
                        }
                    }
                }

                // Add array values: {{step.*.Column}} - as comma-separated for HTTP
                if let Some(columns) = context.get_step_columns(dep_name) {
                    for col in columns {
                        let values = context.get_column_values(dep_name, col);
                        let key = format!("{}.*.{}", dep_name, col);
                        substitutions.insert(key, values.join(","));
                    }
                }

                // Add indexed values: {{step[N].Column}}
                for (idx, row) in results.iter().enumerate() {
                    if let Some(obj) = row.as_object() {
                        for (col, val) in obj {
                            let key = format!("{}[{}].{}", dep_name, idx, col);
                            substitutions.insert(key, self.json_value_to_string(val));
                        }
                    }
                }
            }
        }

        // Add foreach alias values if present
        if let (Some(alias), Some(row)) = (foreach_alias, foreach_row) {
            if let Some(obj) = row.as_object() {
                for (col, val) in obj {
                    let key = format!("{}.{}", alias, col);
                    substitutions.insert(key, self.json_value_to_string(val));
                }
            }
        }

        Ok(substitutions)
    }

    /// Convert JSON value to string for substitution
    fn json_value_to_string(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => String::new(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => value.to_string(),
        }
    }

    /// Extract column information from JSON row objects
    fn extract_columns_from_rows(&self, rows: &[serde_json::Value]) -> Vec<Column> {
        if rows.is_empty() {
            return vec![];
        }

        // Use first row to determine columns
        if let Some(first_row) = rows.first() {
            if let Some(obj) = first_row.as_object() {
                return obj
                    .keys()
                    .map(|k| Column {
                        name: k.clone(),
                        column_type: "string".to_string(), // HTTP responses are untyped
                    })
                    .collect();
            }
        }

        vec![]
    }

    /// Finalize HTTP step results: write to files and store in context
    async fn finalize_http_step_results(
        &self,
        step: &Step,
        context: &mut WorkspaceContext,
        step_output_dir: &Path,
        timestamp: &str,
        columns: Vec<Column>,
        rows: Vec<serde_json::Value>,
    ) -> Result<()> {
        // Write results to files
        self.write_http_step_results(
            step_output_dir,
            &step.name,
            &columns,
            &rows,
            &context.workspace,
            timestamp,
        )
        .await?;

        // Store step results in context for variable substitution
        let column_names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();
        context.set_step_results(&step.name, column_names, rows);

        Ok(())
    }

    /// Write HTTP step results to files
    async fn write_http_step_results(
        &self,
        output_dir: &Path,
        step_name: &str,
        columns: &[Column],
        rows: &[serde_json::Value],
        workspace: &Workspace,
        timestamp: &str,
    ) -> Result<()> {
        // Write CSV
        let csv_path = output_dir.join("results.csv");
        self.write_http_csv(&csv_path, columns, rows).await?;

        // Write JSON
        let json_path = output_dir.join("results.json");
        self.write_http_json(&json_path, columns, rows, workspace, timestamp, step_name)
            .await?;

        Ok(())
    }

    /// Write HTTP results as CSV
    async fn write_http_csv(
        &self,
        path: &Path,
        columns: &[Column],
        rows: &[serde_json::Value],
    ) -> Result<()> {
        let mut content = String::new();

        // Header
        let headers: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
        content.push_str(&headers.join(","));
        content.push('\n');

        // Rows (HTTP rows are already objects)
        for row in rows {
            if let Some(obj) = row.as_object() {
                let values: Vec<String> = columns
                    .iter()
                    .map(|col| {
                        obj.get(&col.name)
                            .map(|v| self.format_csv_value(v))
                            .unwrap_or_default()
                    })
                    .collect();
                content.push_str(&values.join(","));
                content.push('\n');
            }
        }

        tokio::fs::write(path, content).await?;
        Ok(())
    }

    /// Write HTTP results as JSON
    async fn write_http_json(
        &self,
        path: &Path,
        columns: &[Column],
        rows: &[serde_json::Value],
        workspace: &Workspace,
        timestamp: &str,
        step_name: &str,
    ) -> Result<()> {
        let output = serde_json::json!({
            "metadata": {
                "step": step_name,
                "step_type": "http",
                "workspace": workspace.name,
                "workspace_id": workspace.workspace_id,
                "subscription": workspace.subscription_name,
                "timestamp": timestamp,
                "row_count": rows.len(),
            },
            "columns": columns.iter().map(|col| {
                serde_json::json!({
                    "name": col.name,
                    "type": col.column_type,
                })
            }).collect::<Vec<_>>(),
            "rows": rows,
        });

        let content = serde_json::to_string_pretty(&output)?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }
}
