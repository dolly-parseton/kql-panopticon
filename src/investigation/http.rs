//! HTTP step executor for investigation packs
//!
//! This module handles execution of HTTP steps, including:
//! - Variable substitution in URLs, headers, params, and body
//! - Secrets resolution from environment variables
//! - Rate limiting
//! - Response extraction via JSONPath
//! - Error handling strategies

use crate::client::Client;
use crate::error::{KqlPanopticonError, Result};
use crate::investigation_pack::{
    AuthMethod, HttpMethod, HttpResponse, OnError, RateLimitConfig, RateLimitPeriod,
    SecretsConfig, Step,
};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Resolved secrets from environment variables
pub struct ResolvedSecrets {
    secrets: HashMap<String, String>,
}

impl ResolvedSecrets {
    /// Resolve secrets from configuration by reading environment variables
    pub fn resolve(config: Option<&SecretsConfig>) -> Result<Self> {
        let mut secrets = HashMap::new();

        if let Some(config) = config {
            for (name, template) in &config.secrets {
                let value = Self::resolve_template(template)?;
                secrets.insert(name.clone(), value);
            }
        }

        Ok(Self { secrets })
    }

    /// Resolve a single template (e.g., "${ENV_VAR_NAME}")
    fn resolve_template(template: &str) -> Result<String> {
        // Check for ${VAR_NAME} syntax
        if template.starts_with("${") && template.ends_with('}') {
            let var_name = &template[2..template.len() - 1];
            std::env::var(var_name).map_err(|_| {
                KqlPanopticonError::SecretResolutionFailed(format!(
                    "Environment variable '{}' not found",
                    var_name
                ))
            })
        } else {
            // Treat as literal value
            Ok(template.to_string())
        }
    }

    /// Substitute {{secrets.name}} references in a string
    pub fn substitute(&self, input: &str) -> Result<String> {
        let pattern = regex::Regex::new(r"\{\{secrets\.([^}]+)\}\}").unwrap();
        let mut result = input.to_string();

        for cap in pattern.captures_iter(input) {
            let full_match = cap.get(0).unwrap().as_str();
            let secret_name = cap.get(1).unwrap().as_str();

            let value = self.secrets.get(secret_name).ok_or_else(|| {
                KqlPanopticonError::SecretResolutionFailed(format!(
                    "Secret '{}' not found",
                    secret_name
                ))
            })?;

            result = result.replace(full_match, value);
        }

        Ok(result)
    }
}

/// Rate limiter for HTTP requests
pub struct RateLimiter {
    config: Option<RateLimitConfig>,
    request_times: Vec<Instant>,
}

impl RateLimiter {
    pub fn new(config: Option<RateLimitConfig>) -> Self {
        Self {
            config,
            request_times: Vec::new(),
        }
    }

    /// Wait if necessary to respect rate limits
    pub async fn wait_if_needed(&mut self) {
        let config = match &self.config {
            Some(c) => c,
            None => return,
        };

        let window = match config.per {
            RateLimitPeriod::Second => Duration::from_secs(1),
            RateLimitPeriod::Minute => Duration::from_secs(60),
            RateLimitPeriod::Hour => Duration::from_secs(3600),
        };

        // Remove old request times outside the window
        let now = Instant::now();
        self.request_times
            .retain(|t| now.duration_since(*t) < window);

        // If we've hit the limit, wait until the oldest request falls outside the window
        if self.request_times.len() >= config.requests as usize {
            if let Some(oldest) = self.request_times.first() {
                let wait_time = window.saturating_sub(now.duration_since(*oldest));
                if !wait_time.is_zero() {
                    debug!("Rate limiting: waiting {:?}", wait_time);
                    tokio::time::sleep(wait_time).await;
                }
            }
        }

        // Record this request
        self.request_times.push(Instant::now());
    }
}

/// HTTP step executor
pub struct HttpExecutor {
    http_client: reqwest::Client,
    azure_client: Option<Client>,
    secrets: ResolvedSecrets,
    retry_count: u32,
}

