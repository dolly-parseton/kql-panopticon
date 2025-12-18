//! HTTP step execution handler
//!
//! Executes HTTP requests for external API enrichment.

use crate::error::{Error, Result};
use crate::execution::acquisition::{
    AcquisitionContext, AcquisitionStepHandler, AcquisitionStepOutput,
};
use crate::pack::{AcquisitionStepType, HttpMethod, Step};
use crate::variable::{substitute, SubstitutionContext};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::time::Instant;
use tracing::debug;

/// Handler for HTTP request steps
pub struct HttpStepHandler {
    client: reqwest::Client,
}

impl HttpStepHandler {
    /// Create a new HTTP handler
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    /// Create with a custom client
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for HttpStepHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AcquisitionStepHandler for HttpStepHandler {
    fn handles(&self) -> AcquisitionStepType {
        AcquisitionStepType::Http
    }

    async fn execute(
        &self,
        step: &Step,
        ctx: &mut AcquisitionContext<'_>,
    ) -> Result<AcquisitionStepOutput> {
        let start = Instant::now();

        // Get request config
        let request = step.request.as_ref().ok_or_else(|| {
            Error::investigation(&step.name, "HTTP step missing request configuration")
        })?;

        // Get response config
        let response_config = step.response.as_ref().ok_or_else(|| {
            Error::investigation(&step.name, "HTTP step missing response configuration")
        })?;

        // Substitute variables in URL
        let url = substitute(&request.url, ctx.substitution()).map_err(|e| {
            Error::investigation(&step.name, format!("URL substitution failed: {}", e))
        })?;

        debug!(
            "Executing HTTP step '{}': {:?} {}",
            step.name, request.method, url
        );

        // Build request
        let mut req = match request.method {
            HttpMethod::Get => self.client.get(&url),
            HttpMethod::Post => self.client.post(&url),
            HttpMethod::Put => self.client.put(&url),
            HttpMethod::Delete => self.client.delete(&url),
        };

        // Add query parameters with substitution
        for (key, value) in &request.params {
            let resolved = substitute(value, ctx.substitution()).map_err(|e| {
                Error::investigation(
                    &step.name,
                    format!("Query param '{}' substitution failed: {}", key, e),
                )
            })?;
            req = req.query(&[(key, resolved)]);
        }

        // Add headers with substitution
        for (key, value) in &request.headers {
            let resolved = substitute(value, ctx.substitution()).map_err(|e| {
                Error::investigation(
                    &step.name,
                    format!("Header '{}' substitution failed: {}", key, e),
                )
            })?;
            req = req.header(key, resolved);
        }

        // Add body if present (with variable substitution)
        if let Some(body) = &request.body {
            let resolved_body = substitute_json_value(body, ctx.substitution(), &step.name)?;
            req = req.json(&resolved_body);
        }

        // Execute request with timeout
        let response = tokio::time::timeout(ctx.timeout, req.send())
            .await
            .map_err(|_| {
                Error::timeout(format!(
                    "HTTP request '{}' timed out after {:?}",
                    step.name, ctx.timeout
                ))
            })?
            .map_err(|e| Error::http(format!("HTTP request '{}' failed: {}", step.name, e)))?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::http(format!(
                "HTTP request '{}' returned {}: {}",
                step.name,
                status,
                truncate_body(&body, 200)
            )));
        }

        // Parse response as JSON
        let json: JsonValue = response.json().await.map_err(|e| {
            Error::http(format!(
                "HTTP request '{}' response is not valid JSON: {}",
                step.name, e
            ))
        })?;

        // Extract fields using JSONPath
        let row = extract_fields(&json, &response_config.fields).map_err(|e| {
            Error::investigation(&step.name, format!("Field extraction failed: {}", e))
        })?;

        // Write to JSONL file
        let mut writer = ctx.writer(&step.name)?;
        writer.write_row(&row)?;
        let handle = writer.finish()?;

        debug!(
            "HTTP step '{}' completed in {:?}",
            step.name,
            start.elapsed()
        );

