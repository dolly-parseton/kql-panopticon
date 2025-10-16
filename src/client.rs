use crate::error::{KqlPanopticonError, Result};
use crate::workspace::{Workspace, WorkspaceListResponse};
use azure_core::auth::TokenCredential;
use azure_identity::AzureCliCredential;
use log::warn;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Azure client for querying Log Analytics workspaces
#[derive(Clone)]
pub struct Client {
    credential: Arc<AzureCliCredential>,
    http_client: reqwest::Client,
    last_validated: Arc<std::sync::Mutex<Option<SystemTime>>>,
    validation_interval: Duration,
    query_timeout: Duration,
    retry_count: u32,
}

#[derive(Serialize)]
struct QueryRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timespan: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct QueryResponse {
    pub tables: Vec<Table>,
    #[serde(rename = "nextLink")]
    pub next_link: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Table {
    #[allow(dead_code)]
    pub name: String,
    pub columns: Vec<Column>,
    pub rows: Vec<serde_json::Value>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub column_type: String,
}

#[derive(Deserialize, Debug)]
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

impl Client {
    /// Create a new client using Azure CLI credentials
    pub fn new() -> Result<Self> {
        Self::with_config(
            Duration::from_secs(300), // 5 minutes validation interval
            Duration::from_secs(30),  // 30 seconds query timeout
            0,                         // 0 retries by default
        )
    }

    /// Create a new client with a custom validation interval (deprecated, use with_config)
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
            .map_err(|e| KqlPanopticonError::HttpRequestFailed(e.to_string()))?;

        Ok(Self {
            credential: Arc::new(credential),
            http_client,
            last_validated: Arc::new(std::sync::Mutex::new(None)),
            validation_interval,
            query_timeout,
            retry_count,
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
    /// This will check if the token can be acquired and if the validation interval has passed
    pub async fn validate_auth(&self) -> Result<()> {
        // Check if we need to revalidate based on the interval
        let should_validate = {
            let last_validated = self.last_validated.lock().unwrap();
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
                // Update the last validated time
                let mut last_validated = self.last_validated.lock().unwrap();
                *last_validated = Some(SystemTime::now());
                Ok(())
            }
            Err(e) => Err(KqlPanopticonError::AuthenticationFailed(format!(
                "Please run 'az login' to authenticate. Error: {}",
                e
            ))),
        }
    }

    /// Force validation of authentication regardless of interval
    pub async fn force_validate_auth(&self) -> Result<()> {
        match self.get_token_for_management().await {
            Ok(_) => {
                let mut last_validated = self.last_validated.lock().unwrap();
                *last_validated = Some(SystemTime::now());
                Ok(())
            }
            Err(e) => Err(KqlPanopticonError::AuthenticationFailed(format!(
                "Please run 'az login' to authenticate. Error: {}",
                e
            ))),
        }
    }

    /// Get a token for Azure Management API
    async fn get_token_for_management(&self) -> Result<String> {
        let token = self
            .credential
            .get_token(&["https://management.azure.com/.default"])
            .await
            .map_err(|e| {
                KqlPanopticonError::TokenAcquisitionFailed(format!(
                    "Failed to get management token: {}",
                    e
                ))
            })?;

        Ok(token.token.secret().to_string())
    }

    /// Get a token for Log Analytics API
    async fn get_token_for_log_analytics(&self) -> Result<String> {
        let token = self
            .credential
            .get_token(&["https://api.loganalytics.io/.default"])
            .await
            .map_err(|e| {
                KqlPanopticonError::TokenAcquisitionFailed(format!(
                    "Failed to get Log Analytics token: {}",
                    e
                ))
            })?;

        Ok(token.token.secret().to_string())
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
            return Err(KqlPanopticonError::AzureApiError {
                status,
                message: error_text,
            });
        }

        let subscription_response: SubscriptionListResponse = response
            .json()
            .await
            .map_err(|e| KqlPanopticonError::JsonParseFailed(e.to_string()))?;

        if subscription_response.value.is_empty() {
            return Err(KqlPanopticonError::NoSubscriptionsFound);
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
            let error_text = response.text().await.unwrap_or_default();
            return Err(KqlPanopticonError::AzureApiError {
                status,
                message: format!(
                    "Query failed for workspace {}: {}",
                    workspace_id, error_text
                ),
            });
        }

        let result: QueryResponse = response
            .json()
            .await
            .map_err(|e| KqlPanopticonError::JsonParseFailed(e.to_string()))?;

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
            let error_text = response.text().await.unwrap_or_default();
            return Err(KqlPanopticonError::AzureApiError {
                status,
                message: format!("Pagination failed: {}", error_text),
            });
        }

        let result: QueryResponse = response
            .json()
            .await
            .map_err(|e| KqlPanopticonError::JsonParseFailed(e.to_string()))?;

        Ok(result)
    }

    /// List all Log Analytics workspaces across all subscriptions
    /// Returns all workspaces found, with warnings for failed or empty subscriptions
    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        self.validate_auth().await?;

        // Get all subscriptions
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

            // Convert workspace resources to Workspace structs
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
            return Err(KqlPanopticonError::WorkspaceNotFound(
                "No Log Analytics workspaces found in any subscription".to_string(),
            ));
        }

        Ok(all_workspaces)
    }
}