impl HttpExecutor {
    /// Create a new HTTP executor
    pub fn new(
        azure_client: Option<Client>,
        secrets: ResolvedSecrets,
        timeout: Duration,
        retry_count: u32,
    ) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| KqlPanopticonError::HttpRequestFailed(e.to_string()))?;

        Ok(Self {
            http_client,
            azure_client,
            secrets,
            retry_count,
        })
    }

    /// Execute an HTTP step and return extracted fields as rows
    pub async fn execute(
        &self,
        step: &Step,
        substitutions: &HashMap<String, String>,
        rate_limiter: &mut RateLimiter,
    ) -> Result<HttpStepResult> {
        let request = step.request.as_ref().ok_or_else(|| {
            KqlPanopticonError::HttpStepError("Step missing request configuration".into())
        })?;

        let response_config = step.response.as_ref().ok_or_else(|| {
            KqlPanopticonError::HttpStepError("Step missing response configuration".into())
        })?;

        // Build the request with substitutions
        let url = self.substitute_all(&request.url, substitutions)?;
        let headers = self.substitute_headers(&request.headers, substitutions)?;
        let params = self.substitute_params(&request.params, substitutions)?;
        let body = self.substitute_body(&request.body, substitutions)?;

        // Wait for rate limiter
        rate_limiter.wait_if_needed().await;

        // Execute with retry
        let response = self
            .execute_with_retry(&url, &request.method, &headers, &params, body.as_ref(), &request.auth)
            .await?;

        // Extract fields from response
        let rows = self.extract_response(&response, response_config)?;

        Ok(HttpStepResult {
            status_code: response.status_code,
            rows,
            raw_response: response.body,
        })
    }

    /// Execute a single HTTP request with retry logic
    async fn execute_with_retry(
        &self,
        url: &str,
        method: &HttpMethod,
        headers: &HashMap<String, String>,
        params: &HashMap<String, String>,
        body: Option<&serde_json::Value>,
        auth: &Option<AuthMethod>,
    ) -> Result<RawHttpResponse> {
        let max_attempts = self.retry_count + 1;
        let mut last_error = None;

        for attempt in 0..max_attempts {
            if attempt > 0 {
                let backoff = match &last_error {
                    Some(KqlPanopticonError::HttpRateLimited { retry_after }) => {
                        info!("Rate limited. Waiting {} seconds", retry_after);
                        Duration::from_secs(*retry_after)
                    }
                    _ => Duration::from_secs(2u64.pow(attempt - 1)),
                };
                tokio::time::sleep(backoff).await;
            }

            match self
                .execute_single(url, method, headers, params, body, auth)
                .await
            {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            KqlPanopticonError::HttpRequestFailed("Request failed after all retries".into())
        }))
    }

    /// Execute a single HTTP request
    async fn execute_single(
        &self,
        url: &str,
        method: &HttpMethod,
        headers: &HashMap<String, String>,
        params: &HashMap<String, String>,
        body: Option<&serde_json::Value>,
        auth: &Option<AuthMethod>,
    ) -> Result<RawHttpResponse> {
        // Build URL with query params
        let mut url_with_params = reqwest::Url::parse(url)
            .map_err(|e| KqlPanopticonError::HttpStepError(format!("Invalid URL: {}", e)))?;

        for (key, value) in params {
            url_with_params.query_pairs_mut().append_pair(key, value);
        }

        // Build request
        let mut request_builder = match method {
            HttpMethod::Get => self.http_client.get(url_with_params),
            HttpMethod::Post => self.http_client.post(url_with_params),
            HttpMethod::Put => self.http_client.put(url_with_params),
            HttpMethod::Delete => self.http_client.delete(url_with_params),
        };

        // Add headers
        for (key, value) in headers {
            request_builder = request_builder.header(key, value);
        }

        // Add authentication
        if let Some(auth_method) = auth {
            match auth_method {
                AuthMethod::Azure => {
                    let azure_client = self.azure_client.as_ref().ok_or_else(|| {
                        KqlPanopticonError::HttpStepError(
                            "Azure authentication requested but no Azure client available".into(),
                        )
                    })?;
                    // Get Azure Management API token and add as Bearer header
                    let token = azure_client.get_token_for_management().await?;
                    request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
                }
                AuthMethod::None => {}
            }
        }

        // Add body
        if let Some(body) = body {
            request_builder = request_builder.json(body);
        }

        // Send request
        let response = request_builder
            .send()
            .await
            .map_err(|e| KqlPanopticonError::HttpRequestFailed(e.to_string()))?;

        let status_code = response.status().as_u16();

        // Check for rate limiting
        if status_code == 429 {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);

            return Err(KqlPanopticonError::HttpRateLimited { retry_after });
        }

        // Get response body
        let body = response
            .text()
            .await
            .map_err(|e| KqlPanopticonError::HttpRequestFailed(e.to_string()))?;

        // Check for error status
        if status_code >= 400 {
            return Err(KqlPanopticonError::HttpStepError(format!(
                "HTTP {} error: {}",
                status_code,
                body.chars().take(500).collect::<String>()
            )));
        }

        Ok(RawHttpResponse { status_code, body })
    }

    /// Substitute variables in a string (both regular vars and secrets)
    fn substitute_all(
        &self,
        input: &str,
        substitutions: &HashMap<String, String>,
    ) -> Result<String> {
        // First substitute secrets
        let result = self.secrets.substitute(input)?;

        // Then substitute regular variables
        let var_pattern = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();
        let result = var_pattern
            .replace_all(&result, |caps: &regex::Captures| {
                let var_ref = caps.get(1).unwrap().as_str().trim();
                substitutions
                    .get(var_ref)
                    .cloned()
                    .unwrap_or_else(|| format!("{{{{{}}}}}", var_ref))
            })
            .to_string();

        Ok(result)
    }

    /// Substitute variables in headers
    fn substitute_headers(
        &self,
        headers: &HashMap<String, String>,
        substitutions: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let mut result = HashMap::new();
        for (key, value) in headers {
            let resolved_key = self.substitute_all(key, substitutions)?;
            let resolved_value = self.substitute_all(value, substitutions)?;
            result.insert(resolved_key, resolved_value);
        }
        Ok(result)
    }

    /// Substitute variables in params
    fn substitute_params(
        &self,
        params: &HashMap<String, String>,
        substitutions: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let mut result = HashMap::new();
        for (key, value) in params {
            let resolved_key = self.substitute_all(key, substitutions)?;
            let resolved_value = self.substitute_all(value, substitutions)?;
            result.insert(resolved_key, resolved_value);
        }
        Ok(result)
    }

    /// Substitute variables in request body
    fn substitute_body(
        &self,
        body: &Option<serde_json::Value>,
        substitutions: &HashMap<String, String>,
    ) -> Result<Option<serde_json::Value>> {
        match body {
            Some(value) => {
                let json_str = serde_json::to_string(value)?;
                let substituted = self.substitute_all(&json_str, substitutions)?;
                let result = serde_json::from_str(&substituted)?;
                Ok(Some(result))
            }
            None => Ok(None),
        }
    }

    /// Extract fields from HTTP response using JSONPath
    fn extract_response(
        &self,
        response: &RawHttpResponse,
        config: &HttpResponse,
    ) -> Result<Vec<serde_json::Value>> {
        // Parse response as JSON
        let json: serde_json::Value = serde_json::from_str(&response.body).map_err(|e| {
            KqlPanopticonError::JsonPathError(format!(
                "Failed to parse response as JSON: {}. Response: {}",
                e,
                response.body.chars().take(200).collect::<String>()
            ))
        })?;

        // Extract each field using JSONPath
        let mut extracted_values: HashMap<String, Vec<serde_json::Value>> = HashMap::new();

        for (field_name, json_path) in &config.fields {
            let values = jsonpath_lib::select(&json, json_path).map_err(|e| {
                KqlPanopticonError::JsonPathError(format!(
                    "JSONPath '{}' failed for field '{}': {}",
                    json_path, field_name, e
                ))
            })?;

            extracted_values.insert(
                field_name.clone(),
                values.into_iter().cloned().collect(),
            );
        }

        // Convert to row format
        // If all fields have the same number of values, zip them into rows
        // If fields have different lengths, use the max length and fill with null
        let max_len = extracted_values.values().map(|v| v.len()).max().unwrap_or(0);

        if max_len == 0 {
            // No values extracted, return empty result
            return Ok(vec![]);
        }

        let mut rows = Vec::with_capacity(max_len);
        for i in 0..max_len {
            let mut row = serde_json::Map::new();
            for (field_name, values) in &extracted_values {
                let value = values
                    .get(i)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                row.insert(field_name.clone(), value);
            }
            rows.push(serde_json::Value::Object(row));
        }

        Ok(rows)
    }
}

