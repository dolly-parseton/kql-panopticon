use serde::{Deserialize, Serialize};

/// Represents a Log Analytics workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Response from Azure Management API when listing workspaces
#[derive(Debug, Deserialize)]
pub(crate) struct WorkspaceListResponse {
    pub value: Vec<WorkspaceResource>,
}

/// Individual workspace resource from Azure API
#[derive(Debug, Deserialize)]
pub(crate) struct WorkspaceResource {
    pub id: String,
    pub name: String,
    pub location: String,
    pub properties: WorkspaceProperties,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkspaceProperties {
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