        Ok(AcquisitionStepOutput::new(handle, start.elapsed()))
    }

    fn validate(&self, step: &Step) -> Result<()> {
        if step.request.is_none() {
            return Err(Error::pack(format!(
                "HTTP step '{}' must have 'request' configuration",
                step.name
            )));
        }

        if step.response.is_none() {
            return Err(Error::pack(format!(
                "HTTP step '{}' must have 'response' configuration",
                step.name
            )));
        }

        if step.query.as_ref().is_some_and(|q| !q.trim().is_empty()) {
            return Err(Error::pack(format!(
                "HTTP step '{}' should not have a 'query' field",
                step.name
            )));
        }

        // Validate request config
        if let Some(request) = &step.request {
            if request.url.trim().is_empty() {
                return Err(Error::pack(format!(
                    "HTTP step '{}' has empty URL",
                    step.name
                )));
            }
        }

        Ok(())
    }
}

/// Extract fields from JSON response using JSONPath expressions
fn extract_fields(json: &JsonValue, fields: &HashMap<String, String>) -> Result<JsonValue> {
    let mut obj = serde_json::Map::new();

    for (name, path) in fields {
        let values = jsonpath_lib::select(json, path).map_err(|e| {
            Error::http(format!(
                "JSONPath '{}' for field '{}' failed: {}",
                path, name, e
            ))
        })?;

        // Take first match or null
        let extracted = values
            .first()
            .map(|v| (*v).clone())
            .unwrap_or(JsonValue::Null);

        obj.insert(name.clone(), extracted);
    }

    Ok(JsonValue::Object(obj))
}

/// Truncate body for error messages
fn truncate_body(body: &str, max_len: usize) -> String {
    if body.len() > max_len {
        format!("{}...", &body[..max_len])
    } else {
        body.to_string()
    }
}

/// Recursively substitute variables in a JSON value
fn substitute_json_value(
    value: &JsonValue,
    context: &SubstitutionContext,
    step_name: &str,
) -> Result<JsonValue> {
    match value {
        JsonValue::String(s) => {
            let resolved = substitute(s, context).map_err(|e| {
                Error::investigation(step_name, format!("Body substitution failed: {}", e))
            })?;
            Ok(JsonValue::String(resolved))
        }
        JsonValue::Array(arr) => {
            let resolved: Result<Vec<JsonValue>> = arr
                .iter()
                .map(|v| substitute_json_value(v, context, step_name))
                .collect();
            Ok(JsonValue::Array(resolved?))
        }
        JsonValue::Object(obj) => {
            let mut resolved_obj = serde_json::Map::new();
            for (key, val) in obj {
                let resolved_key = substitute(key, context).map_err(|e| {
                    Error::investigation(
                        step_name,
                        format!("Body key '{}' substitution failed: {}", key, e),
                    )
                })?;
                let resolved_val = substitute_json_value(val, context, step_name)?;
                resolved_obj.insert(resolved_key, resolved_val);
            }
            Ok(JsonValue::Object(resolved_obj))
        }
        _ => Ok(value.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::variable::SubstitutionContext;

    #[test]
    fn test_extract_fields_simple() {
        let json = serde_json::json!({
            "data": {
                "score": 85,
                "category": "safe"
            }
        });

        let mut fields = HashMap::new();
        fields.insert("score".to_string(), "$.data.score".to_string());
        fields.insert("category".to_string(), "$.data.category".to_string());

        let result = extract_fields(&json, &fields).unwrap();
        assert_eq!(result["score"], 85);
        assert_eq!(result["category"], "safe");
    }

    #[test]
    fn test_extract_fields_missing() {
        let json = serde_json::json!({"data": {}});

        let mut fields = HashMap::new();
        fields.insert("missing".to_string(), "$.data.nonexistent".to_string());

        let result = extract_fields(&json, &fields).unwrap();
        assert_eq!(result["missing"], JsonValue::Null);
    }

    #[test]
    fn test_substitute_json_value() {
        let context = SubstitutionContext::new()
            .with_input("user", "alice")
            .with_input("count", "42");

        let value = serde_json::json!("Hello {{inputs.user}}!");
        let result = substitute_json_value(&value, &context, "test").unwrap();
        assert_eq!(result, "Hello alice!");

        let value = serde_json::json!({
            "query": "user:{{inputs.user}}",
            "limit": 100
        });
        let result = substitute_json_value(&value, &context, "test").unwrap();
        assert_eq!(result["query"], "user:alice");
        assert_eq!(result["limit"], 100);
    }
}
