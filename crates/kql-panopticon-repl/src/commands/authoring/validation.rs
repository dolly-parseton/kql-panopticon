//! KQL validation utilities for pack authoring
//!
//! Provides schema-aware validation for KQL queries with reference substitution.

use crate::session::PackSession;
use kql_panopticon_core::schema::SchemaRegistry;
use kql_panopticon_core::validation::{KqlValidator, Schema};
use std::sync::OnceLock;

/// Global KQL validator instance (lazy initialized)
static VALIDATOR: OnceLock<Option<KqlValidator>> = OnceLock::new();

/// Get or initialize the KQL validator
pub fn get_validator() -> Option<&'static KqlValidator> {
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
pub fn validate_kql_syntax(query: &str, ctx: &ValidationContext) -> Result<(), String> {
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
