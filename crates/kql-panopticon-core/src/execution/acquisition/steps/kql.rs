//! KQL step execution handler
//!
//! Executes KQL queries against Azure Log Analytics workspaces.

use crate::client::{Client, QueryResponse};
use crate::error::{Error, Result};
use crate::execution::acquisition::{
    AcquisitionContext, AcquisitionStepHandler, AcquisitionStepOutput,
};
use crate::execution::result::ResultWriter;
use crate::pack::{AcquisitionStepType, Step};
use crate::variable::substitute;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use std::time::Instant;
use tracing::debug;

/// Handler for KQL query steps
pub struct KqlStepHandler {
    client: Arc<Client>,
}

impl KqlStepHandler {
    /// Create a new KQL handler with the given client
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl AcquisitionStepHandler for KqlStepHandler {
    fn handles(&self) -> AcquisitionStepType {
        AcquisitionStepType::Kql
    }

    async fn execute(
        &self,
        step: &Step,
        ctx: &mut AcquisitionContext<'_>,
    ) -> Result<AcquisitionStepOutput> {
        let start = Instant::now();

        // Get the query
        let query = step.query.as_ref().ok_or_else(|| {
            Error::investigation(&step.name, "KQL step missing query")
        })?;

        // Substitute variables
        let resolved_query = substitute(query, ctx.substitution()).map_err(|e| {
            Error::investigation(&step.name, format!("Variable substitution failed: {}", e))
        })?;

        debug!(
            "Executing KQL step '{}' on workspace '{}': {}",
            step.name,
            ctx.workspace.name,
            truncate_query(&resolved_query, 100)
        );

        // Execute query with timeout
        let query_future = self.client.query_workspace(
            &ctx.workspace.workspace_id,
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

        // Write results to JSONL file
        let mut writer = ctx.writer(&step.name)?;
        write_response_to_writer(&response, &mut writer)?;

        let handle = writer.finish()?;
        let row_count = handle.row_count()?;

        debug!(
            "KQL step '{}' completed: {} rows in {:?}",
            step.name,
            row_count,
            start.elapsed()
        );

        Ok(AcquisitionStepOutput::new(handle, start.elapsed()))
    }

    fn validate(&self, step: &Step) -> Result<()> {
        if step.query.as_ref().is_none_or(|q| q.trim().is_empty()) {
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

/// Write query response rows directly to JSONL file
fn write_response_to_writer(response: &QueryResponse, writer: &mut ResultWriter) -> Result<()> {
    let table = match response.tables.first() {
        Some(t) => t,
        None => return Ok(()), // No tables = empty result
    };

    let columns: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();

    for row in &table.rows {
        if let Some(values) = row.as_array() {
            let mut obj = serde_json::Map::new();
            for (i, col) in columns.iter().enumerate() {
                if let Some(val) = values.get(i) {
                    obj.insert((*col).to_string(), val.clone());
                }
            }
            writer.write_row(&JsonValue::Object(obj))?;
        }
    }

    Ok(())
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

    fn response_to_rows(response: &QueryResponse) -> Result<Vec<JsonValue>> {
        let table = match response.tables.first() {
            Some(t) => t,
            None => return Ok(vec![]),
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
    }

    #[test]
    fn test_truncate_query() {
        let short = "SELECT * FROM table";
        assert_eq!(truncate_query(short, 100), short);

        let long = "a".repeat(150);
        let truncated = truncate_query(&long, 100);
        assert!(truncated.ends_with("..."));
    }
}