/// Result from executing an HTTP step
pub struct HttpStepResult {
    #[allow(dead_code)]
    pub status_code: u16,
    pub rows: Vec<serde_json::Value>,
    #[allow(dead_code)]
    pub raw_response: String,
}

/// Raw HTTP response before extraction
struct RawHttpResponse {
    status_code: u16,
    body: String,
}

/// Handle error based on OnError strategy
pub fn handle_http_error(
    error: KqlPanopticonError,
    on_error: &OnError,
    field_names: &[String],
) -> Result<Option<serde_json::Value>> {
    match on_error {
        OnError::Fail => Err(error),
        OnError::Skip => {
            warn!("HTTP request failed, skipping row: {}", error);
            Ok(None)
        }
        OnError::Continue => {
            warn!("HTTP request failed, continuing with error: {}", error);
            // Create a row with null values and an _error field
            let mut row = serde_json::Map::new();
            for field in field_names {
                row.insert(field.clone(), serde_json::Value::Null);
            }
            row.insert(
                "_error".to_string(),
                serde_json::Value::String(error.to_string()),
            );
            Ok(Some(serde_json::Value::Object(row)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_env_var_template() {
        std::env::set_var("TEST_SECRET_VAR", "test_value");
        let result = ResolvedSecrets::resolve_template("${TEST_SECRET_VAR}");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_value");
        std::env::remove_var("TEST_SECRET_VAR");
    }

    #[test]
    fn test_resolve_literal_value() {
        let result = ResolvedSecrets::resolve_template("literal_value");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "literal_value");
    }

    #[test]
    fn test_resolve_missing_env_var() {
        let result = ResolvedSecrets::resolve_template("${NONEXISTENT_VAR_12345}");
        assert!(result.is_err());
    }

    #[test]
    fn test_secrets_substitute() {
        std::env::set_var("TEST_API_KEY", "my_api_key");
        let mut secrets_map = HashMap::new();
        secrets_map.insert("api_key".to_string(), "${TEST_API_KEY}".to_string());

        let config = SecretsConfig {
            secrets: secrets_map,
        };
        let resolved = ResolvedSecrets::resolve(Some(&config)).unwrap();

        let input = "Bearer {{secrets.api_key}}";
        let result = resolved.substitute(input).unwrap();
        assert_eq!(result, "Bearer my_api_key");

        std::env::remove_var("TEST_API_KEY");
    }
}
