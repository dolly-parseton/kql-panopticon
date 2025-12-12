//! Run command - execute packs, sessions, or ad-hoc queries

use crate::context::SharedContext;
use crate::history::{ExecutionRecord, ExecutionSource, ExecutionStatusSummary, ExecutionSummary};
use crate::input_form::{InputForm, InputFormResult};
use crate::progress_display::{create_progress_channel, run_progress_display};
use crate::session::{InputDef, PackSession};
use super::CommandResult;
use anyhow::Result;
use kql_panopticon_core::pack::Pack;
use kql_panopticon_core::{ExecutionEngine, PackExecutor, PackExecutorConfig, StepStatus};
use std::collections::HashMap;
use std::path::PathBuf;

/// Execute loaded pack, session steps, pack file, or ad-hoc query
pub async fn execute(
    path: Option<PathBuf>,
    query: Option<String>,
    timespan: Option<String>,
    all: bool,
    ctx: SharedContext,
) -> Result<CommandResult> {
    // Determine what to run based on arguments
    match (query, path) {
        // Ad-hoc query takes priority
        (Some(q), _) => execute_adhoc_query(q, timespan, all, ctx).await,
        // Direct pack file path
        (None, Some(p)) => execute_pack_file(p, all, ctx).await,
        // Session steps or loaded pack
        (None, None) => execute_session_or_pack(all, ctx).await,
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

/// Execute a pack file directly (without loading it into session)
async fn execute_pack_file(path: PathBuf, all: bool, ctx: SharedContext) -> Result<CommandResult> {
    // Load pack from file
    let pack = Pack::load_from_file(&path)
        .map_err(|e| anyhow::anyhow!("Failed to load pack file: {}", e))?;

    println!("Loaded pack: {}", pack.name);

    // Collect inputs if required
    let inputs = collect_pack_inputs(&pack)?;
    if inputs.is_none() {
        return Ok(CommandResult::message("Cancelled"));
    }
    let inputs = inputs.unwrap();

    // Execute the pack
    execute_pack_with_inputs(pack, Some(path), inputs, all, ctx).await
}

/// Execute session steps or loaded pack
async fn execute_session_or_pack(all: bool, ctx: SharedContext) -> Result<CommandResult> {
    // Check if session has steps
    let session_has_steps = {
        let ctx = ctx.read().await;
        !ctx.pack_session().steps.is_empty()
    };

    if session_has_steps {
        execute_session(all, ctx).await
    } else {
        execute_loaded_pack(all, ctx).await
    }
}

/// Execute session steps
async fn execute_session(all: bool, ctx: SharedContext) -> Result<CommandResult> {
    // Get session and convert to pack
    let (pack, inputs) = {
        let ctx = ctx.read().await;
        let session = ctx.pack_session();

        // Collect required inputs
        let required_inputs: Vec<InputDef> = session.inputs.values().cloned().collect();
        let pack: Pack = session.into();

        (pack, required_inputs)
    };

    // Prompt for inputs if any
    let input_values = if inputs.is_empty() {
        HashMap::new()
    } else {
        match collect_session_inputs(&inputs)? {
            Some(values) => values,
            None => return Ok(CommandResult::message("Cancelled")),
        }
    };

    // Execute
    execute_pack_with_inputs(pack, None, input_values, all, ctx).await
}

/// Execute a loaded pack (from 'pack load')
async fn execute_loaded_pack(all: bool, ctx: SharedContext) -> Result<CommandResult> {
    // Get pack from context
    let (pack, pack_path) = {
        let ctx = ctx.read().await;
        let loaded = ctx.loaded_pack().ok_or_else(|| {
            anyhow::anyhow!(
                "No session steps defined and no pack loaded.\n\
                Define steps with 'query <name> = \"<kql>\"' or load a pack with 'pack load <path>'"
            )
        })?;
        (loaded.pack.clone(), loaded.path.clone())
    };

    // Collect inputs if required
    let inputs = collect_pack_inputs(&pack)?;
    if inputs.is_none() {
        return Ok(CommandResult::message("Cancelled"));
    }
    let inputs = inputs.unwrap();

    execute_pack_with_inputs(pack, Some(pack_path), inputs, all, ctx).await
}

/// Execute a pack with provided inputs
async fn execute_pack_with_inputs(
    pack: Pack,
    pack_path: Option<PathBuf>,
    inputs: HashMap<String, String>,
    all: bool,
    ctx: SharedContext,
) -> Result<CommandResult> {
    // Get client and workspaces
    let (client, workspaces, output_dir) = {
        let mut ctx = ctx.write().await;
        if !ctx.is_initialized() {
            println!("Connecting to Azure...");
            ctx.initialize().await?;
        }

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

        (client, workspaces, output_dir)
    };

    println!(
        "Executing '{}' on {} workspace(s)...\n",
        pack.name,
        workspaces.len()
    );

    // Get workspace names for display and execution record
    let workspace_names: Vec<String> = workspaces.iter().map(|w| w.name.clone()).collect();
    let display_workspace = workspace_names.first().cloned().unwrap_or_default();

    // Create execution record
    let mut record = ExecutionRecord::new(
        ExecutionSource::Pack {
            name: pack.name.clone(),
            path: pack_path.clone().unwrap_or_default(),
            inputs: inputs.clone(),
        },
        workspace_names,
    );

    // Create executor and config
    let executor = PackExecutor::new(client);
    let mut config = PackExecutorConfig::new(pack.clone())
        .with_output_dir(output_dir.clone())
        .with_inputs(inputs);

    if let Some(path) = &pack_path {
        config = config.with_pack_path(path);
    }

    // Create progress channel
    let (progress_sender, progress_receiver, _job_id) = create_progress_channel();

    // Get step names for progress display
    let step_names: Vec<String> = pack.steps.iter().map(|s| s.name.clone()).collect();

    // Spawn progress display task
    let progress_handle = tokio::spawn(run_progress_display(
        step_names,
        display_workspace,
        progress_receiver,
    ));

    // Execute
    let start = std::time::Instant::now();
    let result = executor.execute(config, workspaces, Some(progress_sender)).await?;
    let duration = start.elapsed();

    // Wait for progress display to finish
    let _ = progress_handle.await;

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

    // Build summary output
    let mut output = format!(
        "\nCompleted in {:.2}s, {} total rows",
        duration.as_secs_f64(),
        total_rows
    );

    if let Some(path) = &result.output_dir {
        output.push_str(&format!("\nOutput: {}", path.display()));
    }

    Ok(CommandResult::output(output))
}

/// Sample a single step with limited results
pub async fn sample(step_name: String, limit: usize, ctx: SharedContext) -> Result<CommandResult> {
    // Get session and find the step
    let (pack, inputs_needed) = {
        let ctx = ctx.read().await;
        let session = ctx.pack_session();

        let step = session.get_step(&step_name).ok_or_else(|| {
            anyhow::anyhow!("Step '{}' not found in session", step_name)
        })?;

        // Find inputs referenced by this step
        let inputs_needed: Vec<InputDef> = step
            .references_inputs
            .iter()
            .filter_map(|name| session.inputs.get(name).cloned())
            .collect();

        // Create a minimal pack with just this step (and dependencies)
        let mut sample_session = PackSession::with_name(format!("Sample: {}", step_name));

        // Add required inputs
        for input in &inputs_needed {
            sample_session.add_input(input.clone());
        }

        // Add the step with limit appended
        let modified_query = format!("{} | take {}", step.query.trim_end(), limit);
        let mut modified_step = step.clone();
        modified_step.query = modified_query;
        sample_session.add_step(modified_step);

        // Add dependent steps
        for dep_name in &step.depends_on {
            if let Some(dep_step) = session.get_step(dep_name) {
                sample_session.add_step(dep_step.clone());
            }
        }

        let pack: Pack = (&sample_session).into();
        (pack, inputs_needed)
    };

    // Prompt for inputs if any
    let input_values = if inputs_needed.is_empty() {
        HashMap::new()
    } else {
        match collect_session_inputs(&inputs_needed)? {
            Some(values) => values,
            None => return Ok(CommandResult::message("Cancelled")),
        }
    };

    // Execute with sampling message
    println!("Sampling step '{}' (limit: {} rows)...\n", step_name, limit);
    execute_pack_with_inputs(pack, None, input_values, false, ctx).await
}

/// Collect input values from user using the input form
fn collect_session_inputs(inputs: &[InputDef]) -> Result<Option<HashMap<String, String>>> {
    if inputs.is_empty() {
        return Ok(Some(HashMap::new()));
    }

    let form = InputForm::new("Enter Input Values", inputs);
    match form.run()? {
        InputFormResult::Submitted(values) => Ok(Some(values)),
        InputFormResult::Cancelled => Ok(None),
    }
}

/// Collect input values for a pack
fn collect_pack_inputs(pack: &Pack) -> Result<Option<HashMap<String, String>>> {
    let required_inputs: Vec<_> = pack.inputs.iter().filter(|i| i.required).collect();

    if required_inputs.is_empty() {
        // Use defaults for optional inputs
        let defaults: HashMap<_, _> = pack
            .inputs
            .iter()
            .filter_map(|i| i.default.as_ref().map(|d| (i.name.clone(), d.clone())))
            .collect();
        return Ok(Some(defaults));
    }

    // Convert to InputDef for the form
    let input_defs: Vec<InputDef> = pack
        .inputs
        .iter()
        .map(|i| InputDef {
            name: i.name.clone(),
            input_type: crate::session::InputType::String, // Pack doesn't have type info
            description: i.description.clone(),
            required: i.required,
            default: i.default.clone(),
        })
        .collect();

    let form = InputForm::new("Enter Input Values", &input_defs);
    match form.run()? {
        InputFormResult::Submitted(values) => Ok(Some(values)),
        InputFormResult::Cancelled => Ok(None),
    }
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
    session: &PackSession,
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
    session: &PackSession,
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
