//! Pack management commands

use crate::context::SharedContext;
use super::CommandResult;
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum PackAction {
    /// Load a pack from file
    Load {
        /// Path to pack file (YAML or JSON)
        path: PathBuf,
    },

    /// Show loaded pack information
    Info,

    /// List steps in loaded pack
    Steps,

    /// Validate pack queries (requires workspace for schema)
    Validate,

    /// Unload current pack
    Unload,
}

pub async fn execute(action: PackAction, ctx: SharedContext) -> Result<CommandResult> {
    match action {
        PackAction::Load { path } => load(path, ctx).await,
        PackAction::Info => info(ctx).await,
        PackAction::Steps => steps(ctx).await,
        PackAction::Validate => validate(ctx).await,
        PackAction::Unload => unload(ctx).await,
    }
}

async fn load(path: PathBuf, ctx: SharedContext) -> Result<CommandResult> {
    let mut ctx = ctx.write().await;

    // Resolve path
    let resolved_path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };

    if !resolved_path.exists() {
        return Ok(CommandResult::error(format!(
            "File not found: {}",
            resolved_path.display()
        )));
    }

    ctx.load_pack(resolved_path.clone())?;

    let pack = ctx.loaded_pack().unwrap();
    let step_count = pack.pack.steps.len();
    let input_count = pack.pack.inputs.len();

    let mut output = String::new();
    output.push_str(&format!("Loaded: {} ({} steps", pack.pack.name, step_count));
    if input_count > 0 {
        output.push_str(&format!(", {} inputs", input_count));
    }
    output.push_str(")\n");

    if let Some(desc) = &pack.pack.description {
        output.push_str(&format!("  {}\n", desc));
    }

    // Show required inputs
    let required_inputs = pack.pack.required_inputs();
    if !required_inputs.is_empty() {
        output.push_str("\nRequired inputs:\n");
        for input in required_inputs {
            output.push_str(&format!(
                "  - {} ({})\n",
                input.name,
                input.description.as_deref().unwrap_or("no description")
            ));
        }
    }

    Ok(CommandResult::output(output))
}

async fn info(ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;

    let loaded = match ctx.loaded_pack() {
        Some(p) => p,
        None => return Ok(CommandResult::error("No pack loaded. Use 'pack load <path>'")),
    };

    let pack = &loaded.pack;
    let mut output = String::new();

    output.push_str(&format!("Pack: {}\n", pack.name));
    output.push_str(&format!("Path: {}\n", loaded.path.display()));

    if let Some(desc) = &pack.description {
        output.push_str(&format!("Description: {}\n", desc));
    }

    if let Some(version) = &pack.version {
        output.push_str(&format!("Version: {}\n", version));
    }

    output.push_str(&format!("\nSteps: {}\n", pack.steps.len()));

    // Show dependency structure
    let has_deps = pack.has_dependencies();
    if has_deps {
        output.push_str("  Execution: Sequential (has dependencies)\n");
    } else {
        output.push_str("  Execution: Parallel (no dependencies)\n");
    }

    // Show inputs
    if !pack.inputs.is_empty() {
        output.push_str(&format!("\nInputs: {}\n", pack.inputs.len()));
        for input in &pack.inputs {
            let required = if input.required { " (required)" } else { "" };
            let default = input
                .default
                .as_ref()
                .map(|d| format!(" [default: {}]", d))
                .unwrap_or_default();
            output.push_str(&format!("  - {}{}{}\n", input.name, required, default));
        }
    }

    // Show report config
    if pack.report.is_some() {
        output.push_str("\nReport: Configured\n");
    }

    if pack.scoring.is_some() {
        output.push_str("Scoring: Configured\n");
    }

    Ok(CommandResult::output(output))
}

async fn steps(ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;

    let loaded = match ctx.loaded_pack() {
        Some(p) => p,
        None => return Ok(CommandResult::error("No pack loaded. Use 'pack load <path>'")),
    };

    let pack = &loaded.pack;
    let mut output = String::new();

    output.push_str(&format!("Steps in '{}':\n\n", pack.name));

    for (i, step) in pack.steps.iter().enumerate() {
        let step_type = match step.step_type {
            kql_panopticon_core::StepType::Kql => "KQL",
            kql_panopticon_core::StepType::Http => "HTTP",
            kql_panopticon_core::StepType::File => "File",
        };

        output.push_str(&format!("{}. {} [{}]\n", i + 1, step.name, step_type));

        // Show dependencies
        if !step.depends_on.is_empty() {
            output.push_str(&format!("   depends_on: {}\n", step.depends_on.join(", ")));
        }

        // Show foreach
        if let Some(foreach) = &step.foreach {
            output.push_str(&format!("   foreach: {}\n", foreach));
        }

        // Show when condition
        if let Some(when) = &step.when {
            output.push_str(&format!("   when: {}\n", when));
        }

        // Show query preview for KQL steps
        if let Some(query) = &step.query {
            let preview: String = query
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(60)
                .collect();
            output.push_str(&format!("   query: {}...\n", preview));
        }

        output.push('\n');
    }

    Ok(CommandResult::output(output))
}

async fn validate(ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;

    let loaded = match ctx.loaded_pack() {
        Some(p) => p,
        None => return Ok(CommandResult::error("No pack loaded. Use 'pack load <path>'")),
    };

    let pack = &loaded.pack;
    let mut output = String::new();
    let mut errors = Vec::new();

    output.push_str(&format!("Validating pack '{}'...\n\n", pack.name));

    // Try to create a validator
    let validator = match kql_panopticon_core::validation::KqlValidator::new() {
        Ok(v) => Some(v),
        Err(e) => {
            output.push_str(&format!("Warning: Validator unavailable: {}\n\n", e));
            None
        }
    };

    for step in &pack.steps {
        if let Some(query) = &step.query {
            if let Some(ref v) = validator {
                match v.validate_syntax(query) {
                    Ok(result) if result.is_valid() => {
                        output.push_str(&format!("  ✓ {} - valid\n", step.name));
                    }
                    Ok(result) => {
                        let errs: Vec<_> = result.errors().collect();
                        output.push_str(&format!("  ✗ {} - {} error(s)\n", step.name, errs.len()));
                        for err in errs {
                            errors.push(format!("    {}: {} (line {}, col {})",
                                step.name, err.message, err.line, err.column));
                        }
                    }
                    Err(e) => {
                        output.push_str(&format!("  ? {} - error: {}\n", step.name, e));
                    }
                }
            } else {
                output.push_str(&format!("  ? {} - validator unavailable\n", step.name));
            }
        }
    }

    if !errors.is_empty() {
        output.push_str("\nErrors:\n");
        for err in errors {
            output.push_str(&format!("{}\n", err));
        }
    }

    Ok(CommandResult::output(output))
}

async fn unload(ctx: SharedContext) -> Result<CommandResult> {
    let mut ctx = ctx.write().await;

    if ctx.loaded_pack().is_none() {
        return Ok(CommandResult::message("No pack loaded"));
    }

    ctx.unload_pack();
    Ok(CommandResult::message("Pack unloaded"))
}
