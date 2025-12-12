//! Run command - execute packs or ad-hoc queries

use crate::context::SharedContext;
use crate::history::{ExecutionRecord, ExecutionSource, ExecutionStatusSummary, ExecutionSummary};
use super::CommandResult;
use anyhow::Result;
use kql_panopticon_core::{
    ExecutionEngine, PackExecutor, PackExecutorConfig, StepStatus,
};
use std::collections::HashMap;

/// Execute loaded pack or ad-hoc query
pub async fn execute(
    query: Option<String>,
    timespan: Option<String>,
    all: bool,
    ctx: SharedContext,
) -> Result<CommandResult> {
    // Determine what to run
    match query {
        Some(q) => execute_adhoc_query(q, timespan, all, ctx).await,
        None => execute_pack(all, ctx).await,
    }
}

/// Execute an ad-hoc query
async fn execute_adhoc_query(
    query: String,
    timespan: Option<String>,
    all: bool,
    ctx: SharedContext,
) -> Result<CommandResult> {
    // Get workspaces
    let (client, workspaces) = {
        let mut ctx = ctx.write().await;
        if !ctx.is_initialized() {
            println!("Connecting to Azure...");
            ctx.initialize().await?;
        }

        let client = ctx.client().cloned().ok_or_else(|| {
            anyhow::anyhow!("Not connected. Run 'workspace list' first")
        })?;

        let workspaces = if all {
            ctx.available_workspaces().to_vec()
        } else {
            let selected = ctx.selected_workspaces().to_vec();
            if selected.is_empty() {
                return Ok(CommandResult::error(
                    "No workspace selected. Use 'workspace select <name>' or 'run --all'"
                ));
            }
            selected
        };

        (client, workspaces)
    };

    println!("Executing on {} workspace(s)...", workspaces.len());

    // Create execution record
    let workspace_names: Vec<String> = workspaces.iter().map(|w| w.name.clone()).collect();
    let mut record = ExecutionRecord::new(
        ExecutionSource::AdHocQuery {
            query: query.clone(),
            timespan: timespan.clone(),
        },
        workspace_names.clone(),
    );

    // Execute on each workspace
    let start = std::time::Instant::now();
    let mut total_rows = 0;
    let mut failed = false;
    let mut output = String::new();

    for ws in &workspaces {
        print!("  {} ... ", ws.name);

        match client.query_workspace(&ws.workspace_id, &query, timespan.as_deref()).await {
            Ok(response) => {
                let rows = response
                    .tables
                    .first()
                    .map(|t| t.rows.len())
                    .unwrap_or(0);
                total_rows += rows;
                println!("{} rows", rows);
                output.push_str(&format!("  {} - {} rows\n", ws.name, rows));
            }
            Err(e) => {
                failed = true;
                println!("ERROR: {}", e);
                output.push_str(&format!("  {} - ERROR: {}\n", ws.name, e));
            }
        }
    }

    let duration = start.elapsed();

    // Update record
    record.result = ExecutionSummary {
        status: if failed {
            ExecutionStatusSummary::PartialSuccess
        } else {
            ExecutionStatusSummary::Completed
        },
        duration_ms: duration.as_millis() as u64,
        total_rows,
        steps_completed: 1,
        steps_failed: if failed { 1 } else { 0 },
        steps_skipped: 0,
    };

    // Save to history
    {
        let mut ctx = ctx.write().await;
        ctx.record_execution(record);
    }

    let mut result = String::new();
    result.push_str(&format!(
        "\nCompleted in {:.2}s, {} total rows\n",
        duration.as_secs_f64(),
        total_rows
    ));
    result.push_str(&output);

    Ok(CommandResult::output(result))
}

