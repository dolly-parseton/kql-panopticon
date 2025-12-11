//! Jobs management commands

use crate::context::SharedContext;
use super::CommandResult;
use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum JobsAction {
    /// List all jobs
    #[command(alias = "ls")]
    List,

    /// View job details
    View {
        /// Job ID or short prefix
        job_id: String,
    },

    /// Cancel a running job
    Cancel {
        /// Job ID or short prefix
        job_id: String,
    },
}

pub async fn execute(action: Option<JobsAction>, ctx: SharedContext) -> Result<CommandResult> {
    match action {
        None | Some(JobsAction::List) => list(ctx).await,
        Some(JobsAction::View { job_id }) => view(job_id, ctx).await,
        Some(JobsAction::Cancel { job_id }) => cancel(job_id, ctx).await,
    }
}

async fn list(ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let registry = ctx.job_registry();
    let jobs = registry.list();

    if jobs.is_empty() {
        return Ok(CommandResult::message("No jobs"));
    }

    let mut output = String::new();
    output.push_str(&format!("Jobs ({}):\n\n", jobs.len()));

    for job in jobs {
        let status_icon = match job.status {
            kql_panopticon_core::JobStatus::Pending => "○",
            kql_panopticon_core::JobStatus::Running => "⟳",
            kql_panopticon_core::JobStatus::Completed => "✓",
            kql_panopticon_core::JobStatus::Failed => "✗",
            kql_panopticon_core::JobStatus::Cancelled => "⊘",
        };

        let short_id: String = job.id.to_string().chars().take(8).collect();
        output.push_str(&format!(
            "  {} {} {} ({})\n",
            short_id,
            status_icon,
            job.name,
            job.status
        ));

        if let Some(progress) = job.progress_percent {
            output.push_str(&format!("      Progress: {}%\n", progress));
        }

        if let Some(msg) = &job.progress_message {
            output.push_str(&format!("      Status: {}\n", msg));
        }
    }

    Ok(CommandResult::output(output))
}

async fn view(job_id: String, ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let registry = ctx.job_registry();

    // Try to find job by ID prefix
    let jobs = registry.list();
    let job = jobs.iter().find(|j| {
        j.id.to_string().starts_with(&job_id)
    });

    match job {
        Some(job) => {
            let mut output = String::new();
            output.push_str(&format!("Job: {}\n", job.id));
            output.push_str(&format!("Name: {}\n", job.name));
            output.push_str(&format!("Status: {}\n", job.status));

            if let Some(progress) = job.progress_percent {
                output.push_str(&format!("Progress: {}%\n", progress));
            }

            if let Some(msg) = &job.progress_message {
                output.push_str(&format!("Status: {}\n", msg));
            }

            if let Some(error) = &job.error {
                output.push_str(&format!("Error: {}\n", error));
            }

            Ok(CommandResult::output(output))
        }
        None => Ok(CommandResult::error(format!("Job not found: {}", job_id))),
    }
}

async fn cancel(job_id: String, ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let registry = ctx.job_registry();

    // Try to find and cancel job
    let jobs = registry.list();
    let job = jobs.iter().find(|j| {
        j.id.to_string().starts_with(&job_id)
    });

    match job {
        Some(job) => {
            // TODO: Implement actual cancellation via JobRegistry
            Ok(CommandResult::message(format!(
                "Job cancellation not yet implemented: {}",
                job.id
            )))
        }
        None => Ok(CommandResult::error(format!("Job not found: {}", job_id))),
    }
}

/// View results from an execution
pub async fn results(job_id: Option<String>, ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let history = ctx.history();

    let record = match &job_id {
        Some(id) => history.get_by_prefix(id),
        None => history.last(),
    };

    match record {
        Some(record) => {
            let mut output = String::new();
            output.push_str(&format!("Execution: {}\n", record.id));
            output.push_str(&format!("Source: {}\n", record.source_name()));
            output.push_str(&format!("Time: {}\n", record.timestamp.format("%Y-%m-%d %H:%M:%S")));
            output.push_str(&format!("Status: {}\n", record.result.status));
            output.push_str(&format!("Duration: {}ms\n", record.result.duration_ms));
            output.push_str(&format!("Total rows: {}\n", record.result.total_rows));

            if record.result.steps_completed > 0 || record.result.steps_failed > 0 {
                output.push_str(&format!(
                    "Steps: {} completed, {} failed\n",
                    record.result.steps_completed,
                    record.result.steps_failed
                ));
            }

            output.push_str(&format!(
                "Workspaces: {}\n",
                record.workspaces.join(", ")
            ));

            if let Some(path) = &record.output_path {
                output.push_str(&format!("Output: {}\n", path.display()));
            }

            Ok(CommandResult::output(output))
        }
        None => {
            if job_id.is_some() {
                Ok(CommandResult::error("Execution not found"))
            } else {
                Ok(CommandResult::message("No executions in history"))
            }
        }
    }
}
