use crate::{
    client::Client,
    error::{KqlPanopticonError, Result},
    investigation_job::{InvestigationRunner, ProgressUpdate, Status},
    investigation_pack::InvestigationPack,
    workspace::Workspace,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub async fn execute(
    pack_path: String,
    workspaces_override: Option<String>,
    inputs: Vec<(String, String)>,
    output_override: Option<PathBuf>,
    validate_only: bool,
    json_output: bool,
) -> Result<()> {
    // Load pack
    let (pack, resolved_path) = load_pack(&pack_path)?;

    // Validate
    pack.validate()?;

    if validate_only {
        print_validation_result(&pack, json_output);
        return Ok(());
    }

    // Convert inputs to HashMap
    let input_map: HashMap<String, String> = inputs.into_iter().collect();

    // Check required inputs
    validate_inputs(&pack, &input_map)?;

    // Initialize client
    let client = Client::new()?;

    if !json_output {
        eprintln!("Authenticating with Azure...");
    }
    client.force_validate_auth().await?;

    if !json_output {
        eprintln!("Loading workspaces...");
    }
    let all_workspaces = client.list_workspaces().await?;

    // Determine workspace selection
    let selected_workspaces = select_workspaces(&all_workspaces, workspaces_override)?;

    if selected_workspaces.is_empty() {
        return Err(KqlPanopticonError::InvestigationPackValidation(
            "No workspaces selected for execution".into(),
        ));
    }

    if !json_output {
        eprintln!(
            "Running investigation '{}' with {} step{} across {} workspace{}...",
            pack.name,
            pack.steps.len(),
            if pack.steps.len() == 1 { "" } else { "s" },
            selected_workspaces.len(),
            if selected_workspaces.len() == 1 { "" } else { "s" }
        );
    }

    // Set up progress channel
    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<ProgressUpdate>();

    // Determine output folder
    let output_base = output_override.unwrap_or_else(|| PathBuf::from("."));

    // Create runner
    let runner = InvestigationRunner::new(
        client,
        pack.clone(),
        selected_workspaces,
        input_map,
        output_base,
    )
    .with_pack_path(resolved_path);

    // Spawn progress printer if not JSON output
    let progress_handle = if !json_output {
        Some(tokio::spawn(async move {
            while let Some(update) = progress_rx.recv().await {
                print_progress(&update);
            }
        }))
    } else {
        // Drain the channel but don't print
        Some(tokio::spawn(async move {
            while progress_rx.recv().await.is_some() {}
        }))
    };

    // Run investigation
    let result = runner.run(Some(progress_tx)).await;

    // Wait for progress printer to finish
    if let Some(handle) = progress_handle {
        let _ = handle.await;
    }

    match result {
        Ok(investigation_result) => {
            if json_output {
                println!("{}", serde_json::to_string_pretty(&investigation_result)?);
            } else {
                print_summary(&investigation_result);
            }
            Ok(())
        }
        Err(e) => {
            if json_output {
                let error_json = serde_json::json!({
                    "status": "failed",
                    "error": e.to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&error_json)?);
            }
            Err(e)
        }
    }
}

fn load_pack(path_str: &str) -> Result<(InvestigationPack, PathBuf)> {
    let path = Path::new(path_str);

    // If absolute path, use directly
    if path.is_absolute() {
        let pack = InvestigationPack::load_from_file(path)?;
        return Ok((pack, path.to_path_buf()));
    }

    // Try as relative path first
    if path.exists() {
        let pack = InvestigationPack::load_from_file(path)?;
        return Ok((pack, path.to_path_buf()));
    }

    // Try in library location
    let library_path = InvestigationPack::get_library_path(path_str)?;
    if library_path.exists() {
        let pack = InvestigationPack::load_from_file(&library_path)?;
        return Ok((pack, library_path));
    }

    Err(KqlPanopticonError::InvestigationPackNotFound(
        path_str.to_string(),
    ))
}

fn validate_inputs(pack: &InvestigationPack, provided: &HashMap<String, String>) -> Result<()> {
    let mut missing = Vec::new();

    for input in &pack.inputs {
        if input.required && !provided.contains_key(&input.name) && input.default.is_none() {
            missing.push(input.name.clone());
        }
    }

    if !missing.is_empty() {
        return Err(KqlPanopticonError::InvestigationPackValidation(format!(
            "Missing required input(s): {}. Use --set name=value to provide them.",
            missing.join(", ")
        )));
    }

    Ok(())
}

fn select_workspaces(
    all_workspaces: &[Workspace],
    cli_override: Option<String>,
) -> Result<Vec<Workspace>> {
    match cli_override {
        Some(spec) if spec == "all" => Ok(all_workspaces.to_vec()),
        Some(spec) => {
            // Comma-separated IDs or names
            let ids: Vec<&str> = spec.split(',').map(|s| s.trim()).collect();
            let selected: Vec<Workspace> = all_workspaces
                .iter()
                .filter(|ws| {
                    ids.iter()
                        .any(|id| ws.workspace_id.contains(id) || ws.name.contains(id))
                })
                .cloned()
                .collect();

            if selected.is_empty() {
                return Err(KqlPanopticonError::InvestigationPackValidation(format!(
                    "No workspaces matched: {}",
                    spec
                )));
            }

            Ok(selected)
        }
        None => {
            // Default to all workspaces
            Ok(all_workspaces.to_vec())
        }
    }
}

fn print_validation_result(pack: &InvestigationPack, json_output: bool) {
    if json_output {
        let validation = serde_json::json!({
            "valid": true,
            "name": pack.name,
            "description": pack.description,
            "steps": pack.steps.len(),
            "inputs": pack.inputs.iter().map(|i| {
                serde_json::json!({
                    "name": i.name,
                    "description": i.description,
                    "required": i.required,
                    "has_default": i.default.is_some(),
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&validation).unwrap());
    } else {
        eprintln!("✓ Investigation pack is valid");
        eprintln!("  Name: {}", pack.name);
        if let Some(desc) = &pack.description {
            eprintln!("  Description: {}", desc);
        }
        eprintln!("  Steps: {}", pack.steps.len());

        if !pack.inputs.is_empty() {
            eprintln!("  Inputs:");
            for input in &pack.inputs {
                let required = if input.required { " (required)" } else { "" };
                let default = input
                    .default
                    .as_ref()
                    .map(|d| format!(" [default: {}]", d))
                    .unwrap_or_default();
                eprintln!("    - {}{}{}", input.name, required, default);
                if let Some(desc) = &input.description {
                    eprintln!("      {}", desc);
                }
            }
        }

        eprintln!("  Steps:");
        for step in &pack.steps {
            let deps = if step.depends_on.is_empty() {
                String::new()
            } else {
                format!(" (depends on: {})", step.depends_on.join(", "))
            };
            let extracts = if step.extract.is_empty() {
                String::new()
            } else {
                format!(
                    " -> extracts: {}",
                    step.extract.keys().cloned().collect::<Vec<_>>().join(", ")
                )
            };
            eprintln!("    - {}{}{}", step.name, deps, extracts);
        }
    }
}

fn print_progress(update: &ProgressUpdate) {
    match update {
        ProgressUpdate::Started {
            workspace_count,
            step_count,
        } => {
            eprintln!(
                "\n▶ Starting investigation ({} workspaces, {} steps)",
                workspace_count, step_count
            );
        }
        ProgressUpdate::StepStarted {
            workspace_name,
            step_name,
        } => {
            eprintln!("  ⏳ {} / {}", workspace_name, step_name);
        }
        ProgressUpdate::StepCompleted {
            workspace_name,
            step_name,
            rows,
            duration,
        } => {
            eprintln!(
                "  ✓ {} / {} - {} rows ({:.2}s)",
                workspace_name,
                step_name,
                rows,
                duration.as_secs_f64()
            );
        }
        ProgressUpdate::StepFailed {
            workspace_name,
            step_name,
            error,
        } => {
            eprintln!("  ✗ {} / {} - {}", workspace_name, step_name, error);
        }
        ProgressUpdate::Completed { result } => {
            eprintln!("\n▶ Investigation completed");
            eprintln!("  Output: {}", result.output_folder.display());
        }
    }
}

fn print_summary(result: &crate::investigation_job::InvestigationResult) {
    eprintln!("\n--- Investigation Summary ---");
    eprintln!("Name: {}", result.investigation_name);
    eprintln!(
        "Status: {}",
        match result.status {
            Status::Success => "✓ Success",
            Status::Failed => "✗ Failed",
            _ => "Unknown",
        }
    );
    eprintln!("Output: {}", result.output_folder.display());

    if let Some(reason) = &result.failure_reason {
        eprintln!("\nFailure reason: {}", reason);
    }

    // Per-workspace summary
    let mut success_count = 0;
    let mut failed_count = 0;

    for (workspace_id, ws_result) in &result.workspaces {
        match ws_result.status {
            Status::Success => success_count += 1,
            Status::Failed => {
                failed_count += 1;
                if let (Some(step), Some(reason)) =
                    (&ws_result.failure_step, &ws_result.failure_reason)
                {
                    eprintln!("\n  Workspace {} failed at step '{}': {}", workspace_id, step, reason);
                }
            }
            _ => {}
        }
    }

    eprintln!(
        "\nWorkspaces: {} succeeded, {} failed",
        success_count, failed_count
    );
}
