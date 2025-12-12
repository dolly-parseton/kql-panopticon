//! Pack authoring commands
//!
//! Commands for interactively building investigation packs:
//! - `input` - Define user input parameters
//! - `query` - Define query steps
//! - `inputs` - List defined inputs
//! - `steps` - List defined steps
//! - `remove` - Remove an input or step
//! - `edit` - Edit an existing input or step
//! - `info` - Show session info

use super::CommandResult;
use crate::context::SharedContext;
use crate::editor::{EditorConfig, EditorResult, KqlEditorMode};
use crate::session::{InputDef, InputType, PackSession, StepDef};
use anyhow::Result;
use kql_panopticon_core::schema::SchemaRegistry;
use kql_panopticon_core::validation::{KqlValidator, Schema};
use std::sync::{Arc, OnceLock};

/// Global KQL validator instance (lazy initialized)
static VALIDATOR: OnceLock<Option<KqlValidator>> = OnceLock::new();

/// Get or initialize the KQL validator
fn get_validator() -> Option<&'static KqlValidator> {
    VALIDATOR
        .get_or_init(|| KqlValidator::new().ok())
        .as_ref()
}

/// Validation context for schema-aware validation
pub struct ValidationContext<'a> {
    pub session: &'a PackSession,
    pub schema: Option<Schema>,
    pub workspace_id: Option<&'a str>,
}

impl<'a> ValidationContext<'a> {
    /// Create validation context from session only (syntax validation)
    pub fn syntax_only(session: &'a PackSession) -> Self {
        Self {
            session,
            schema: None,
            workspace_id: None,
        }
    }

    /// Create validation context with schema from registry
    pub fn with_registry(
        session: &'a PackSession,
        registry: &SchemaRegistry,
        workspace_id: Option<&'a str>,
    ) -> Self {
        let schema = registry.to_validation_schema(workspace_id);
        Self {
            session,
            schema: Some(schema),
            workspace_id,
        }
    }
}

/// Validate KQL syntax, substituting references with example values
///
/// Returns Ok(()) if valid, Err(error_message) if invalid.
/// Uses schema-aware validation if schema is provided.
fn validate_kql_syntax(query: &str, ctx: &ValidationContext) -> Result<(), String> {
    let validator = match get_validator() {
        Some(v) => v,
        None => {
            // Validator not available - skip validation with warning
            // This allows usage without the native library
            return Ok(());
        }
    };

    // Substitute references with example values for validation
    let prepared_query = ctx.session.prepare_query_for_validation(query);

    // Use schema-aware validation if schema available
    let validation_result = match &ctx.schema {
        Some(schema) if validator.supports_schema_validation() => {
            validator.validate_with_schema(&prepared_query, schema)
        }
        _ => validator.validate_syntax(&prepared_query),
    };

    match validation_result {
        Ok(result) if result.is_valid() => Ok(()),
        Ok(result) => {
            // Build error message with diagnostics
            let errors: Vec<_> = result.errors().collect();
            let validation_type = if ctx.schema.is_some() { "schema" } else { "syntax" };
            let mut msg = format!(
                "\x1b[31m✗\x1b[0m Invalid KQL ({} validation, {} error(s)):",
                validation_type,
                errors.len()
            );

            for err in errors {
                msg.push_str(&format!(
                    "\n  Line {}, Col {}: {}",
                    err.line, err.column, err.message
                ));
            }

            // Add hint about substituted values if query had references
            if query != prepared_query {
                msg.push_str("\n\n  Note: References were substituted with example values for validation.");
            }

            // Add hint about schema validation
            if ctx.schema.is_some() {
                msg.push_str("\n  Schema-aware validation enabled for selected workspace.");
            }

            Err(msg)
        }
        Err(e) => {
            // Validation itself failed - don't block, but warn
            log::warn!("KQL validation error: {}", e);
            Ok(())
        }
    }
}