/// Execute loaded pack
async fn execute_pack(all: bool, ctx: SharedContext) -> Result<CommandResult> {
    // Get pack, client, workspaces
    let (pack, pack_path, client, workspaces, output_dir) = {
        let mut ctx = ctx.write().await;
        if !ctx.is_initialized() {
            println!("Connecting to Azure...");
            ctx.initialize().await?;
        }

        let loaded = ctx.loaded_pack().ok_or_else(|| {
            anyhow::anyhow!("No pack loaded. Use 'pack load <path>'")
        })?;
        let pack = loaded.pack.clone();
        let pack_path = loaded.path.clone();

        let client = ctx.client().cloned().ok_or_else(|| {
            anyhow::anyhow!("Not connected")
        })?;

        let workspaces = if all {
            ctx.available_workspaces().to_vec()
        } else {
            let selected = ctx.selected_workspaces().to_vec();
            if selected.is_empty() {
                return Ok(CommandResult::error(
                    "No workspace selected. Use 'workspace select <name>' or 'run --all'"
                ));
            }
            selected
        };

        let output_dir = ctx.output_dir().clone();

        (pack, pack_path, client, workspaces, output_dir)
    };

    // Check for required inputs (for now, just warn)
    let required = pack.required_inputs();
    if !required.is_empty() {
        let names: Vec<_> = required.iter().map(|i| i.name.as_str()).collect();
        println!(
            "Warning: Pack requires inputs: {}",
            names.join(", ")
        );
        println!("Use --set to provide inputs (not yet implemented)\n");
    }

    println!(
        "Executing pack '{}' on {} workspace(s)...\n",
        pack.name,
        workspaces.len()
    );

    // Create execution record
    let workspace_names: Vec<String> = workspaces.iter().map(|w| w.name.clone()).collect();
    let mut record = ExecutionRecord::new(
        ExecutionSource::Pack {
            name: pack.name.clone(),
            path: pack_path.clone(),
            inputs: HashMap::new(), // TODO: Add input support
        },
        workspace_names,
    );

    // Create executor and config
    let executor = PackExecutor::new(client);
    let config = PackExecutorConfig::new(pack.clone())
        .with_output_dir(output_dir.clone());

    // Execute
    let start = std::time::Instant::now();
    let result = executor.execute(config, workspaces, None).await?;
    let duration = start.elapsed();

    // Build output
    let mut output = String::new();

    // Show per-workspace results
    for (ws_name, ws_result) in &result.workspace_results {
        output.push_str(&format!("\n{}:\n", ws_name));
        for (step_name, step_result) in &ws_result.step_results {
            let status_icon = match step_result.status {
                StepStatus::Success => "✓",
                StepStatus::Failed => "✗",
                StepStatus::Skipped => "○",
                StepStatus::Pending | StepStatus::Running => "·",
            };
            output.push_str(&format!(
                "  {} {} - {} rows ({}ms)\n",
                status_icon,
                step_name,
                step_result.row_count.unwrap_or(0),
                step_result.duration_ms
            ));
            if let Some(err) = &step_result.error {
                output.push_str(&format!("    Error: {}\n", err));
            }
        }
    }

    // Calculate totals
    let total_rows: usize = result
        .workspace_results
        .values()
        .flat_map(|ws| ws.step_results.values())
        .filter_map(|s| s.row_count)
        .sum();

    let steps_completed = result
        .workspace_results
        .values()
        .flat_map(|ws| ws.step_results.values())
        .filter(|s| matches!(s.status, StepStatus::Success))
        .count();

    let steps_failed = result
        .workspace_results
        .values()
        .flat_map(|ws| ws.step_results.values())
        .filter(|s| matches!(s.status, StepStatus::Failed))
        .count();

    // Update record
    record.result = ExecutionSummary {
        status: ExecutionStatusSummary::from(result.status),
        duration_ms: duration.as_millis() as u64,
        total_rows,
        steps_completed,
        steps_failed,
        steps_skipped: 0,
    };
    record.output_path = result.output_dir.clone();

    // Save to history
    {
        let mut ctx = ctx.write().await;
        ctx.record_execution(record);
    }

    output.push_str(&format!(
        "\nCompleted in {:.2}s, {} total rows",
        duration.as_secs_f64(),
        total_rows
    ));

    if let Some(path) = &result.output_dir {
        output.push_str(&format!("\nOutput: {}", path.display()));
    }

    Ok(CommandResult::output(output))
}

