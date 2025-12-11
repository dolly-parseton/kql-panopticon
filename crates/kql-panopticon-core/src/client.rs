//! Azure Log Analytics client
//!
//! Provides authentication and API access to Azure Log Analytics workspaces.

use crate::error::{Error, Result};
use crate::workspace::{Workspace, WorkspaceListResponse};
use azure_core::auth::TokenCredential;
use azure_identity::AzureCliCredential;
use log::warn;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Cached token with expiry information
#[derive(Clone)]
struct CachedToken {
    token: String,
    expires_at: SystemTime,
}

/// Azure client for querying Log Analytics workspaces
#[derive(Clone)]
pub struct Client {
    credential: Arc<AzureCliCredential>,
    http_client: reqwest::Client,
    last_validated: Arc<std::sync::Mutex<Option<SystemTime>>>,
    validation_interval: Duration,
    query_timeout: Duration,
    retry_count: u32,
    log_analytics_token: Arc<std::sync::Mutex<Option<CachedToken>>>,
}

#[derive(Serialize)]
struct QueryRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timespan: Option<String>,
}

/// Response from a Log Analytics query
#[derive(Deserialize, Debug, Clone)]
pub struct QueryResponse {
    pub tables: Vec<Table>,
    #[serde(rename = "nextLink")]
    pub next_link: Option<String>,
}

/// Table in query response
#[derive(Deserialize, Debug, Clone)]
pub struct Table {
    #[allow(dead_code)]
    pub name: String,
    pub columns: Vec<Column>,
    pub rows: Vec<serde_json::Value>,
}

/// Column metadata
#[derive(Deserialize, Debug, Clone)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub column_type: String,
}

/// Azure subscription
#[derive(Deserialize, Debug, Clone)]
pub struct Subscription {
    #[serde(rename = "subscriptionId")]
    pub subscription_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[allow(dead_code)]
    pub state: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
}

#[derive(Deserialize, Debug)]
struct SubscriptionListResponse {
    value: Vec<Subscription>,
}

/// Azure API error response structure
#[derive(Deserialize, Debug)]
struct AzureErrorResponse {
    error: AzureError,
}

#[derive(Deserialize, Debug)]
struct AzureError {
    code: Option<String>,
    message: String,
    #[serde(default)]
    details: Vec<AzureErrorDetail>,
    innererror: Option<AzureInnerError>,
}

#[derive(Deserialize, Debug)]
struct AzureErrorDetail {
    #[allow(dead_code)]
    code: Option<String>,
    message: String,
}

#[derive(Deserialize, Debug)]
struct AzureInnerError {
    #[allow(dead_code)]
    code: Option<String>,
    message: Option<String>,
}

impl Client {
    /// Create a new client using Azure CLI credentials
    pub fn new() -> Result<Self> {
        Self::with_config(
            Duration::from_secs(300), // 5 minutes validation interval
            Duration::from_secs(30),  // 30 seconds query timeout
            0,                        // 0 retries by default
        )
    }

    /// Create a new client with a custom validation interval
    #[allow(dead_code)]
    pub fn with_validation_interval(validation_interval: Duration) -> Result<Self> {
        Self::with_config(validation_interval, Duration::from_secs(30), 0)
    }

