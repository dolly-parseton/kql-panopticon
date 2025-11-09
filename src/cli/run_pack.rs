use crate::cli::args::OutputFormat;
use crate::{
    client::Client,
    error::Result,
    query_job::{QueryJobBuilder, QueryJobResult},
    query_pack::{QueryPack, WorkspaceScope},
    workspace::Workspace,
};
use std::path::Path;

pub async fn execute(
    pack_path: String,
    workspaces_override: Option<String>,
    format: OutputFormat,
    json_output: bool,
    validate_only: bool,
) -> Result<()> {
    // Load pack
    let pack = load_pack(&pack_path)?;

    // Validate
    pack.validate()?;

    if validate_only {
        eprintln!("✓ Query pack is valid");
        eprintln!("  Name: {}", pack.name);
        eprintln!("  Queries: {}", pack.get_queries().len());
        return Ok(());
    }

    // Initialize client
    let client = Client::new()?;

    eprintln!("Authenticating with Azure...");
    client.force_validate_auth().await?;

    eprintln!("Loading workspaces...");
    let all_workspaces = client.list_workspaces().await?;

    // Determine workspace selection
    let selected_workspaces = select_workspaces(
        &all_workspaces,
        workspaces_override,
        pack.workspaces.as_ref(),
    )?;

    if selected_workspaces.is_empty() {
        return Err(crate::error::KqlPanopticonError::QueryPackValidation(
            "No workspaces selected for execution".into(),
        ));
    }

    eprintln!(
        "Executing {} quer{} across {} workspace{}...",
        pack.get_queries().len(),
        if pack.get_queries().len() == 1 {
            "y"
        } else {
            "ies"
        },
        selected_workspaces.len(),
        if selected_workspaces.len() == 1 {
            ""
        } else {
            "s"
        }
    );

    // Get base settings from pack or use defaults
    let base_settings = pack.settings.clone().unwrap_or_default();

    // Execute all queries across all workspaces
    let mut all_results = Vec::new();

    for pack_query in pack.get_queries() {
        eprintln!("\nExecuting: {}", pack_query.name);

        // Create settings for this query
        let mut settings = base_settings.clone();
        settings.job_name = sanitize_name(&pack_query.name);

        // Build and execute job
        let results = QueryJobBuilder::new()
            .workspaces(selected_workspaces.clone())
            .queries(vec![pack_query.query.clone()])
            .settings(settings)
            .execute(&client)
            .await?;

        all_results.extend(results);
    }

    // Create session name from pack
    let session_name = format!(
        "{}-{}",
        sanitize_name(&pack.name),
        chrono::Utc::now().format("%Y-%m-%d_%H%M%S")
    );

    // Output results based on format
    let effective_format = if json_output {
        OutputFormat::Stdout
    } else {
        format
    };

    match effective_format {
        OutputFormat::Files => {
            output_to_files(&all_results, &pack)?;
            print_summary(&all_results);
            eprintln!("\nSession: {}", session_name);
        }
        OutputFormat::Stdout => {
            output_to_stdout(&all_results)?;
        }
    }

    Ok(())
}

fn load_pack(path_str: &str) -> Result<QueryPack> {
    let path = Path::new(path_str);

    // If absolute path, use directly
    if path.is_absolute() {
        return QueryPack::load_from_file(path);
    }

    // Try as relative path first
    if path.exists() {
        return QueryPack::load_from_file(path);
    }

    // Try in library location
    let library_path = QueryPack::get_library_path(path_str)?;
    if library_path.exists() {
        return QueryPack::load_from_file(&library_path);
    }

    Err(crate::error::KqlPanopticonError::QueryPackNotFound(
        path_str.to_string(),
    ))
}

fn select_workspaces(
    all_workspaces: &[Workspace],
    cli_override: Option<String>,
    pack_scope: Option<&WorkspaceScope>,
) -> Result<Vec<Workspace>> {
    // CLI override takes precedence
    if let Some(override_spec) = cli_override {
        return parse_workspace_spec(&override_spec, all_workspaces);
    }

    // Fall back to pack scope
    if let Some(scope) = pack_scope {
        return match scope {
            WorkspaceScope::All => Ok(all_workspaces.to_vec()),
            WorkspaceScope::Selected { ids } => Ok(all_workspaces
                .iter()
                .filter(|ws| ids.contains(&ws.workspace_id) || ids.contains(&ws.resource_id))
                .cloned()
                .collect()),
            WorkspaceScope::Pattern { pattern } => {
                filter_workspaces_by_pattern(all_workspaces, pattern)
            }
        };
    }

    // Default to all workspaces
    Ok(all_workspaces.to_vec())
}

fn parse_workspace_spec(spec: &str, all_workspaces: &[Workspace]) -> Result<Vec<Workspace>> {
    if spec == "all" {
        return Ok(all_workspaces.to_vec());
    }

    // Comma-separated IDs or names
    let ids: Vec<&str> = spec.split(',').map(|s| s.trim()).collect();
    Ok(all_workspaces
        .iter()
        .filter(|ws| {
            ids.iter()
                .any(|id| ws.workspace_id.contains(id) || ws.name.contains(id))
        })
        .cloned()
        .collect())
}

fn filter_workspaces_by_pattern(workspaces: &[Workspace], pattern: &str) -> Result<Vec<Workspace>> {
    // Simple glob-style pattern matching
    let pattern = pattern.replace('*', ".*");
    let regex = regex::Regex::new(&pattern).map_err(|e| {
        crate::error::KqlPanopticonError::QueryPackValidation(format!(
            "Invalid workspace pattern: {}",
            e
        ))
    })?;

    Ok(workspaces
        .iter()
        .filter(|ws| regex.is_match(&ws.name))
        .cloned()
        .collect())
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .to_lowercase()
}

fn output_to_files(results: &[QueryJobResult], _pack: &QueryPack) -> Result<()> {
    // The QueryJobResult already handles file output via its internal logic
    // We just need to report the outcome
    let success = results.iter().filter(|r| r.result.is_ok()).count();

    // Files are already written by QueryJobBuilder
    // Just provide feedback
    if success > 0 {
        eprintln!("\n✓ Results written to output directory");
    }

    Ok(())
}

fn output_to_stdout(results: &[QueryJobResult]) -> Result<()> {
    let output: Vec<_> = results
        .iter()
        .map(|result| {
            serde_json::json!({
                "workspace": result.workspace_name,
                "workspace_id": result.workspace_id,
                "success": result.result.is_ok(),
                "elapsed_ms": result.elapsed.as_millis(),
                "data": result.result.as_ref().ok(),
                "error": result.result.as_ref().err().map(|e| e.to_string()),
            })
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

fn print_summary(results: &[QueryJobResult]) {
    let total = results.len();
    let success = results.iter().filter(|r| r.result.is_ok()).count();
    let failed = total - success;

    eprintln!("\n--- Summary ---");
    eprintln!("Total executions: {}", total);
    eprintln!("Succeeded: {}", success);
    eprintln!("Failed: {}", failed);

    if failed > 0 {
        eprintln!("\nFailed executions:");
        for result in results {
            if let Err(e) = &result.result {
                eprintln!("  - {}: {}", result.workspace_name, e);
            }
        }
    }
}