/// Validate a query, session steps, or loaded pack
pub async fn validate(
    step: Option<String>,
    query: Option<String>,
    ctx: SharedContext,
) -> Result<CommandResult> {
    let ctx = ctx.read().await;

    // Get schema for validation if available
    let workspace_id = ctx.selected_workspaces().first().map(|ws| ws.workspace_id.as_str());
    let schema = ctx.schema_registry().map(|r| r.to_validation_schema(workspace_id));

    // Priority: query > step > session > pack
    if let Some(q) = query {
        return validate_single_query(&q, schema.as_ref());
    }

    // Validate specific session step
    if let Some(step_name) = step {
        return validate_session_step(&step_name, ctx.pack_session(), schema.as_ref());
    }

    // If session has steps, validate those
    if !ctx.pack_session().steps.is_empty() {
        return validate_session_steps(ctx.pack_session(), schema.as_ref());
    }

    // Otherwise validate loaded pack
    let loaded = ctx.loaded_pack().ok_or_else(|| {
        anyhow::anyhow!("No session steps, no pack loaded, and no query provided.\nDefine steps with 'query <name> = \"<kql>\"' or load a pack with 'pack load <path>'")
    })?;

    validate_loaded_pack(loaded, schema.as_ref())
}

/// Validate a single session step by name
fn validate_session_step(
    step_name: &str,
    session: &crate::session::PackSession,
    schema: Option<&kql_panopticon_core::validation::Schema>,
) -> Result<CommandResult> {
    let step = session.get_step(step_name).ok_or_else(|| {
        anyhow::anyhow!("Step '{}' not found", step_name)
    })?;

    let validator = match kql_panopticon_core::validation::KqlValidator::new() {
        Ok(v) => v,
        Err(e) => return Ok(CommandResult::error(format!("Validator unavailable: {}", e))),
    };

    // Prepare query for validation (substitute references)
    let prepared_query = session.prepare_query_for_validation(&step.query);

    // Use schema-aware validation if available
    let result = match schema {
        Some(s) if validator.supports_schema_validation() => {
            validator.validate_with_schema(&prepared_query, s)
        }
        _ => validator.validate_syntax(&prepared_query),
    };

    let validation_type = if schema.is_some() { "schema" } else { "syntax" };

    match result {
        Ok(r) if r.is_valid() => {
            Ok(CommandResult::message(format!(
                "\x1b[32m✓\x1b[0m Step '{}' is valid ({} validation)",
                step_name, validation_type
            )))
        }
        Ok(r) => {
            let errs: Vec<_> = r.errors().collect();
            let mut output = format!(
                "\x1b[31m✗\x1b[0m Step '{}' has {} error(s) ({} validation):\n",
                step_name,
                errs.len(),
                validation_type
            );
            for err in errs {
                output.push_str(&format!(
                    "  Line {}, Col {}: {}\n",
                    err.line, err.column, err.message
                ));
            }
            if step.query != prepared_query {
                output.push_str("\nNote: References were substituted with example values for validation.");
            }
            Ok(CommandResult::output(output))
        }
        Err(e) => Ok(CommandResult::error(format!("Validation error: {}", e))),
    }
}

/// Validate all session steps
fn validate_session_steps(
    session: &crate::session::PackSession,
    schema: Option<&kql_panopticon_core::validation::Schema>,
) -> Result<CommandResult> {
    let validator = match kql_panopticon_core::validation::KqlValidator::new() {
        Ok(v) => Some(v),
        Err(e) => {
            return Ok(CommandResult::output(format!(
                "Warning: Validator unavailable: {}\nCannot validate session steps.",
                e
            )));
        }
    };

    let validator = validator.unwrap();
    let validation_type = if schema.is_some() { "schema" } else { "syntax" };
    let mut output = format!(
        "Validating {} session step(s) ({} validation)...\n\n",
        session.steps.len(),
        validation_type
    );
    let mut valid_count = 0;
    let mut error_count = 0;

    for (name, step) in &session.steps {
        let prepared_query = session.prepare_query_for_validation(&step.query);

        let result = match schema {
            Some(s) if validator.supports_schema_validation() => {
                validator.validate_with_schema(&prepared_query, s)
            }
            _ => validator.validate_syntax(&prepared_query),
        };

        match result {
            Ok(r) if r.is_valid() => {
                output.push_str(&format!("  \x1b[32m✓\x1b[0m {}\n", name));
                valid_count += 1;
            }
            Ok(r) => {
                let errs: Vec<_> = r.errors().collect();
                output.push_str(&format!(
                    "  \x1b[31m✗\x1b[0m {} ({} error(s))\n",
                    name,
                    errs.len()
                ));
                for err in errs {
                    output.push_str(&format!(
                        "      Line {}, Col {}: {}\n",
                        err.line, err.column, err.message
                    ));
                }
                error_count += 1;
            }
            Err(e) => {
                output.push_str(&format!("  \x1b[33m?\x1b[0m {} (error: {})\n", name, e));
            }
        }
    }

    if error_count > 0 {
        output.push_str(&format!(
            "\nValidation: {} valid, {} with errors",
            valid_count, error_count
        ));
    } else {
        output.push_str(&format!("\n\x1b[32m✓\x1b[0m All {} steps valid", valid_count));
    }

    Ok(CommandResult::output(output))
}

