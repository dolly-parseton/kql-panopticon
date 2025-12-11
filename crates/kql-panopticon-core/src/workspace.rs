//! Workspace model and utilities
//!
//! Represents Azure Log Analytics workspaces.

use serde::{Deserialize, Serialize};

/// Represents a Log Analytics workspace
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Workspace {
    /// The workspace GUID used for querying
    pub workspace_id: String,

    /// The full Azure resource ID
    pub resource_id: String,

    /// The workspace name
    pub name: String,

    /// The Azure region/location
    pub location: String,

    /// The subscription ID this workspace belongs to
    pub subscription_id: String,

    /// The resource group name
    pub resource_group: String,

    /// The tenant ID (for Lighthouse support)
    pub tenant_id: String,

    /// The subscription display name
    pub subscription_name: String,
}

impl Workspace {
    /// Normalize a name to be safe for use as a folder name
    /// Replaces spaces and special characters with underscores, converts to lowercase
    pub fn normalize_name(name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect()
    }

    /// Get a display name for the workspace
    pub fn display_name(&self) -> String {
        format!("{} ({})", self.name, self.subscription_name)
    }

    /// Get a normalized name suitable for file paths
    pub fn normalized_name(&self) -> String {
        Self::normalize_name(&self.name)
    }

    /// Extract resource group name from resource ID
    /// Resource ID format: /subscriptions/{sub}/resourceGroups/{rg}/providers/Microsoft.OperationalInsights/workspaces/{name}
    pub fn extract_resource_group(resource_id: &str) -> Option<String> {
        let parts: Vec<&str> = resource_id.split('/').collect();

        // Find "resourceGroups" and get the next element
        for (i, part) in parts.iter().enumerate() {
            if part.eq_ignore_ascii_case("resourceGroups") && i + 1 < parts.len() {
                return Some(parts[i + 1].to_string());
            }
        }

        None
    }
}

impl std::fmt::Display for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Response from Azure Management API when listing workspaces
#[derive(Debug, Deserialize)]
pub struct WorkspaceListResponse {
    pub value: Vec<WorkspaceResource>,
}

/// Individual workspace resource from Azure API
#[derive(Debug, Deserialize)]
pub struct WorkspaceResource {
    pub id: String,
    pub name: String,
    pub location: String,
    pub properties: WorkspaceProperties,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceProperties {
    #[serde(rename = "customerId")]
    pub customer_id: String,
}

impl From<(WorkspaceResource, String, String, String)> for Workspace {
    fn from(
        (resource, subscription_id, tenant_id, subscription_name): (
            WorkspaceResource,
            String,
            String,
            String,
        ),
    ) -> Self {
        let resource_group = Workspace::extract_resource_group(&resource.id)
            .unwrap_or_else(|| "unknown".to_string());

        Workspace {
            workspace_id: resource.properties.customer_id,
            resource_id: resource.id,
            name: resource.name,
            location: resource.location,
            subscription_id,
            resource_group,
            tenant_id,
            subscription_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_name() {
        assert_eq!(Workspace::normalize_name("My Workspace"), "my_workspace");
        assert_eq!(Workspace::normalize_name("test-workspace-1"), "test-workspace-1");
        assert_eq!(Workspace::normalize_name("Test@Workspace#123"), "test_workspace_123");
    }

    #[test]
    fn test_extract_resource_group() {
        let resource_id = "/subscriptions/123/resourceGroups/my-rg/providers/Microsoft.OperationalInsights/workspaces/my-ws";
        assert_eq!(
            Workspace::extract_resource_group(resource_id),
            Some("my-rg".to_string())
        );
    }

    #[test]
    fn test_display_name() {
        let ws = Workspace {
            workspace_id: "guid".to_string(),
            resource_id: "id".to_string(),
            name: "test-ws".to_string(),
            location: "eastus".to_string(),
            subscription_id: "sub".to_string(),
            resource_group: "rg".to_string(),
            tenant_id: "tenant".to_string(),
            subscription_name: "Production".to_string(),
        };
        assert_eq!(ws.display_name(), "test-ws (Production)");
    }
}