    /// Create a new client with full configuration
    pub fn with_config(
        validation_interval: Duration,
        query_timeout: Duration,
        retry_count: u32,
    ) -> Result<Self> {
        let credential = AzureCliCredential::new();
        let http_client = reqwest::Client::builder()
            .timeout(query_timeout)
            .build()
            .map_err(|e| Error::azure(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            credential: Arc::new(credential),
            http_client,
            last_validated: Arc::new(std::sync::Mutex::new(None)),
            validation_interval,
            query_timeout,
            retry_count,
            log_analytics_token: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    /// Get the configured query timeout
    pub fn query_timeout(&self) -> Duration {
        self.query_timeout
    }

    /// Get the configured retry count
    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    /// Validate that the Azure CLI authentication is still valid
    pub async fn validate_auth(&self) -> Result<()> {
        // Check if we need to revalidate based on the interval
        let should_validate = {
            let last_validated = self.last_validated.lock().map_err(|e| {
                Error::azure(format!("Auth validation lock poisoned: {}", e))
            })?;
            match *last_validated {
                None => true,
                Some(last_time) => {
                    SystemTime::now()
                        .duration_since(last_time)
                        .unwrap_or(Duration::from_secs(0))
                        >= self.validation_interval
                }
            }
        };

        if !should_validate {
            return Ok(());
        }

        // Try to get a token to validate authentication
        match self.get_token_for_management().await {
            Ok(_) => {
                let mut last_validated = self.last_validated.lock().map_err(|e| {
                    Error::azure(format!("Auth validation lock poisoned: {}", e))
                })?;
                *last_validated = Some(SystemTime::now());
                Ok(())
            }
            Err(e) => Err(Error::Azure {
                message: format!("Please run 'az login' to authenticate. Error: {}", e),
                source: None,
            }),
        }
    }

    /// Force validation of authentication regardless of interval
    pub async fn force_validate_auth(&self) -> Result<()> {
        match self.get_token_for_management().await {
            Ok(_) => {
                let mut last_validated = self.last_validated.lock().map_err(|e| {
                    Error::azure(format!("Auth validation lock poisoned: {}", e))
                })?;
                *last_validated = Some(SystemTime::now());
                Ok(())
            }
            Err(e) => Err(Error::Azure {
                message: format!("Please run 'az login' to authenticate. Error: {}", e),
                source: None,
            }),
        }
    }

    /// Get a token for Azure Management API
    pub async fn get_token_for_management(&self) -> Result<String> {
        let token = self
            .credential
            .get_token(&["https://management.azure.com/.default"])
            .await
            .map_err(|e| {
                Error::azure(format!("Failed to get management token: {}", e))
            })?;

        Ok(token.token.secret().to_string())
    }

    /// Get a token for Log Analytics API with caching and expiry tracking
    async fn get_token_for_log_analytics(&self) -> Result<String> {
        const TOKEN_REFRESH_BUFFER: Duration = Duration::from_secs(300); // 5 minutes before expiry

        {
            let cached = self.log_analytics_token.lock().map_err(|e| {
                Error::azure(format!("Token cache lock poisoned: {}", e))
            })?;

            if let Some(cached_token) = cached.as_ref() {
                if let Ok(time_until_expiry) =
                    cached_token.expires_at.duration_since(SystemTime::now())
                {
                    if time_until_expiry > TOKEN_REFRESH_BUFFER {
                        log::debug!(
                            "Using cached Log Analytics token (expires in {:?})",
                            time_until_expiry
                        );
                        return Ok(cached_token.token.clone());
                    } else {
                        log::debug!(
                            "Cached token expiring soon (in {:?}), refreshing",
                            time_until_expiry
                        );
                    }
                }
            }
        }

        // No valid cached token, fetch a new one
        log::debug!("Fetching new Log Analytics token");
        let token = self
            .credential
            .get_token(&["https://api.loganalytics.io/.default"])
            .await
            .map_err(|e| {
                Error::azure(format!("Failed to get Log Analytics token: {}", e))
            })?;

        let token_string = token.token.secret().to_string();
        let expires_at =
            SystemTime::UNIX_EPOCH + Duration::from_secs(token.expires_on.unix_timestamp() as u64);

        // Cache the new token
        {
            let mut cached = self.log_analytics_token.lock().map_err(|e| {
                Error::azure(format!("Token cache lock poisoned: {}", e))
            })?;
            *cached = Some(CachedToken {
                token: token_string.clone(),
                expires_at,
            });

            if let Ok(duration) = expires_at.duration_since(SystemTime::now()) {
                log::debug!("Cached new token (expires in {:?})", duration);
            }
        }

        Ok(token_string)
    }

    /// Parse Azure error response and create a detailed error message
    fn parse_azure_error(status: u16, error_text: &str, context: &str) -> Error {
        if let Ok(azure_error) = serde_json::from_str::<AzureErrorResponse>(error_text) {
            let mut message = azure_error.error.message.clone();

            if let Some(code) = &azure_error.error.code {
                message = format!("{}: {}", code, message);
            }

            if let Some(inner) = &azure_error.error.innererror {
                if let Some(inner_msg) = &inner.message {
                    message.push_str(&format!("\n  Details: {}", inner_msg));
                }
            }

            for detail in &azure_error.error.details {
                message.push_str(&format!("\n  - {}", detail.message));
            }

            Error::Http {
                message: format!("{}: {}", context, message),
                url: None,
                status: Some(status),
            }
        } else {
            Error::Http {
                message: format!("{}: {}", context, error_text),
                url: None,
                status: Some(status),
            }
        }
    }

    /// Parse Retry-After header from HTTP response
    fn parse_retry_after(response: &reqwest::Response) -> u64 {
        response
            .headers()
            .get("Retry-After")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(60)
    }

    /// List all subscriptions the user has access to
    pub async fn list_subscriptions(&self) -> Result<Vec<Subscription>> {
        self.validate_auth().await?;

        let token = self.get_token_for_management().await?;
        let url = "https://management.azure.com/subscriptions?api-version=2020-01-01";

        let response = self
            .http_client
            .get(url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::Http {
                message: format!("Failed to list subscriptions: {}", error_text),
                url: Some(url.to_string()),
                status: Some(status),
            });
        }

        let subscription_response: SubscriptionListResponse = response
            .json()
            .await
            .map_err(|e| Error::azure(format!("Failed to parse subscriptions: {}", e)))?;

        if subscription_response.value.is_empty() {
            return Err(Error::azure("No subscriptions found"));
        }

        Ok(subscription_response.value)
    }

    /// Query a single Log Analytics workspace
    pub async fn query_workspace(
        &self,
        workspace_id: &str,
        query: &str,
        timespan: Option<&str>,
    ) -> Result<QueryResponse> {
        self.validate_auth().await?;

        let token = self.get_token_for_log_analytics().await?;
        let url = format!(
            "https://api.loganalytics.io/v1/workspaces/{}/query",
            workspace_id
        );

        let body = QueryRequest {
            query: query.to_string(),
            timespan: timespan.map(|s| s.to_string()),
        };

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();

            // Check for rate limiting (429)
            if status == 429 {
                let retry_after = Self::parse_retry_after(&response);
                let error_text = response.text().await.unwrap_or_default();
                warn!(
                    "Rate limited on workspace {}. Retry after {} seconds. Details: {}",
                    workspace_id, retry_after, error_text
                );
                return Err(Error::Http {
                    message: format!("Rate limited. Retry after {} seconds", retry_after),
                    url: Some(url),
                    status: Some(status),
                });
            }

            let error_text = response.text().await.unwrap_or_default();
            return Err(Self::parse_azure_error(
                status,
                &error_text,
                &format!("Query failed for workspace {}", workspace_id),
            ));
        }

        let result: QueryResponse = response
            .json()
            .await
            .map_err(|e| Error::azure(format!("Failed to parse query response: {}", e)))?;

        Ok(result)
    }

    /// Query the next page using a nextLink URL from a previous QueryResponse
    pub async fn query_next_page(&self, next_link: &str) -> Result<QueryResponse> {
        self.validate_auth().await?;

        let token = self.get_token_for_log_analytics().await?;

        let response = self
            .http_client
            .get(next_link)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();

            if status == 429 {
                let retry_after = Self::parse_retry_after(&response);
                let error_text = response.text().await.unwrap_or_default();
                warn!(
                    "Rate limited during pagination. Retry after {} seconds. Details: {}",
                    retry_after, error_text
                );
                return Err(Error::Http {
                    message: format!("Rate limited. Retry after {} seconds", retry_after),
                    url: Some(next_link.to_string()),
                    status: Some(status),
                });
            }

            let error_text = response.text().await.unwrap_or_default();
            return Err(Self::parse_azure_error(
                status,
                &error_text,
                "Pagination failed",
            ));
        }

        let result: QueryResponse = response
            .json()
            .await
            .map_err(|e| Error::azure(format!("Failed to parse pagination response: {}", e)))?;

        Ok(result)
    }