/// Validate a loaded pack
fn validate_loaded_pack(
    loaded: &crate::context::LoadedPack,
    schema: Option<&kql_panopticon_core::validation::Schema>,
) -> Result<CommandResult> {
    let mut output = String::new();
    let mut has_errors = false;

    let validation_type = if schema.is_some() { "schema" } else { "syntax" };
    output.push_str(&format!(
        "Validating pack '{}' ({} validation)...\n\n",
        loaded.pack.name, validation_type
    ));

    let validator = match kql_panopticon_core::validation::KqlValidator::new() {
        Ok(v) => Some(v),
        Err(e) => {
            output.push_str(&format!("Warning: Validator unavailable: {}\n\n", e));
            None
        }
    };

    for step in &loaded.pack.steps {
        if let Some(q) = &step.query {
            if let Some(ref v) = validator {
                // Use schema-aware validation if available
                let result = match schema {
                    Some(s) if v.supports_schema_validation() => {
                        v.validate_with_schema(q, s)
                    }
                    _ => v.validate_syntax(q),
                };

                match result {
                    Ok(r) if r.is_valid() => {
                        output.push_str(&format!("  \x1b[32m✓\x1b[0m {}\n", step.name));
                    }
                    Ok(r) => {
                        has_errors = true;
                        let errs: Vec<_> = r.errors().collect();
                        output.push_str(&format!(
                            "  \x1b[31m✗\x1b[0m {} ({} error(s))\n",
                            step.name,
                            errs.len()
                        ));
                        for err in errs {
                            output.push_str(&format!(
                                "      Line {}, Col {}: {}\n",
                                err.line, err.column, err.message
                            ));
                        }
                    }
                    Err(e) => {
                        output.push_str(&format!(
                            "  \x1b[33m?\x1b[0m {} (error: {})\n",
                            step.name, e
                        ));
                    }
                }
            } else {
                output.push_str(&format!("  ? {} (validator unavailable)\n", step.name));
            }
        }
    }

    if has_errors {
        output.push_str("\nValidation completed with errors");
    } else if validator.is_some() {
        output.push_str(&format!("\n\x1b[32m✓\x1b[0m All {} queries valid", loaded.pack.steps.len()));
    }

    Ok(CommandResult::output(output))
}

fn validate_single_query(
    query: &str,
    schema: Option<&kql_panopticon_core::validation::Schema>,
) -> Result<CommandResult> {
    let validator = match kql_panopticon_core::validation::KqlValidator::new() {
        Ok(v) => v,
        Err(e) => return Ok(CommandResult::error(format!("Validator unavailable: {}", e))),
    };

    // Use schema-aware validation if available
    let result = match schema {
        Some(s) if validator.supports_schema_validation() => {
            validator.validate_with_schema(query, s)
        }
        _ => validator.validate_syntax(query),
    };

    let validation_type = if schema.is_some() { "schema" } else { "syntax" };

    match result {
        Ok(r) if r.is_valid() => {
            Ok(CommandResult::message(format!(
                "\x1b[32m✓\x1b[0m Query is valid ({} validation)",
                validation_type
            )))
        }
        Ok(r) => {
            let mut output = String::new();
            let errs: Vec<_> = r.errors().collect();
            output.push_str(&format!(
                "Query has {} error(s) ({} validation):\n",
                errs.len(),
                validation_type
            ));
            for err in errs {
                output.push_str(&format!(
                    "  Line {}, Col {}: {}\n",
                    err.line, err.column, err.message
                ));
            }
            Ok(CommandResult::output(output))
        }
        Err(e) => {
            Ok(CommandResult::error(format!("Validation error: {}", e)))
        }
    }
}
