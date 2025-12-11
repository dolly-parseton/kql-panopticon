//! KQL step execution handler
//!
//! Executes KQL queries against Azure Log Analytics workspaces.

use crate::client::{Client, QueryResponse};
use crate::error::{Error, Result};
use crate::execution::step::{StepContext, StepHandler, StepOutput};
use crate::pack::{Step, StepType};
use crate::variable::substitute;
use async_trait::async_trait;
use log::debug;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use std::time::Instant;

/// Handler for KQL query steps
pub(crate) struct KqlHandler {
    client: Arc<Client>,
}

impl KqlHandler {
    /// Create a new KQL handler with the given client
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StepHandler for KqlHandler {
    fn step_type(&self) -> StepType {
        StepType::Kql
    }

    async fn execute(&self, step: &Step, ctx: &StepContext<'_>) -> Result<StepOutput> {
        let start = Instant::now();

        // KQL steps require a workspace
        let workspace = ctx.workspace.ok_or_else(|| {
            Error::investigation(
                &step.name,
                "KQL step requires a workspace context",
            )
        })?;

        // Get the query
        let query = step.query.as_ref().ok_or_else(|| {
            Error::investigation(&step.name, "KQL step missing query")
        })?;

        // Substitute variables
        let resolved_query = substitute(query, ctx.substitution).map_err(|e| {
            Error::investigation(
                &step.name,
                format!("Variable substitution failed: {}", e),
            )
        })?;

        debug!(
            "Executing KQL step '{}' on workspace '{}': {}",
            step.name,
            workspace.name,
            truncate_query(&resolved_query, 100)
        );

        // Execute query with timeout
        let query_future = self.client.query_workspace(
            &workspace.workspace_id,
            &resolved_query,
            step.timespan.as_deref(),
        );

        let response = tokio::time::timeout(ctx.timeout, query_future)
            .await
            .map_err(|_| {
                Error::timeout(format!(
                    "KQL query '{}' timed out after {:?}",
                    step.name, ctx.timeout
                ))
            })?
            .map_err(|e| {
                Error::investigation(&step.name, format!("Query execution failed: {}", e))
            })?;

        // Convert response to uniform row format
        let rows = response_to_rows(&response).map_err(|e| {
            Error::investigation(&step.name, format!("Failed to process results: {}", e))
        })?;

        debug!(
            "KQL step '{}' completed: {} rows in {:?}",
            step.name,
            rows.len(),
            start.elapsed()
        );

        Ok(StepOutput::from_rows(rows, start.elapsed()))
    }

    fn validate(&self, step: &Step) -> Result<()> {
        if step.query.as_ref().map_or(true, |q| q.trim().is_empty()) {
            return Err(Error::pack(format!(
                "KQL step '{}' must have a non-empty query",
                step.name
            )));
        }

        if step.request.is_some() {
            return Err(Error::pack(format!(
                "KQL step '{}' should not have 'request' configuration",
                step.name
            )));
        }

        Ok(())
    }
}

/// Convert query response to Vec<JsonValue> rows
///
/// Each row becomes a JSON object with column names as keys.
fn response_to_rows(response: &QueryResponse) -> Result<Vec<JsonValue>> {
    let table = match response.tables.first() {
        Some(t) => t,
        None => return Ok(vec![]), // No tables = empty result
    };

    let columns: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();

    let mut rows = Vec::with_capacity(table.rows.len());
    for row in &table.rows {
        if let Some(values) = row.as_array() {
            let mut obj = serde_json::Map::new();
            for (i, col) in columns.iter().enumerate() {
                if let Some(val) = values.get(i) {
                    obj.insert((*col).to_string(), val.clone());
                }
            }
            rows.push(JsonValue::Object(obj));
        }
    }

    Ok(rows)
}

/// Truncate query for logging
fn truncate_query(query: &str, max_len: usize) -> String {
    let single_line = query.replace('\n', " ").replace("  ", " ");
    if single_line.len() > max_len {
        format!("{}...", &single_line[..max_len])
    } else {
        single_line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Column, Table};

    #[test]
    fn test_response_to_rows() {
        let response = QueryResponse {
            tables: vec![Table {
                name: "PrimaryResult".to_string(),
                columns: vec![
                    Column {
                        name: "Id".to_string(),
                        column_type: "string".to_string(),
                    },
                    Column {
                        name: "Name".to_string(),
                        column_type: "string".to_string(),
                    },
                ],
                rows: vec![
                    serde_json::json!(["1", "Alice"]),
                    serde_json::json!(["2", "Bob"]),
                ],
            }],
            next_link: None,
        };

        let rows = response_to_rows(&response).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["Id"], "1");
        assert_eq!(rows[0]["Name"], "Alice");
        assert_eq!(rows[1]["Id"], "2");
        assert_eq!(rows[1]["Name"], "Bob");
    }

    #[test]
    fn test_response_to_rows_empty() {
        let response = QueryResponse {
            tables: vec![],
            next_link: None,
        };
        let rows = response_to_rows(&response).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_truncate_query() {
        let short = "SELECT * FROM table";
        assert_eq!(truncate_query(short, 100), short);

        let long = "a".repeat(150);
        let truncated = truncate_query(&long, 100);
        assert!(truncated.ends_with("..."));
        assert_eq!(truncated.len(), 103); // 100 + "..."
    }
}