/// Execute the `input` command
pub async fn input(
    name: String,
    value: Option<String>,
    shared_ctx: SharedContext,
) -> Result<CommandResult> {
    // Validate name
    if !is_valid_identifier(&name) {
        return Ok(CommandResult::error(format!(
            "Invalid name: '{}'\nNames must start with a letter and contain only letters, numbers, and underscores",
            name
        )));
    }

    // Check if name conflicts with existing step
    {
        let ctx = shared_ctx.read().await;
        if ctx.pack_session().steps.contains_key(&name) {
            return Ok(CommandResult::error(format!(
                "Name '{}' is already used by a query step",
                name
            )));
        }
    }

    match value {
        Some(val) => {
            // Inline definition: input name = "value"
            let input_type = infer_input_type(&val);
            let input_def = InputDef {
                name: name.clone(),
                input_type: input_type.clone(),
                description: None,
                required: false, // Has a default value, so not required
                default: Some(val.clone()),
            };

            let mut ctx = shared_ctx.write().await;

            // Commit first to create new state (clones current), then modify
            let command = format!("input {} = \"{}\"", name, truncate_value(&val, 30));
            ctx.commit_state(command);
            ctx.pack_session_mut().add_input(input_def);

            Ok(CommandResult::message(format!(
                "\x1b[32m✓\x1b[0m Input defined: {} ({})",
                name, input_type
            )))
        }
        None => {
            // YAML pack editor for inputs is planned for Phase 7
            // For now, require inline value syntax
            Ok(CommandResult::error(format!(
                "Usage: input {} = \"<value>\"\n\n\
                 The YAML input editor is not yet implemented.\n\
                 Use inline syntax: input {} = \"default_value\"",
                name, name
            )))
        }
    }
}