    /// List all Log Analytics workspaces across all subscriptions
    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        self.validate_auth().await?;

        let subscriptions = self.list_subscriptions().await?;
        let token = self.get_token_for_management().await?;

        let mut all_workspaces = Vec::new();

        for subscription in subscriptions {
            let url = format!(
                "https://management.azure.com/subscriptions/{}/providers/Microsoft.OperationalInsights/workspaces?api-version=2021-06-01",
                subscription.subscription_id
            );

            let response = match self
                .http_client
                .get(&url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    warn!(
                        "Warning: Failed to list workspaces in subscription '{}' ({}): {}",
                        subscription.display_name, subscription.subscription_id, e
                    );
                    continue;
                }
            };

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let error_text = response.text().await.unwrap_or_default();
                warn!(
                    "Warning: Failed to list workspaces in subscription '{}' ({}): HTTP {} - {}",
                    subscription.display_name, subscription.subscription_id, status, error_text
                );
                continue;
            }

            let workspace_response: WorkspaceListResponse = match response.json().await {
                Ok(resp) => resp,
                Err(e) => {
                    warn!(
                        "Warning: Failed to parse workspace list for subscription '{}' ({}): {}",
                        subscription.display_name, subscription.subscription_id, e
                    );
                    continue;
                }
            };

            if workspace_response.value.is_empty() {
                warn!(
                    "Warning: No workspaces found in subscription '{}' ({})",
                    subscription.display_name, subscription.subscription_id
                );
                continue;
            }

            for workspace_resource in workspace_response.value {
                let workspace = Workspace::from((
                    workspace_resource,
                    subscription.subscription_id.clone(),
                    subscription.tenant_id.clone(),
                    subscription.display_name.clone(),
                ));
                all_workspaces.push(workspace);
            }
        }

        if all_workspaces.is_empty() {
            return Err(Error::azure(
                "No Log Analytics workspaces found in any subscription",
            ));
        }

        Ok(all_workspaces)
    }
}

impl QueryResponse {
    /// Get column names as a vector
    pub fn column_names(&self) -> Vec<&str> {
        self.tables
            .first()
            .map(|t| t.columns.iter().map(|c| c.name.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get row count
    pub fn row_count(&self) -> usize {
        self.tables.first().map(|t| t.rows.len()).unwrap_or(0)
    }

    /// Check if there are more results
    pub fn has_more(&self) -> bool {
        self.next_link.is_some()
    }
}
