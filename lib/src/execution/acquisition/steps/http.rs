//! HTTP step execution handler
//!
//! Executes HTTP requests for external API enrichment.

use crate::error::{Error, Result};
use crate::execution::acquisition::{
    AcquisitionContext, AcquisitionStepHandler, AcquisitionStepOutput,
};
use crate::execution::result::ResultWriter;
use crate::pack::{AcquisitionStepType, HttpMethod, Step};
use crate::variable::{ContextType, EvaluationContext, SubstitutionBuilder};
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

        // Check if URL uses for_each pattern
        let url_builder = SubstitutionBuilder::new(&request.url, ctx.evaluation()).map_err(|e| {
            Error::investigation(&step.name, format!("URL parsing failed: {}", e))
        })?;

        // Validate context
        url_builder.validate(ContextType::HttpRequest).map_err(|e| {
            Error::investigation(&step.name, format!("URL validation failed: {}", e))
        })?;

        // Prepare writer for aggregated results
        let mut writer = ctx.writer(&step.name)?;

        if url_builder.has_for_each() {
            // Iterate over for_each values
            let iter = url_builder.substitute_iter().map_err(|e| {
                Error::investigation(&step.name, format!("URL iteration failed: {}", e))
            })?;
            let urls: Result<Vec<String>> = iter.collect();
            let urls = urls.map_err(|e| {
                Error::investigation(&step.name, format!("URL substitution failed: {}", e))
            })?;

            debug!(
                "Executing HTTP step '{}' with {} iterations: {:?}",
                step.name, urls.len(), request.method
            );

            for url in urls {
                self.execute_single_request(
                    step,
                    request,
                    response_config,
                    &url,
                    ctx,
                    &mut writer,
                ).await?;
            }
        } else {
            // Single request
            let url = url_builder.substitute().map_err(|e| {
                Error::investigation(&step.name, format!("URL substitution failed: {}", e))
            })?;

            debug!(
                "Executing HTTP step '{}': {:?} {}",
                step.name, request.method, url
            );

            self.execute_single_request(
                step,
                request,
                response_config,
                &url,
                ctx,
                &mut writer,
            ).await?;
        }

        let handle = writer.finish()?;
        let row_count = handle.row_count()?;

        debug!(
            "HTTP step '{}' completed: {} rows in {:?}",
            step.name,
            row_count,
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

impl HttpStepHandler {
    /// Execute a single HTTP request and write results to writer
    async fn execute_single_request(
        &self,
        step: &Step,
        request: &crate::pack::HttpRequest,
        response_config: &crate::pack::HttpResponse,
        url: &str,
        ctx: &AcquisitionContext<'_>,
        writer: &mut ResultWriter,
    ) -> Result<()> {
        // Build request
        let mut req = match request.method {
            HttpMethod::Get => self.client.get(url),
            HttpMethod::Post => self.client.post(url),
            HttpMethod::Put => self.client.put(url),
            HttpMethod::Delete => self.client.delete(url),
        };

        // Add query parameters with substitution
        for (key, value) in &request.params {
            let builder = SubstitutionBuilder::new(value, ctx.evaluation()).map_err(|e| {
                Error::investigation(
                    &step.name,
                    format!("Query param '{}' parsing failed: {}", key, e),
                )
            })?;
            let resolved = builder.substitute().map_err(|e| {
                Error::investigation(
                    &step.name,
                    format!("Query param '{}' substitution failed: {}", key, e),
                )
            })?;
            req = req.query(&[(key, resolved)]);
        }

        // Add headers with substitution
        for (key, value) in &request.headers {
            let builder = SubstitutionBuilder::new(value, ctx.evaluation()).map_err(|e| {
                Error::investigation(
                    &step.name,
                    format!("Header '{}' parsing failed: {}", key, e),
                )
            })?;
            let resolved = builder.substitute().map_err(|e| {
                Error::investigation(
                    &step.name,
                    format!("Header '{}' substitution failed: {}", key, e),
                )
            })?;
            req = req.header(key, resolved);
        }

        // Add body if present (with variable substitution)
        if let Some(body) = &request.body {
            let resolved_body = substitute_json_value(body, ctx.evaluation(), &step.name)?;
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

        // Write to aggregated output
        writer.write_row(&row)?;

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
    context: &EvaluationContext,
    step_name: &str,
) -> Result<JsonValue> {
    match value {
        JsonValue::String(s) => {
            let builder = SubstitutionBuilder::new(s, context).map_err(|e| {
                Error::investigation(step_name, format!("Body parsing failed: {}", e))
            })?;
            let resolved = builder.substitute().map_err(|e| {
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
                let key_builder = SubstitutionBuilder::new(key, context).map_err(|e| {
                    Error::investigation(
                        step_name,
                        format!("Body key '{}' parsing failed: {}", key, e),
                    )
                })?;
                let resolved_key = key_builder.substitute().map_err(|e| {
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
        let context = EvaluationContext::new()
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