/// Truncate a value for display in trace
fn truncate_value(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Execute the `query` command
pub async fn query(
    name: String,
    kql: Option<String>,
    shared_ctx: SharedContext,
) -> Result<CommandResult> {
    // Validate name
    if !is_valid_identifier(&name) {
        return Ok(CommandResult::error(format!(
            "Invalid name: '{}'\nNames must start with a letter and contain only letters, numbers, and underscores",
            name
        )));
    }

    // Check if name conflicts with existing input
    {
        let ctx = shared_ctx.read().await;
        if ctx.pack_session().inputs.contains_key(&name) {
            return Ok(CommandResult::error(format!(
                "Name '{}' is already used by an input",
                name
            )));
        }
    }

    match kql {
        Some(query_str) => {
            let mut ctx = shared_ctx.write().await;

            // Validate references before adding
            if let Err(errors) = ctx.pack_session().validate_references(&query_str) {
                let hints: Vec<String> = errors
                    .iter()
                    .map(|e| {
                        if e.contains("Unknown input") {
                            format!("{}\n  Hint: Define with 'input <name>'", e)
                        } else if e.contains("Unknown step") {
                            format!("{}\n  Hint: Define with 'query <name>'", e)
                        } else {
                            e.clone()
                        }
                    })
                    .collect();

                return Ok(CommandResult::error(format!(
                    "\x1b[31m✗\x1b[0m Invalid references:\n  {}",
                    hints.join("\n  ")
                )));
            }

            // Build validation context - use schema if available
            let workspace_id = ctx.selected_workspaces().first().map(|ws| ws.workspace_id.as_str());
            let validation_ctx = match ctx.schema_registry() {
                Some(registry) => ValidationContext::with_registry(
                    ctx.pack_session(),
                    registry,
                    workspace_id,
                ),
                None => ValidationContext::syntax_only(ctx.pack_session()),
            };

            // Validate KQL (schema-aware if schema available)
            if let Err(validation_error) = validate_kql_syntax(&query_str, &validation_ctx) {
                return Ok(CommandResult::error(validation_error));
            }

            // Create step (dependencies auto-detected)
            let step = StepDef::new(name.clone(), query_str.clone());
            let deps = step.depends_on.clone();
            let refs = step.references_inputs.clone();

            // Commit first to create new state (clones current), then modify
            let command = format!("query {} = \"{}\"", name, truncate_value(&query_str, 30));
            ctx.commit_state(command);
            ctx.pack_session_mut().add_step(step);

            // Build response with dependency info
            let mut response = format!("\x1b[32m✓\x1b[0m Step defined: {}", name);

            if !refs.is_empty() {
                response.push_str(&format!("\n  refs: [{}]", refs.join(", ")));
            }

            if !deps.is_empty() {
                response.push_str(&format!("\n  depends: [{}]", deps.join(", ")));
            }

            Ok(CommandResult::message(response))
        }
        None => {
            // Open KQL editor
            // Get session and schema context for reference/column completion
            let (session, schema) = {
                let ctx = shared_ctx.read().await;
                let session = Arc::new(ctx.pack_session().clone());
                let workspace_id = ctx.selected_workspaces().first().map(|ws| ws.workspace_id.as_str());
                let schema = ctx.schema_registry().map(|r| r.to_validation_schema(workspace_id));
                (session, schema)
            };

            let config = EditorConfig::kql(&name);
            let mode = match schema {
                Some(s) => KqlEditorMode::with_schema(session, s),
                None => KqlEditorMode::with_session(session),
            };
            let result = mode.run(config);

            match result {
                Ok(EditorResult::Saved(query_str)) => {
                    // Acquire lock and add the step
                    let mut ctx = shared_ctx.write().await;

                    // Validate references before adding
                    if let Err(errors) = ctx.pack_session().validate_references(&query_str) {
                        let hints: Vec<String> = errors
                            .iter()
                            .map(|e| {
                                if e.contains("Unknown input") {
                                    format!("{}\n  Hint: Define with 'input <name>'", e)
                                } else if e.contains("Unknown step") {
                                    format!("{}\n  Hint: Define with 'query <name>'", e)
                                } else {
                                    e.clone()
                                }
                            })
                            .collect();

                        return Ok(CommandResult::error(format!(
                            "\x1b[31m✗\x1b[0m Invalid references:\n  {}",
                            hints.join("\n  ")
                        )));
                    }

                    // Build validation context
                    let workspace_id = ctx.selected_workspaces().first().map(|ws| ws.workspace_id.as_str());
                    let validation_ctx = match ctx.schema_registry() {
                        Some(registry) => ValidationContext::with_registry(
                            ctx.pack_session(),
                            registry,
                            workspace_id,
                        ),
                        None => ValidationContext::syntax_only(ctx.pack_session()),
                    };

                    // Validate KQL
                    if let Err(validation_error) = validate_kql_syntax(&query_str, &validation_ctx) {
                        return Ok(CommandResult::error(validation_error));
                    }

                    // Create step
                    let step = StepDef::new(name.clone(), query_str.clone());
                    let deps = step.depends_on.clone();
                    let refs = step.references_inputs.clone();

                    // Commit state
                    let command = format!("query {} = \"{}\"", name, truncate_value(&query_str, 30));
                    ctx.commit_state(command);
                    ctx.pack_session_mut().add_step(step);

                    // Build response
                    let mut response = format!("\x1b[32m✓\x1b[0m Step defined: {}", name);
                    if !refs.is_empty() {
                        response.push_str(&format!("\n  refs: [{}]", refs.join(", ")));
                    }
                    if !deps.is_empty() {
                        response.push_str(&format!("\n  depends: [{}]", deps.join(", ")));
                    }

                    Ok(CommandResult::message(response))
                }
                Ok(EditorResult::Cancelled) => {
                    Ok(CommandResult::message("Cancelled"))
                }
                Err(e) => Ok(CommandResult::error(format!(
                    "\x1b[31m✗\x1b[0m Editor error: {}",
                    e
                ))),
            }
        }
    }
}

/// Execute the `inputs` command - list all defined inputs
pub async fn list_inputs(ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let session = ctx.pack_session();

    if session.inputs.is_empty() {
        return Ok(CommandResult::message("Inputs (0): (none defined)"));
    }

    let mut output = format!("Inputs ({}):\n", session.inputs.len());

    for (name, input) in &session.inputs {
        let req_str = if input.required {
            "required"
        } else {
            match &input.default {
                Some(v) => &format!("default: \"{}\"", v),
                None => "optional",
            }
        };

        output.push_str(&format!(
            "  {:<20} {:<12} {}\n",
            name,
            input.input_type.to_string(),
            req_str
        ));
    }

    Ok(CommandResult::output(output.trim_end().to_string()))
}

/// Execute the `steps` command - list all defined steps
pub async fn list_steps(show: Option<String>, ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let session = ctx.pack_session();

    // If --show is specified, display the full query for that step
    if let Some(step_name) = show {
        return match session.get_step(&step_name) {
            Some(step) => {
                let mut output = format!("Step: {}\n", step.name);
                output.push_str(&"-".repeat(40));
                output.push('\n');
                output.push_str(&step.query);
                output.push('\n');
                output.push_str(&"-".repeat(40));

                if !step.references_inputs.is_empty() {
                    output.push_str(&format!("\nrefs: [{}]", step.references_inputs.join(", ")));
                }
                if !step.depends_on.is_empty() {
                    output.push_str(&format!("\ndepends: [{}]", step.depends_on.join(", ")));
                }

                Ok(CommandResult::output(output))
            }
            None => Ok(CommandResult::error(format!(
                "Step '{}' not found",
                step_name
            ))),
        };
    }

    // List all steps
    if session.steps.is_empty() {
        return Ok(CommandResult::message("Steps (0): (none defined)"));
    }

    // Get validator for status checks
    let validator = get_validator();

    // Build validation context - use schema if available
    let workspace_id = ctx.selected_workspaces().first().map(|ws| ws.workspace_id.as_str());
    let schema = ctx.schema_registry().map(|r| r.to_validation_schema(workspace_id));
    let has_schema = schema.is_some();

    let mut output = format!("Steps ({}):\n", session.steps.len());

    for (i, (name, step)) in session.steps.iter().enumerate() {
        // Check validation status (schema-aware if available)
        let status_icon = if let Some(v) = validator {
            let prepared = session.prepare_query_for_validation(&step.query);
            let result = match &schema {
                Some(s) if v.supports_schema_validation() => v.validate_with_schema(&prepared, s),
                _ => v.validate_syntax(&prepared),
            };
            match result {
                Ok(r) if r.is_valid() => "\x1b[32m✓\x1b[0m",
                Ok(_) => "\x1b[31m✗\x1b[0m",
                Err(_) => "\x1b[33m?\x1b[0m",
            }
        } else {
            " " // No validator available
        };

        let deps = if step.depends_on.is_empty() && step.references_inputs.is_empty() {
            "(no dependencies)".to_string()
        } else {
            let mut parts = Vec::new();
            if !step.references_inputs.is_empty() {
                parts.push(format!("refs: [{}]", step.references_inputs.join(", ")));
            }
            if !step.depends_on.is_empty() {
                parts.push(format!("depends: [{}]", step.depends_on.join(", ")));
            }
            parts.join(", ")
        };

        output.push_str(&format!("  {} {}. {:<20} → {}\n", status_icon, i + 1, name, deps));
    }

    // Show validation mode
    if validator.is_some() {
        let mode = if has_schema { "schema" } else { "syntax" };
        output.push_str(&format!("\n  (validation: {})", mode));
    }

    Ok(CommandResult::output(output.trim_end().to_string()))
}

/// Execute the `remove` command - remove an input or step
pub async fn remove(name: String, ctx: SharedContext) -> Result<CommandResult> {
    let mut ctx = ctx.write().await;

    // Check if it's an input or step first (before committing)
    let is_input = ctx.pack_session().inputs.contains_key(&name);
    let is_step = ctx.pack_session().steps.contains_key(&name);

    if is_input {
        // Commit first to create new state (clones current), then modify
        ctx.commit_state(format!("remove {}", name));
        ctx.pack_session_mut().remove_input(&name);
        return Ok(CommandResult::message(format!(
            "\x1b[32m✓\x1b[0m Removed input: {}",
            name
        )));
    }

    if is_step {
        // Commit first to create new state (clones current), then modify
        ctx.commit_state(format!("remove {}", name));
        ctx.pack_session_mut().remove_step(&name);
        return Ok(CommandResult::message(format!(
            "\x1b[32m✓\x1b[0m Removed step: {}",
            name
        )));
    }

    Ok(CommandResult::error(format!(
        "No input or step named '{}' found",
        name
    )))
}

/// Execute the `info` command - show session summary
pub async fn info(ctx: SharedContext) -> Result<CommandResult> {
    let ctx = ctx.read().await;
    let session = ctx.pack_session();
    let summary = ctx.status_summary();

    let mut output = String::new();

    // Session name
    let session_name = session.name.as_deref().unwrap_or("(untitled)");
    output.push_str(&format!("Pack: {}\n", session_name));

    // Description if set
    if let Some(desc) = &session.description {
        output.push_str(&format!("Description: {}\n", desc));
    }

    output.push('\n');

    // Counts
    output.push_str(&format!("Inputs: {} defined\n", session.inputs.len()));
    output.push_str(&format!("Steps: {} defined\n", session.steps.len()));

    output.push('\n');

    // Workspace info
    if let Some(ws) = &summary.selected_workspace {
        output.push_str(&format!("Workspace: {}\n", ws));
    } else {
        output.push_str("Workspace: (none selected)\n");
    }

    Ok(CommandResult::output(output.trim_end().to_string()))
}

/// Execute the `new` command - start a new pack session
pub async fn new_session(name: Option<String>, ctx: SharedContext) -> Result<CommandResult> {
    let mut ctx = ctx.write().await;

    // Check if current session has unsaved work
    let session = ctx.pack_session();
    if !session.is_empty() {
        // TODO: Prompt for confirmation
        // For now, just warn
    }

    ctx.new_pack_session(name.clone());

    let msg = match name {
        Some(n) => format!("\x1b[32m✓\x1b[0m New pack session: {}", n),
        None => "\x1b[32m✓\x1b[0m New pack session started".to_string(),
    };

    Ok(CommandResult::message(msg))
}

/// Execute the `edit` command - edit an existing input or step
pub async fn edit(name: String, shared_ctx: SharedContext) -> Result<CommandResult> {
    // Check if name exists and what type it is
    let (is_input, is_step, existing_input, existing_step) = {
        let ctx = shared_ctx.read().await;
        let session = ctx.pack_session();
        (
            session.inputs.contains_key(&name),
            session.steps.contains_key(&name),
            session.inputs.get(&name).cloned(),
            session.steps.get(&name).cloned(),
        )
    };

    if is_input {
        // YAML pack editor for inputs is planned for Phase 7
        // For now, inputs can only be edited by removing and re-adding
        let input_def = existing_input.unwrap();
        Ok(CommandResult::error(format!(
            "Editing inputs via YAML is not yet implemented.\n\n\
             Current value of '{}': \"{}\"\n\n\
             To modify, use:\n\
             1. remove {}\n\
             2. input {} = \"new_value\"",
            name,
            input_def.default.as_deref().unwrap_or("(no default)"),
            name,
            name
        )))
    } else if is_step {
        // Edit step with KQL editor
        let step_def = existing_step.unwrap();

        // Get session and schema context for reference/column completion
        let (session, schema) = {
            let ctx = shared_ctx.read().await;
            let session = Arc::new(ctx.pack_session().clone());
            let workspace_id = ctx.selected_workspaces().first().map(|ws| ws.workspace_id.as_str());
            let schema = ctx.schema_registry().map(|r| r.to_validation_schema(workspace_id));
            (session, schema)
        };

        let config = EditorConfig::kql(&name).with_content(&step_def.query);
        let mode = match schema {
            Some(s) => KqlEditorMode::with_schema(session, s),
            None => KqlEditorMode::with_session(session),
        };
        let result = mode.run(config);

        match result {
            Ok(EditorResult::Saved(query_str)) => {
                let mut ctx = shared_ctx.write().await;

                // Validate references
                if let Err(errors) = ctx.pack_session().validate_references(&query_str) {
                    let hints: Vec<String> = errors
                        .iter()
                        .map(|e| {
                            if e.contains("Unknown input") {
                                format!("{}\n  Hint: Define with 'input <name>'", e)
                            } else if e.contains("Unknown step") {
                                format!("{}\n  Hint: Define with 'query <name>'", e)
                            } else {
                                e.clone()
                            }
                        })
                        .collect();

                    return Ok(CommandResult::error(format!(
                        "\x1b[31m✗\x1b[0m Invalid references:\n  {}",
                        hints.join("\n  ")
                    )));
                }

                // Build validation context
                let workspace_id = ctx.selected_workspaces().first().map(|ws| ws.workspace_id.as_str());
                let validation_ctx = match ctx.schema_registry() {
                    Some(registry) => ValidationContext::with_registry(
                        ctx.pack_session(),
                        registry,
                        workspace_id,
                    ),
                    None => ValidationContext::syntax_only(ctx.pack_session()),
                };

                // Validate KQL
                if let Err(validation_error) = validate_kql_syntax(&query_str, &validation_ctx) {
                    return Ok(CommandResult::error(validation_error));
                }

                // Commit state
                ctx.commit_state(format!("edit {}", name));

                // Update the step
                let step = StepDef::new(name.clone(), query_str);
                let deps = step.depends_on.clone();
                let refs = step.references_inputs.clone();
                ctx.pack_session_mut().add_step(step);

                // Build response
                let mut response = format!("\x1b[32m✓\x1b[0m Step updated: {}", name);
                if !refs.is_empty() {
                    response.push_str(&format!("\n  refs: [{}]", refs.join(", ")));
                }
                if !deps.is_empty() {
                    response.push_str(&format!("\n  depends: [{}]", deps.join(", ")));
                }

                Ok(CommandResult::message(response))
            }
            Ok(EditorResult::Cancelled) => {
                Ok(CommandResult::message("Cancelled"))
            }
            Err(e) => Ok(CommandResult::error(format!(
                "\x1b[31m✗\x1b[0m Editor error: {}",
                e
            ))),
        }
    } else {
        Ok(CommandResult::error(format!(
            "No input or step named '{}' found",
            name
        )))
    }
}

/// Check if a string is a valid identifier
fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut chars = s.chars();

    // First character must be a letter or underscore
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }

    // Remaining characters must be alphanumeric or underscore
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Infer input type from a string value
fn infer_input_type(value: &str) -> InputType {
    // Check for boolean
    if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
        return InputType::Bool;
    }

    // Check for integer
    if value.parse::<i64>().is_ok() {
        return InputType::Int;
    }

    // Check for timespan (ISO 8601 duration)
    if value.starts_with('P') && value.len() > 1 {
        return InputType::Timespan;
    }

    // Check for datetime (ISO 8601)
    if value.contains('T') && value.contains('-') {
        return InputType::Datetime;
    }

    // Default to string
    InputType::String
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_identifier() {
        assert!(is_valid_identifier("foo"));
        assert!(is_valid_identifier("foo_bar"));
        assert!(is_valid_identifier("foo123"));
        assert!(is_valid_identifier("_foo"));

        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("123foo"));
        assert!(!is_valid_identifier("foo-bar"));
        assert!(!is_valid_identifier("foo bar"));
    }

    #[test]
    fn test_infer_input_type() {
        assert_eq!(infer_input_type("hello"), InputType::String);
        assert_eq!(infer_input_type("123"), InputType::Int);
        assert_eq!(infer_input_type("true"), InputType::Bool);
        assert_eq!(infer_input_type("P7D"), InputType::Timespan);
        assert_eq!(
            infer_input_type("2024-01-15T10:30:00Z"),
            InputType::Datetime
        );
    }
}
