//! Schema registry implementation

use super::types::{ColumnDef, RegistryData, SchemaType, TableInfo, WorkspaceSchema};
use crate::error::{Error, Result};
use chrono::Duration;
use log::{debug, info};
use std::path::{Path, PathBuf};

/// Default staleness threshold (30 days)
const DEFAULT_STALENESS_DAYS: i64 = 30;

/// Schema file name
const SCHEMA_FILE: &str = "schemas.json";

/// Schema registry for tracking table definitions and workspace mappings
///
/// The registry maintains:
/// - Canonical table schemas (shared across workspaces)
/// - Per-workspace information (which tables exist, extensions for extensible tables)
/// - Custom table schemas (workspace-specific *_CL tables)
#[derive(Debug, Clone)]
pub struct SchemaRegistry {
    /// Underlying data
    data: RegistryData,
    /// Path to schema file (None = in-memory only)
    path: Option<PathBuf>,
    /// Whether data has been modified since last save
    dirty: bool,
}

impl SchemaRegistry {
    /// Create a new empty registry (in-memory only)
    pub fn new() -> Self {
        Self {
            data: RegistryData::default(),
            path: None,
            dirty: false,
        }
    }

    /// Create a registry with default schemas
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.load_defaults();
        registry
    }

    /// Load registry from the default location (~/.kql-panopticon/schemas.json)
    pub fn load_default() -> Result<Self> {
        let path = Self::default_path()?;
        Self::load_from_path(&path)
    }

    /// Load registry from a specific path
    pub fn load_from_path(path: &Path) -> Result<Self> {
        if path.exists() {
            debug!("Loading schema registry from {:?}", path);
            let content = std::fs::read_to_string(path)?;
            let data: RegistryData = serde_json::from_str(&content).map_err(|e| {
                Error::config(format!("Failed to parse schema file: {}", e))
            })?;
            info!("Loaded {} tables, {} workspaces from schema registry",
                data.tables.len(), data.workspaces.len());
            Ok(Self {
                data,
                path: Some(path.to_path_buf()),
                dirty: false,
            })
        } else {
            debug!("Schema file not found at {:?}, creating new registry", path);
            let mut registry = Self::with_defaults();
            registry.path = Some(path.to_path_buf());
            registry.dirty = true;
            Ok(registry)
        }
    }

    /// Get the default schema file path
    pub fn default_path() -> Result<PathBuf> {
        let config_dir = dirs::home_dir()
            .ok_or_else(|| Error::config("Could not determine home directory"))?
            .join(".kql-panopticon");
        Ok(config_dir.join(SCHEMA_FILE))
    }

    /// Save registry to disk
    pub fn save(&mut self) -> Result<()> {
        let path = self.path.as_ref().ok_or_else(|| {
            Error::config("No path configured for schema registry")
        })?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&self.data)?;
        std::fs::write(path, content)?;
        self.dirty = false;
        debug!("Saved schema registry to {:?}", path);
        Ok(())
    }

    /// Save if dirty
    pub fn save_if_dirty(&mut self) -> Result<()> {
        if self.dirty {
            self.save()
        } else {
            Ok(())
        }
    }

    /// Check if registry has unsaved changes
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    // ========== Table Operations ==========

    /// Get a canonical table schema
    pub fn get_table(&self, name: &str) -> Option<&TableInfo> {
        self.data.tables.get(name)
    }

    /// Check if a canonical table exists
    pub fn has_table(&self, name: &str) -> bool {
        self.data.tables.contains_key(name)
    }

    /// Add or update a canonical table schema
    pub fn add_table(&mut self, table: TableInfo) {
        self.data.tables.insert(table.name.clone(), table);
        self.dirty = true;
    }

    /// Get all canonical table names
    pub fn table_names(&self) -> Vec<&str> {
        self.data.tables.keys().map(|s| s.as_str()).collect()
    }

    /// Get all canonical tables
    pub fn tables(&self) -> impl Iterator<Item = &TableInfo> {
        self.data.tables.values()
    }

    // ========== Workspace Operations ==========

    /// Get workspace schema info
    pub fn get_workspace(&self, workspace_id: &str) -> Option<&WorkspaceSchema> {
        self.data.workspaces.get(workspace_id)
    }

    /// Get mutable workspace schema (creates if not exists)
    pub fn get_or_create_workspace(&mut self, workspace_id: &str) -> &mut WorkspaceSchema {
        self.dirty = true;
        self.data.workspaces
            .entry(workspace_id.to_string())
            .or_insert_with(|| WorkspaceSchema::new(workspace_id))
    }

    /// Check if workspace has a specific table
    pub fn workspace_has_table(&self, workspace_id: &str, table_name: &str) -> bool {
        self.data.workspaces
            .get(workspace_id)
            .map(|ws| ws.has_table(table_name))
            .unwrap_or(false)
    }

    /// Add a table to a workspace's known tables
    pub fn add_workspace_table(&mut self, workspace_id: &str, table_name: &str) {
        self.get_or_create_workspace(workspace_id)
            .add_table(table_name);
    }

    /// Get all workspace IDs
    pub fn workspace_ids(&self) -> Vec<&str> {
        self.data.workspaces.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a workspace's schema is stale
    pub fn is_workspace_stale(&self, workspace_id: &str) -> bool {
        self.data.workspaces
            .get(workspace_id)
            .map(|ws| ws.is_stale(Duration::days(DEFAULT_STALENESS_DAYS)))
            .unwrap_or(true)
    }

    /// Mark workspace as updated
    pub fn touch_workspace(&mut self, workspace_id: &str) {
        self.get_or_create_workspace(workspace_id).touch();
    }

    // ========== Extended Schema Operations ==========

    /// Get full column list for a table in a specific workspace
    /// (combines canonical columns with workspace extensions)
    pub fn get_columns_for_workspace(
        &self,
        table_name: &str,
        workspace_id: &str,
    ) -> Option<Vec<&ColumnDef>> {
        // First check canonical schema
        let table = self.data.tables.get(table_name)?;

        let mut columns: Vec<&ColumnDef> = table.columns.iter().collect();

        // Add workspace-specific extensions for extensible tables
        if table.schema_type == SchemaType::Extensible {
            if let Some(ws) = self.data.workspaces.get(workspace_id) {
                if let Some(extensions) = ws.extensions.get(table_name) {
                    columns.extend(extensions.iter());
                }
            }
        }

        Some(columns)
    }

    /// Get a custom table schema from a specific workspace
    pub fn get_custom_table(&self, workspace_id: &str, table_name: &str) -> Option<&TableInfo> {
        self.data.workspaces
            .get(workspace_id)?
            .custom_tables
            .get(table_name)
    }

    /// Add extension columns to an extensible table for a workspace
    pub fn add_table_extensions(
        &mut self,
        workspace_id: &str,
        table_name: &str,
        columns: Vec<ColumnDef>,
    ) {
        self.get_or_create_workspace(workspace_id)
            .add_extensions(table_name, columns);
    }

    /// Add a custom table to a workspace
    pub fn add_custom_table(&mut self, workspace_id: &str, table: TableInfo) {
        self.get_or_create_workspace(workspace_id)
            .add_custom_table(table);
    }

    // ========== Validation Integration ==========

    /// Convert to FFI validation schema (for a specific workspace)
    pub fn to_validation_schema(&self, workspace_id: Option<&str>) -> kql_language_ffi::Schema {
        let mut schema = kql_language_ffi::Schema::new();

        // Add canonical tables
        for table_info in self.data.tables.values() {
            let mut table = kql_language_ffi::Table::new(&table_info.name);

            // Add base columns
            for col in &table_info.columns {
                table = table.with_column(&col.name, &col.data_type);
            }

            // Add workspace extensions if applicable
            if table_info.schema_type == SchemaType::Extensible {
                if let Some(ws_id) = workspace_id {
                    if let Some(ws) = self.data.workspaces.get(ws_id) {
                        if let Some(extensions) = ws.extensions.get(&table_info.name) {
                            for col in extensions {
                                table = table.with_column(&col.name, &col.data_type);
                            }
                        }
                    }
                }
            }

            schema = schema.table(table);
        }

        // Add workspace custom tables
        if let Some(ws_id) = workspace_id {
            if let Some(ws) = self.data.workspaces.get(ws_id) {
                for custom_table in ws.custom_tables.values() {
                    let mut table = kql_language_ffi::Table::new(&custom_table.name);
                    for col in &custom_table.columns {
                        table = table.with_column(&col.name, &col.data_type);
                    }
                    schema = schema.table(table);
                }
            }
        }

        schema
    }

    // ========== Defaults ==========

    /// Load default table schemas
    fn load_defaults(&mut self) {
        // Common base columns for most tables
        let time_col = ColumnDef::datetime("TimeGenerated");
        let tenant_col = ColumnDef::string("TenantId");

        // SecurityEvent - Windows Security logs (comprehensive schema)
        self.add_table(
            TableInfo::canonical("SecurityEvent")
                .source("sentinel")
                .description("Security events collected from Windows machines")
                .column(time_col.clone())
                .column(tenant_col.clone())
                .with_column("AccessMask", "string")
                .with_column("Account", "string")
                .with_column("AccountDomain", "string")
                .with_column("AccountExpires", "string")
                .with_column("AccountName", "string")
                .with_column("AccountSessionIdentifier", "string")
                .with_column("AccountType", "string")
                .with_column("Activity", "string")
                .with_column("AdditionalInfo", "string")
                .with_column("AdditionalInfo2", "string")
                .with_column("AuthenticationPackageName", "string")
                .with_column("CallerProcessId", "string")
                .with_column("CallerProcessName", "string")
                .with_column("Channel", "string")
                .with_column("ClientAddress", "string")
                .with_column("ClientIPAddress", "string")
                .with_column("ClientName", "string")
                .with_column("CommandLine", "string")
                .with_column("Computer", "string")
                .with_column("DCDNSName", "string")
                .with_column("DeviceDescription", "string")
                .with_column("DeviceId", "string")
                .with_column("DomainName", "string")
                .with_column("ElevatedToken", "string")
                .with_column("ErrorCode", "int")
                .with_column("EventData", "string")
                .with_column("EventID", "int")
                .with_column("EventLevelName", "string")
                .with_column("EventRecordId", "string")
                .with_column("EventSourceName", "string")
                .with_column("FailureReason", "string")
                .with_column("FileHash", "string")
                .with_column("FilePath", "string")
                .with_column("HandleId", "string")
                .with_column("IpAddress", "string")
                .with_column("IpPort", "string")
                .with_column("KeyLength", "int")
                .with_column("Level", "string")
                .with_column("LmPackageName", "string")
                .with_column("LogonGuid", "string")
                .with_column("LogonID", "string")
                .with_column("LogonProcessName", "string")
                .with_column("LogonType", "int")
                .with_column("LogonTypeName", "string")
                .with_column("MemberName", "string")
                .with_column("MemberSid", "string")
                .with_column("NewProcessId", "string")
                .with_column("NewProcessName", "string")
                .with_column("ObjectName", "string")
                .with_column("ObjectServer", "string")
                .with_column("ObjectType", "string")
                .with_column("PackageName", "string")
                .with_column("ParentProcessName", "string")
                .with_column("PrivilegeList", "string")
                .with_column("Process", "string")
                .with_column("ProcessId", "string")
                .with_column("ProcessName", "string")
                .with_column("Properties", "string")
                .with_column("SourceComputerId", "string")
                .with_column("SourceSystem", "string")
                .with_column("Status", "string")
                .with_column("SubcategoryGuid", "string")
                .with_column("SubcategoryId", "string")
                .with_column("Subject", "string")
                .with_column("SubjectAccount", "string")
                .with_column("SubjectDomainName", "string")
                .with_column("SubjectLogonId", "string")
                .with_column("SubjectUserName", "string")
                .with_column("SubjectUserSid", "string")
                .with_column("SubStatus", "string")
                .with_column("TargetAccount", "string")
                .with_column("TargetDomainName", "string")
                .with_column("TargetInfo", "string")
                .with_column("TargetLinkedLogonId", "string")
                .with_column("TargetLogonGuid", "string")
                .with_column("TargetLogonId", "string")
                .with_column("TargetServerName", "string")
                .with_column("TargetSid", "string")
                .with_column("TargetUser", "string")
                .with_column("TargetUserName", "string")
                .with_column("TargetUserSid", "string")
                .with_column("Task", "int")
                .with_column("TokenElevationType", "string")
                .with_column("TransmittedServices", "string")
                .with_column("Type", "string")
                .with_column("UserAccountControl", "string")
                .with_column("UserPrincipalName", "string")
                .with_column("VirtualAccount", "string")
                .with_column("Workstation", "string")
                .with_column("WorkstationName", "string")
        );

        // SigninLogs - Azure AD sign-in logs (comprehensive schema)
        self.add_table(
            TableInfo::canonical("SigninLogs")
                .source("sentinel")
                .description("Azure AD sign-in logs")
                .column(time_col.clone())
                .with_column("AADTenantId", "string")
                .with_column("AlternateSignInName", "string")
                .with_column("AppDisplayName", "string")
                .with_column("AppId", "string")
                .with_column("AppOwnerTenantId", "string")
                .with_column("AuthenticationDetails", "string")
                .with_column("AuthenticationMethodsUsed", "string")
                .with_column("AuthenticationProcessingDetails", "string")
                .with_column("AuthenticationProtocol", "string")
                .with_column("AuthenticationRequirement", "string")
                .with_column("AuthenticationRequirementPolicies", "string")
                .with_column("AutonomousSystemNumber", "string")
                .with_column("Category", "string")
                .with_column("ClientAppUsed", "string")
                .with_column("ConditionalAccessPolicies", "dynamic")
                .with_column("ConditionalAccessStatus", "string")
                .with_column("CorrelationId", "string")
                .with_column("CreatedDateTime", "datetime")
                .with_column("CrossTenantAccessType", "string")
                .with_column("DeviceDetail", "dynamic")
                .with_column("DurationMs", "long")
                .with_column("FlaggedForReview", "bool")
                .with_column("HomeTenantId", "string")
                .with_column("Id", "string")
                .with_column("Identity", "string")
                .with_column("IPAddress", "string")
                .with_column("IPAddressFromResourceProvider", "string")
                .with_column("IsInteractive", "bool")
                .with_column("IsRisky", "bool")
                .with_column("IsTenantRestricted", "bool")
                .with_column("Level", "string")
                .with_column("Location", "string")
                .with_column("LocationDetails", "dynamic")
                .with_column("MfaDetail", "dynamic")
                .with_column("NetworkLocationDetails", "string")
                .with_column("OperationName", "string")
                .with_column("OriginalRequestId", "string")
                .with_column("ProcessingTimeInMilliseconds", "string")
                .with_column("ResourceDisplayName", "string")
                .with_column("ResourceId", "string")
                .with_column("ResourceIdentity", "string")
                .with_column("ResourceServicePrincipalId", "string")
                .with_column("ResourceTenantId", "string")
                .with_column("ResultDescription", "string")
                .with_column("ResultSignature", "string")
                .with_column("ResultType", "string")
                .with_column("RiskDetail", "string")
                .with_column("RiskEventTypes", "string")
                .with_column("RiskEventTypes_V2", "string")
                .with_column("RiskLevel", "string")
                .with_column("RiskLevelAggregated", "string")
                .with_column("RiskLevelDuringSignIn", "string")
                .with_column("RiskState", "string")
                .with_column("ServicePrincipalId", "string")
                .with_column("ServicePrincipalName", "string")
                .with_column("SessionId", "string")
                .with_column("SignInIdentifier", "string")
                .with_column("SignInIdentifierType", "string")
                .with_column("SourceSystem", "string")
                .with_column("Status", "dynamic")
                .with_column("TokenIssuerName", "string")
                .with_column("TokenIssuerType", "string")
                .with_column("Type", "string")
                .with_column("UniqueTokenIdentifier", "string")
                .with_column("UserAgent", "string")
                .with_column("UserDisplayName", "string")
                .with_column("UserId", "string")
                .with_column("UserPrincipalName", "string")
                .with_column("UserType", "string")
        );

        // AADUserRiskEvents - Azure AD Identity Protection risk events
        self.add_table(
            TableInfo::canonical("AADUserRiskEvents")
                .source("sentinel")
                .description("Logs generated by identity protection for Azure AD user risk events")
                .column(time_col.clone())
                .column(tenant_col.clone())
                .with_column("Activity", "string")
                .with_column("ActivityDateTime", "datetime")
                .with_column("AdditionalInfo", "dynamic")
                .with_column("CorrelationId", "string")
                .with_column("DetectedDateTime", "datetime")
                .with_column("DetectionTimingType", "string")
                .with_column("Id", "string")
                .with_column("IpAddress", "string")
                .with_column("LastUpdatedDateTime", "datetime")
                .with_column("Location", "dynamic")
                .with_column("OperationName", "string")
                .with_column("RequestId", "string")
                .with_column("RiskDetail", "string")
                .with_column("RiskEventType", "string")
                .with_column("RiskLevel", "string")
                .with_column("RiskState", "string")
                .with_column("Source", "string")
                .with_column("SourceSystem", "string")
                .with_column("TokenIssuerType", "string")
                .with_column("Type", "string")
                .with_column("UserDisplayName", "string")
                .with_column("UserId", "string")
                .with_column("UserPrincipalName", "string")
        );

        // AzureDiagnostics - extensible table (schema varies by resource)
        self.add_table(
            TableInfo::extensible("AzureDiagnostics")
                .source("sentinel")
                .description("Azure resource diagnostic logs (schema varies by resource)")
                .with_column("Category", "string")
                .with_column("CorrelationId", "string")
                .with_column("DurationMs", "string")
                .with_column("Level", "string")
                .with_column("Message", "string")
                .with_column("OperationName", "string")
                .with_column("OperationVersion", "string")
                .with_column("Resource", "string")
                .with_column("ResourceGroup", "string")
                .with_column("ResourceProvider", "string")
                .with_column("ResourceType", "string")
                .with_column("ResultDescription", "string")
                .with_column("ResultSignature", "string")
                .with_column("ResultType", "string")
                .with_column("TimeGenerated", "datetime")
                .with_column("_ResourceId", "string")
        );

        // Syslog - Linux syslog events
        self.add_table(
            TableInfo::canonical("Syslog")
                .source("sentinel")
                .description("Syslog events on Linux computers using the Log Analytics agent")
                .column(time_col.clone())
                .with_column("CollectorHostName", "string")
                .with_column("Computer", "string")
                .with_column("EventTime", "datetime")
                .with_column("Facility", "string")
                .with_column("HostIP", "string")
                .with_column("HostName", "string")
                .with_column("ProcessID", "int")
                .with_column("ProcessName", "string")
                .with_column("SeverityLevel", "string")
                .with_column("SourceSystem", "string")
                .with_column("SyslogMessage", "string")
                .with_column("Type", "string")
                .with_column("_ResourceId", "string")
                .with_column("_SubscriptionId", "string")
        );

        // CommonSecurityLog (CEF) - comprehensive schema
        self.add_table(
            TableInfo::canonical("CommonSecurityLog")
                .source("sentinel")
                .description("Common Event Format (CEF) logs from security appliances")
                .column(time_col.clone())
                .column(tenant_col.clone())
                .with_column("Activity", "string")
                .with_column("AdditionalExtensions", "string")
                .with_column("ApplicationProtocol", "string")
                .with_column("CollectorHostName", "string")
                .with_column("CommunicationDirection", "string")
                .with_column("Computer", "string")
                .with_column("DestinationDnsDomain", "string")
                .with_column("DestinationHostName", "string")
                .with_column("DestinationIP", "string")
                .with_column("DestinationMACAddress", "string")
                .with_column("DestinationNTDomain", "string")
                .with_column("DestinationPort", "int")
                .with_column("DestinationProcessId", "int")
                .with_column("DestinationProcessName", "string")
                .with_column("DestinationServiceName", "string")
                .with_column("DestinationTranslatedAddress", "string")
                .with_column("DestinationTranslatedPort", "int")
                .with_column("DestinationUserID", "string")
                .with_column("DestinationUserName", "string")
                .with_column("DestinationUserPrivileges", "string")
                .with_column("DeviceAction", "string")
                .with_column("DeviceAddress", "string")
                .with_column("DeviceCustomString1", "string")
                .with_column("DeviceCustomString1Label", "string")
                .with_column("DeviceCustomString2", "string")
                .with_column("DeviceCustomString2Label", "string")
                .with_column("DeviceCustomString3", "string")
                .with_column("DeviceCustomString3Label", "string")
                .with_column("DeviceCustomString4", "string")
                .with_column("DeviceCustomString4Label", "string")
                .with_column("DeviceCustomString5", "string")
                .with_column("DeviceCustomString5Label", "string")
                .with_column("DeviceCustomString6", "string")
                .with_column("DeviceCustomString6Label", "string")
                .with_column("DeviceDnsDomain", "string")
                .with_column("DeviceEventCategory", "string")
                .with_column("DeviceEventClassID", "string")
                .with_column("DeviceExternalID", "string")
                .with_column("DeviceFacility", "string")
                .with_column("DeviceInboundInterface", "string")
                .with_column("DeviceMacAddress", "string")
                .with_column("DeviceName", "string")
                .with_column("DeviceNtDomain", "string")
                .with_column("DeviceOutboundInterface", "string")
                .with_column("DevicePayloadId", "string")
                .with_column("DeviceProduct", "string")
                .with_column("DeviceTimeZone", "string")
                .with_column("DeviceTranslatedAddress", "string")
                .with_column("DeviceVendor", "string")
                .with_column("DeviceVersion", "string")
                .with_column("EndTime", "datetime")
                .with_column("EventCount", "int")
                .with_column("EventOutcome", "string")
                .with_column("EventType", "int")
                .with_column("ExternalID", "int")
                .with_column("FileHash", "string")
                .with_column("FileName", "string")
                .with_column("FilePath", "string")
                .with_column("FilePermission", "string")
                .with_column("FileSize", "int")
                .with_column("FileType", "string")
                .with_column("IndicatorThreatType", "string")
                .with_column("LogSeverity", "string")
                .with_column("MaliciousIP", "string")
                .with_column("MaliciousIPCountry", "string")
                .with_column("MaliciousIPLatitude", "real")
                .with_column("MaliciousIPLongitude", "real")
                .with_column("Message", "string")
                .with_column("ProcessID", "int")
                .with_column("ProcessName", "string")
                .with_column("Protocol", "string")
                .with_column("Reason", "string")
                .with_column("ReceiptTime", "string")
                .with_column("ReceivedBytes", "long")
                .with_column("RemoteIP", "string")
                .with_column("RemotePort", "string")
                .with_column("RequestClientApplication", "string")
                .with_column("RequestContext", "string")
                .with_column("RequestCookies", "string")
                .with_column("RequestMethod", "string")
                .with_column("RequestURL", "string")
                .with_column("SentBytes", "long")
                .with_column("SimplifiedDeviceAction", "string")
                .with_column("SourceDnsDomain", "string")
                .with_column("SourceHostName", "string")
                .with_column("SourceIP", "string")
                .with_column("SourceMACAddress", "string")
                .with_column("SourceNTDomain", "string")
                .with_column("SourcePort", "int")
                .with_column("SourceProcessId", "int")
                .with_column("SourceProcessName", "string")
                .with_column("SourceServiceName", "string")
                .with_column("SourceSystem", "string")
                .with_column("SourceTranslatedAddress", "string")
                .with_column("SourceTranslatedPort", "int")
                .with_column("SourceUserID", "string")
                .with_column("SourceUserName", "string")
                .with_column("SourceUserPrivileges", "string")
                .with_column("StartTime", "datetime")
                .with_column("ThreatConfidence", "string")
                .with_column("ThreatDescription", "string")
                .with_column("ThreatSeverity", "int")
                .with_column("Type", "string")
                .with_column("_ResourceId", "string")
                .with_column("_SubscriptionId", "string")
        );

        // Heartbeat - Agent heartbeat information
        self.add_table(
            TableInfo::canonical("Heartbeat")
                .source("sentinel")
                .description("Records logged by Log Analytics agents once per minute to report on agent health")
                .column(time_col.clone())
                .with_column("Category", "string")
                .with_column("Computer", "string")
                .with_column("ComputerEnvironment", "string")
                .with_column("ComputerIP", "string")
                .with_column("ComputerPrivateIPs", "dynamic")
                .with_column("IsGatewayInstalled", "bool")
                .with_column("ManagementGroupName", "string")
                .with_column("OSMajorVersion", "string")
                .with_column("OSMinorVersion", "string")
                .with_column("OSName", "string")
                .with_column("OSType", "string")
                .with_column("RemoteIPCountry", "string")
                .with_column("RemoteIPLatitude", "real")
                .with_column("RemoteIPLongitude", "real")
                .with_column("Resource", "string")
                .with_column("ResourceGroup", "string")
                .with_column("ResourceId", "string")
                .with_column("ResourceProvider", "string")
                .with_column("ResourceType", "string")
                .with_column("SCAgentChannel", "string")
                .with_column("Solutions", "string")
                .with_column("SourceSystem", "string")
                .with_column("SubscriptionId", "string")
                .with_column("Type", "string")
                .with_column("Version", "string")
                .with_column("VMUUID", "string")
                .with_column("_ResourceId", "string")
                .with_column("_SubscriptionId", "string")
        );

        // AuditLogs - Azure AD audit logs
        self.add_table(
            TableInfo::canonical("AuditLogs")
                .source("sentinel")
                .description("Audit log for Azure Active Directory")
                .column(time_col.clone())
                .with_column("AADOperationType", "string")
                .with_column("AADTenantId", "string")
                .with_column("ActivityDateTime", "datetime")
                .with_column("ActivityDisplayName", "string")
                .with_column("AdditionalDetails", "dynamic")
                .with_column("Category", "string")
                .with_column("CorrelationId", "string")
                .with_column("DurationMs", "long")
                .with_column("Id", "string")
                .with_column("Identity", "string")
                .with_column("InitiatedBy", "dynamic")
                .with_column("Level", "string")
                .with_column("Location", "string")
                .with_column("LoggedByService", "string")
                .with_column("OperationName", "string")
                .with_column("OperationVersion", "string")
                .with_column("Resource", "string")
                .with_column("ResourceGroup", "string")
                .with_column("ResourceId", "string")
                .with_column("ResourceProvider", "string")
                .with_column("Result", "string")
                .with_column("ResultDescription", "string")
                .with_column("ResultReason", "string")
                .with_column("ResultSignature", "string")
                .with_column("ResultType", "string")
                .with_column("SourceSystem", "string")
                .with_column("TargetResources", "dynamic")
                .with_column("Type", "string")
        );

        // OfficeActivity - Office 365 audit logs
        self.add_table(
            TableInfo::canonical("OfficeActivity")
                .source("sentinel")
                .description("Audit logs for Office 365 tenants including Exchange, SharePoint and Teams logs")
                .column(time_col.clone())
                .column(tenant_col.clone())
                .with_column("AADGroupId", "string")
                .with_column("AADTarget", "string")
                .with_column("Activity", "string")
                .with_column("Actor", "string")
                .with_column("ActorContextId", "string")
                .with_column("ActorIpAddress", "string")
                .with_column("Application", "string")
                .with_column("ApplicationId", "string")
                .with_column("AzureActiveDirectory_EventType", "string")
                .with_column("ChannelGuid", "string")
                .with_column("ChannelName", "string")
                .with_column("ChannelType", "string")
                .with_column("ChatName", "string")
                .with_column("ChatThreadId", "string")
                .with_column("Client", "string")
                .with_column("ClientIP", "string")
                .with_column("ClientInfoString", "string")
                .with_column("ClientMachineName", "string")
                .with_column("ClientProcessName", "string")
                .with_column("ClientVersion", "string")
                .with_column("CrossMailboxOperations", "bool")
                .with_column("DestFolder", "string")
                .with_column("DestinationFileExtension", "string")
                .with_column("DestinationFileName", "string")
                .with_column("DestinationRelativeUrl", "string")
                .with_column("DestMailboxId", "string")
                .with_column("DestMailboxOwnerUPN", "string")
                .with_column("EventSource", "string")
                .with_column("ExtendedProperties", "string")
                .with_column("ExternalAccess", "string")
                .with_column("ExtraProperties", "dynamic")
                .with_column("Folder", "string")
                .with_column("Folders", "string")
                .with_column("InternalLogonType", "int")
                .with_column("InterSystemsId", "string")
                .with_column("IntraSystemId", "string")
                .with_column("IsManagedDevice", "bool")
                .with_column("Item", "string")
                .with_column("ItemName", "string")
                .with_column("ItemType", "string")
                .with_column("LoginStatus", "int")
                .with_column("Logon_Type", "string")
                .with_column("LogonUserDisplayName", "string")
                .with_column("LogonUserSid", "string")
                .with_column("MailboxGuid", "string")
                .with_column("MailboxOwnerMasterAccountSid", "string")
                .with_column("MailboxOwnerSid", "string")
                .with_column("MailboxOwnerUPN", "string")
                .with_column("Members", "dynamic")
                .with_column("MessageId", "string")
                .with_column("ModifiedObjectResolvedName", "string")
                .with_column("ModifiedProperties", "string")
                .with_column("Name", "string")
                .with_column("NewValue", "string")
                .with_column("OfficeId", "string")
                .with_column("OfficeObjectId", "string")
                .with_column("OfficeTenantId", "string")
                .with_column("OfficeWorkload", "string")
                .with_column("OldValue", "string")
                .with_column("Operation", "string")
                .with_column("OperationProperties", "dynamic")
                .with_column("OperationScope", "string")
                .with_column("OrganizationId", "string")
                .with_column("OrganizationName", "string")
                .with_column("OriginatingServer", "string")
                .with_column("Parameters", "string")
                .with_column("RecordType", "string")
                .with_column("ResultReasonType", "string")
                .with_column("ResultStatus", "string")
                .with_column("SensitivityLabelId", "string")
                .with_column("SharingType", "string")
                .with_column("Site_", "string")
                .with_column("Site_Url", "string")
                .with_column("Source_Name", "string")
                .with_column("SourceFileExtension", "string")
                .with_column("SourceFileName", "string")
                .with_column("SourceRecordId", "string")
                .with_column("SourceRelativeUrl", "string")
                .with_column("SourceSystem", "string")
                .with_column("Start_Time", "datetime")
                .with_column("TabType", "string")
                .with_column("TargetContextId", "string")
                .with_column("TargetUserId", "string")
                .with_column("TargetUserOrGroupName", "string")
                .with_column("TargetUserOrGroupType", "string")
                .with_column("TeamGuid", "string")
                .with_column("TeamName", "string")
                .with_column("Type", "string")
                .with_column("UniqueSharingId", "string")
                .with_column("UserAgent", "string")
                .with_column("UserDomain", "string")
                .with_column("UserId", "string")
                .with_column("UserKey", "string")
                .with_column("UserSharedWith", "string")
                .with_column("UserType", "string")
                .with_column("_ResourceId", "string")
                .with_column("_SubscriptionId", "string")
        );

        // ThreatIntelligenceIndicator
        self.add_table(
            TableInfo::canonical("ThreatIntelligenceIndicator")
                .source("sentinel")
                .description("Threat Intelligence Indicator")
                .column(time_col.clone())
                .column(tenant_col.clone())
                .with_column("Action", "string")
                .with_column("Active", "bool")
                .with_column("ActivityGroupNames", "string")
                .with_column("AdditionalInformation", "string")
                .with_column("ConfidenceScore", "real")
                .with_column("Description", "string")
                .with_column("DiamondModel", "string")
                .with_column("DomainName", "string")
                .with_column("EmailEncoding", "string")
                .with_column("EmailLanguage", "string")
                .with_column("EmailRecipient", "string")
                .with_column("EmailSenderAddress", "string")
                .with_column("EmailSenderName", "string")
                .with_column("EmailSourceDomain", "string")
                .with_column("EmailSourceIpAddress", "string")
                .with_column("EmailSubject", "string")
                .with_column("EmailXMailer", "string")
                .with_column("ExpirationDateTime", "datetime")
                .with_column("ExternalIndicatorId", "string")
                .with_column("FileCompileDateTime", "datetime")
                .with_column("FileCreatedDateTime", "datetime")
                .with_column("FileHashType", "string")
                .with_column("FileHashValue", "string")
                .with_column("FileMutexName", "string")
                .with_column("FileName", "string")
                .with_column("FilePacker", "string")
                .with_column("FilePath", "string")
                .with_column("FileSize", "int")
                .with_column("FileType", "string")
                .with_column("IndicatorId", "string")
                .with_column("IndicatorProvider", "string")
                .with_column("KillChainActions", "bool")
                .with_column("KillChainC2", "bool")
                .with_column("KillChainDelivery", "bool")
                .with_column("KillChainExploitation", "bool")
                .with_column("KillChainReconnaissance", "bool")
                .with_column("KillChainWeaponization", "bool")
                .with_column("KnownFalsePositives", "string")
                .with_column("MalwareNames", "string")
                .with_column("NetworkCidrBlock", "string")
                .with_column("NetworkDestinationAsn", "int")
                .with_column("NetworkDestinationCidrBlock", "string")
                .with_column("NetworkDestinationIP", "string")
                .with_column("NetworkDestinationPort", "int")
                .with_column("NetworkIP", "string")
                .with_column("NetworkPort", "int")
                .with_column("NetworkProtocol", "int")
                .with_column("NetworkSourceAsn", "int")
                .with_column("NetworkSourceCidrBlock", "string")
                .with_column("NetworkSourceIP", "string")
                .with_column("NetworkSourcePort", "int")
                .with_column("PassiveOnly", "bool")
                .with_column("SourceSystem", "string")
                .with_column("Tags", "string")
                .with_column("ThreatSeverity", "int")
                .with_column("ThreatType", "string")
                .with_column("TrafficLightProtocolLevel", "string")
                .with_column("Type", "string")
                .with_column("Url", "string")
                .with_column("UserAgent", "string")
        );

        // SecurityAlert - Alerts from security products
        self.add_table(
            TableInfo::canonical("SecurityAlert")
                .source("sentinel")
                .description("Alerts that have been generated by security products")
                .column(time_col.clone())
                .with_column("AlertLink", "string")
                .with_column("AlertName", "string")
                .with_column("AlertSeverity", "string")
                .with_column("AlertType", "string")
                .with_column("CompromisedEntity", "string")
                .with_column("ConfidenceLevel", "string")
                .with_column("ConfidenceScore", "real")
                .with_column("Description", "string")
                .with_column("DisplayName", "string")
                .with_column("EndTime", "datetime")
                .with_column("Entities", "string")
                .with_column("ExtendedLinks", "string")
                .with_column("ExtendedProperties", "string")
                .with_column("IsIncident", "bool")
                .with_column("ProcessingEndTime", "datetime")
                .with_column("ProductComponentName", "string")
                .with_column("ProductName", "string")
                .with_column("ProviderName", "string")
                .with_column("RemediationSteps", "string")
                .with_column("ResourceId", "string")
                .with_column("SourceComputerId", "string")
                .with_column("StartTime", "datetime")
                .with_column("Status", "string")
                .with_column("SubTechniques", "string")
                .with_column("SystemAlertId", "string")
                .with_column("Tactics", "string")
                .with_column("Techniques", "string")
                .with_column("Type", "string")
                .with_column("VendorName", "string")
                .with_column("VendorOriginalId", "string")
                .with_column("WorkspaceResourceGroup", "string")
                .with_column("WorkspaceSubscriptionId", "string")
        );

        // SecurityIncident - Incidents from security products
        self.add_table(
            TableInfo::canonical("SecurityIncident")
                .source("sentinel")
                .description("Incidents generated by security products")
                .column(time_col.clone())
                .column(tenant_col.clone())
                .with_column("AdditionalData", "dynamic")
                .with_column("AlertIds", "dynamic")
                .with_column("BookmarkIds", "dynamic")
                .with_column("Classification", "string")
                .with_column("ClassificationComment", "string")
                .with_column("ClassificationReason", "string")
                .with_column("ClosedTime", "datetime")
                .with_column("Comments", "dynamic")
                .with_column("CreatedTime", "datetime")
                .with_column("Description", "string")
                .with_column("FirstActivityTime", "datetime")
                .with_column("FirstModifiedTime", "datetime")
                .with_column("IncidentName", "string")
                .with_column("IncidentNumber", "int")
                .with_column("IncidentUrl", "string")
                .with_column("Labels", "dynamic")
                .with_column("LastActivityTime", "datetime")
                .with_column("LastModifiedTime", "datetime")
                .with_column("ModifiedBy", "string")
                .with_column("Owner", "dynamic")
                .with_column("ProviderIncidentId", "string")
                .with_column("ProviderName", "string")
                .with_column("RelatedAnalyticRuleIds", "dynamic")
                .with_column("Severity", "string")
                .with_column("SourceSystem", "string")
                .with_column("Status", "string")
                .with_column("Tasks", "dynamic")
                .with_column("Title", "string")
                .with_column("Type", "string")
        );

        // DeviceEvents - Microsoft Defender for Endpoints events
        self.add_table(
            TableInfo::canonical("DeviceEvents")
                .source("sentinel")
                .description("Microsoft Defender for Endpoints device events")
                .column(time_col.clone())
                .column(tenant_col.clone())
                .with_column("AccountDomain", "string")
                .with_column("AccountName", "string")
                .with_column("AccountSid", "string")
                .with_column("ActionType", "string")
                .with_column("AdditionalFields", "dynamic")
                .with_column("AppGuardContainerId", "string")
                .with_column("CreatedProcessSessionId", "long")
                .with_column("DeviceId", "string")
                .with_column("DeviceName", "string")
                .with_column("FileName", "string")
                .with_column("FileOriginIP", "string")
                .with_column("FileOriginUrl", "string")
                .with_column("FileSize", "long")
                .with_column("FolderPath", "string")
                .with_column("InitiatingProcessAccountDomain", "string")
                .with_column("InitiatingProcessAccountName", "string")
                .with_column("InitiatingProcessAccountObjectId", "string")
                .with_column("InitiatingProcessAccountSid", "string")
                .with_column("InitiatingProcessAccountUpn", "string")
                .with_column("InitiatingProcessCommandLine", "string")
                .with_column("InitiatingProcessCreationTime", "datetime")
                .with_column("InitiatingProcessFileName", "string")
                .with_column("InitiatingProcessFileSize", "long")
                .with_column("InitiatingProcessFolderPath", "string")
                .with_column("InitiatingProcessId", "long")
                .with_column("InitiatingProcessLogonId", "long")
                .with_column("InitiatingProcessMD5", "string")
                .with_column("InitiatingProcessParentCreationTime", "datetime")
                .with_column("InitiatingProcessParentFileName", "string")
                .with_column("InitiatingProcessParentId", "long")
                .with_column("InitiatingProcessRemoteSessionDeviceName", "string")
                .with_column("InitiatingProcessRemoteSessionIP", "string")
                .with_column("InitiatingProcessSessionId", "long")
                .with_column("InitiatingProcessSHA1", "string")
                .with_column("InitiatingProcessSHA256", "string")
                .with_column("InitiatingProcessUniqueId", "string")
                .with_column("InitiatingProcessVersionInfoCompanyName", "string")
                .with_column("InitiatingProcessVersionInfoFileDescription", "string")
                .with_column("InitiatingProcessVersionInfoInternalFileName", "string")
                .with_column("InitiatingProcessVersionInfoOriginalFileName", "string")
                .with_column("InitiatingProcessVersionInfoProductName", "string")
                .with_column("InitiatingProcessVersionInfoProductVersion", "string")
                .with_column("IsInitiatingProcessRemoteSession", "bool")
                .with_column("IsProcessRemoteSession", "bool")
                .with_column("LocalIP", "string")
                .with_column("LocalPort", "int")
                .with_column("LogonId", "long")
                .with_column("MachineGroup", "string")
                .with_column("MD5", "string")
                .with_column("ProcessCommandLine", "string")
                .with_column("ProcessCreationTime", "datetime")
                .with_column("ProcessId", "long")
                .with_column("ProcessRemoteSessionDeviceName", "string")
                .with_column("ProcessRemoteSessionIP", "string")
                .with_column("ProcessTokenElevation", "string")
                .with_column("RegistryKey", "string")
                .with_column("RegistryValueData", "string")
                .with_column("RegistryValueName", "string")
                .with_column("RemoteDeviceName", "string")
                .with_column("RemoteIP", "string")
                .with_column("RemotePort", "int")
                .with_column("RemoteUrl", "string")
                .with_column("ReportId", "long")
                .with_column("SHA1", "string")
                .with_column("SHA256", "string")
                .with_column("SourceSystem", "string")
                .with_column("Type", "string")
        );

        // DeviceProcessEvents - MDE process events
        self.add_table(
            TableInfo::canonical("DeviceProcessEvents")
                .source("sentinel")
                .description("Microsoft Defender for Endpoints device process events")
                .column(time_col.clone())
                .column(tenant_col.clone())
                .with_column("AccountDomain", "string")
                .with_column("AccountName", "string")
                .with_column("AccountObjectId", "string")
                .with_column("AccountSid", "string")
                .with_column("AccountUpn", "string")
                .with_column("ActionType", "string")
                .with_column("AdditionalFields", "dynamic")
                .with_column("AppGuardContainerId", "string")
                .with_column("CreatedProcessSessionId", "long")
                .with_column("DeviceId", "string")
                .with_column("DeviceName", "string")
                .with_column("FileName", "string")
                .with_column("FileSize", "long")
                .with_column("FolderPath", "string")
                .with_column("InitiatingProcessAccountDomain", "string")
                .with_column("InitiatingProcessAccountName", "string")
                .with_column("InitiatingProcessAccountObjectId", "string")
                .with_column("InitiatingProcessAccountSid", "string")
                .with_column("InitiatingProcessAccountUpn", "string")
                .with_column("InitiatingProcessCommandLine", "string")
                .with_column("InitiatingProcessCreationTime", "datetime")
                .with_column("InitiatingProcessFileName", "string")
                .with_column("InitiatingProcessFileSize", "long")
                .with_column("InitiatingProcessFolderPath", "string")
                .with_column("InitiatingProcessId", "long")
                .with_column("InitiatingProcessIntegrityLevel", "string")
                .with_column("InitiatingProcessLogonId", "long")
                .with_column("InitiatingProcessMD5", "string")
                .with_column("InitiatingProcessParentCreationTime", "datetime")
                .with_column("InitiatingProcessParentFileName", "string")
                .with_column("InitiatingProcessParentId", "long")
                .with_column("InitiatingProcessRemoteSessionDeviceName", "string")
                .with_column("InitiatingProcessRemoteSessionIP", "string")
                .with_column("InitiatingProcessSessionId", "long")
                .with_column("InitiatingProcessSHA1", "string")
                .with_column("InitiatingProcessSHA256", "string")
                .with_column("InitiatingProcessSignatureStatus", "string")
                .with_column("InitiatingProcessSignerType", "string")
                .with_column("InitiatingProcessTokenElevation", "string")
                .with_column("InitiatingProcessUniqueId", "string")
                .with_column("InitiatingProcessVersionInfoCompanyName", "string")
                .with_column("InitiatingProcessVersionInfoFileDescription", "string")
                .with_column("InitiatingProcessVersionInfoInternalFileName", "string")
                .with_column("InitiatingProcessVersionInfoOriginalFileName", "string")
                .with_column("InitiatingProcessVersionInfoProductName", "string")
                .with_column("InitiatingProcessVersionInfoProductVersion", "string")
                .with_column("IsInitiatingProcessRemoteSession", "bool")
                .with_column("IsProcessRemoteSession", "bool")
                .with_column("LogonId", "long")
                .with_column("MachineGroup", "string")
                .with_column("MD5", "string")
                .with_column("ProcessCommandLine", "string")
                .with_column("ProcessCreationTime", "datetime")
                .with_column("ProcessId", "long")
                .with_column("ProcessIntegrityLevel", "string")
                .with_column("ProcessRemoteSessionDeviceName", "string")
                .with_column("ProcessRemoteSessionIP", "string")
                .with_column("ProcessTokenElevation", "string")
                .with_column("ProcessUniqueId", "string")
                .with_column("ProcessVersionInfoCompanyName", "string")
                .with_column("ProcessVersionInfoFileDescription", "string")
                .with_column("ProcessVersionInfoInternalFileName", "string")
                .with_column("ProcessVersionInfoOriginalFileName", "string")
                .with_column("ProcessVersionInfoProductName", "string")
                .with_column("ProcessVersionInfoProductVersion", "string")
                .with_column("ReportId", "long")
                .with_column("SHA1", "string")
                .with_column("SHA256", "string")
                .with_column("SourceSystem", "string")
                .with_column("Type", "string")
        );

        // DeviceNetworkEvents - MDE network events
        self.add_table(
            TableInfo::canonical("DeviceNetworkEvents")
                .source("sentinel")
                .description("Microsoft Defender for Endpoints device network events")
                .column(time_col.clone())
                .column(tenant_col.clone())
                .with_column("ActionType", "string")
                .with_column("AdditionalFields", "dynamic")
                .with_column("AppGuardContainerId", "string")
                .with_column("DeviceId", "string")
                .with_column("DeviceName", "string")
                .with_column("InitiatingProcessAccountDomain", "string")
                .with_column("InitiatingProcessAccountName", "string")
                .with_column("InitiatingProcessAccountObjectId", "string")
                .with_column("InitiatingProcessAccountSid", "string")
                .with_column("InitiatingProcessAccountUpn", "string")
                .with_column("InitiatingProcessCommandLine", "string")
                .with_column("InitiatingProcessCreationTime", "datetime")
                .with_column("InitiatingProcessFileName", "string")
                .with_column("InitiatingProcessFileSize", "long")
                .with_column("InitiatingProcessFolderPath", "string")
                .with_column("InitiatingProcessId", "long")
                .with_column("InitiatingProcessIntegrityLevel", "string")
                .with_column("InitiatingProcessMD5", "string")
                .with_column("InitiatingProcessParentCreationTime", "datetime")
                .with_column("InitiatingProcessParentFileName", "string")
                .with_column("InitiatingProcessParentId", "long")
                .with_column("InitiatingProcessRemoteSessionDeviceName", "string")
                .with_column("InitiatingProcessRemoteSessionIP", "string")
                .with_column("InitiatingProcessSessionId", "long")
                .with_column("InitiatingProcessSHA1", "string")
                .with_column("InitiatingProcessSHA256", "string")
                .with_column("InitiatingProcessTokenElevation", "string")
                .with_column("InitiatingProcessUniqueId", "string")
                .with_column("InitiatingProcessVersionInfoCompanyName", "string")
                .with_column("InitiatingProcessVersionInfoFileDescription", "string")
                .with_column("InitiatingProcessVersionInfoInternalFileName", "string")
                .with_column("InitiatingProcessVersionInfoOriginalFileName", "string")
                .with_column("InitiatingProcessVersionInfoProductName", "string")
                .with_column("InitiatingProcessVersionInfoProductVersion", "string")
                .with_column("IsInitiatingProcessRemoteSession", "bool")
                .with_column("LocalIP", "string")
                .with_column("LocalIPType", "string")
                .with_column("LocalPort", "int")
                .with_column("MachineGroup", "string")
                .with_column("Protocol", "string")
                .with_column("RemoteIP", "string")
                .with_column("RemoteIPType", "string")
                .with_column("RemotePort", "int")
                .with_column("RemoteUrl", "string")
                .with_column("ReportId", "long")
                .with_column("SourceSystem", "string")
                .with_column("Type", "string")
        );

        // DeviceFileEvents - MDE file events
        self.add_table(
            TableInfo::canonical("DeviceFileEvents")
                .source("sentinel")
                .description("Microsoft Defender for Endpoints device file events")
                .column(time_col.clone())
                .column(tenant_col.clone())
                .with_column("ActionType", "string")
                .with_column("AdditionalFields", "dynamic")
                .with_column("AppGuardContainerId", "string")
                .with_column("DeviceId", "string")
                .with_column("DeviceName", "string")
                .with_column("FileName", "string")
                .with_column("FileOriginIP", "string")
                .with_column("FileOriginReferrerUrl", "string")
                .with_column("FileOriginUrl", "string")
                .with_column("FileSize", "long")
                .with_column("FolderPath", "string")
                .with_column("InitiatingProcessAccountDomain", "string")
                .with_column("InitiatingProcessAccountName", "string")
                .with_column("InitiatingProcessAccountObjectId", "string")
                .with_column("InitiatingProcessAccountSid", "string")
                .with_column("InitiatingProcessAccountUpn", "string")
                .with_column("InitiatingProcessCommandLine", "string")
                .with_column("InitiatingProcessCreationTime", "datetime")
                .with_column("InitiatingProcessFileName", "string")
                .with_column("InitiatingProcessFileSize", "long")
                .with_column("InitiatingProcessFolderPath", "string")
                .with_column("InitiatingProcessId", "long")
                .with_column("InitiatingProcessIntegrityLevel", "string")
                .with_column("InitiatingProcessMD5", "string")
                .with_column("InitiatingProcessParentCreationTime", "datetime")
                .with_column("InitiatingProcessParentFileName", "string")
                .with_column("InitiatingProcessParentId", "long")
                .with_column("InitiatingProcessRemoteSessionDeviceName", "string")
                .with_column("InitiatingProcessRemoteSessionIP", "string")
                .with_column("InitiatingProcessSessionId", "long")
                .with_column("InitiatingProcessSHA1", "string")
                .with_column("InitiatingProcessSHA256", "string")
                .with_column("InitiatingProcessTokenElevation", "string")
                .with_column("InitiatingProcessUniqueId", "string")
                .with_column("InitiatingProcessVersionInfoCompanyName", "string")
                .with_column("InitiatingProcessVersionInfoFileDescription", "string")
                .with_column("InitiatingProcessVersionInfoInternalFileName", "string")
                .with_column("InitiatingProcessVersionInfoOriginalFileName", "string")
                .with_column("InitiatingProcessVersionInfoProductName", "string")
                .with_column("InitiatingProcessVersionInfoProductVersion", "string")
                .with_column("IsAzureInfoProtectionApplied", "bool")
                .with_column("IsInitiatingProcessRemoteSession", "bool")
                .with_column("MachineGroup", "string")
                .with_column("MD5", "string")
                .with_column("PreviousFileName", "string")
                .with_column("PreviousFolderPath", "string")
                .with_column("ReportId", "long")
                .with_column("RequestAccountDomain", "string")
                .with_column("RequestAccountName", "string")
                .with_column("RequestAccountSid", "string")
                .with_column("RequestProtocol", "string")
                .with_column("RequestSourceIP", "string")
                .with_column("RequestSourcePort", "int")
                .with_column("SensitivityLabel", "string")
                .with_column("SensitivitySubLabel", "string")
                .with_column("SHA1", "string")
                .with_column("SHA256", "string")
                .with_column("ShareName", "string")
                .with_column("SourceSystem", "string")
                .with_column("Type", "string")
        );

        // EmailEvents - Office 365 email events
        self.add_table(
            TableInfo::canonical("EmailEvents")
                .source("sentinel")
                .description("Office 365 email events, including email delivery and blocking events")
                .column(time_col.clone())
                .column(tenant_col.clone())
                .with_column("AdditionalFields", "dynamic")
                .with_column("AttachmentCount", "int")
                .with_column("AuthenticationDetails", "string")
                .with_column("BulkComplaintLevel", "int")
                .with_column("Cc", "dynamic")
                .with_column("ConfidenceLevel", "string")
                .with_column("Connectors", "string")
                .with_column("Context", "string")
                .with_column("DeliveryAction", "string")
                .with_column("DeliveryLocation", "string")
                .with_column("DetectionMethods", "string")
                .with_column("DistributionList", "string")
                .with_column("EmailAction", "string")
                .with_column("EmailActionPolicy", "string")
                .with_column("EmailActionPolicyGuid", "string")
                .with_column("EmailClusterId", "long")
                .with_column("EmailDirection", "string")
                .with_column("EmailLanguage", "string")
                .with_column("EmailSize", "int")
                .with_column("ExchangeTransportRule", "string")
                .with_column("ForwardingInformation", "string")
                .with_column("InternetMessageId", "string")
                .with_column("IsFirstContact", "bool")
                .with_column("LatestDeliveryAction", "string")
                .with_column("LatestDeliveryLocation", "string")
                .with_column("NetworkMessageId", "string")
                .with_column("OrgLevelAction", "string")
                .with_column("OrgLevelPolicy", "string")
                .with_column("RecipientDomain", "string")
                .with_column("RecipientEmailAddress", "string")
                .with_column("RecipientObjectId", "string")
                .with_column("ReportId", "string")
                .with_column("SenderDisplayName", "string")
                .with_column("SenderFromAddress", "string")
                .with_column("SenderFromDomain", "string")
                .with_column("SenderIPv4", "string")
                .with_column("SenderIPv6", "string")
                .with_column("SenderMailFromAddress", "string")
                .with_column("SenderMailFromDomain", "string")
                .with_column("SenderObjectId", "string")
                .with_column("SourceSystem", "string")
                .with_column("Subject", "string")
                .with_column("ThreatClassification", "string")
                .with_column("ThreatNames", "string")
                .with_column("ThreatTypes", "string")
                .with_column("To", "dynamic")
                .with_column("Type", "string")
                .with_column("UrlCount", "int")
                .with_column("UserLevelAction", "string")
                .with_column("UserLevelPolicy", "string")
        );

        info!("Loaded {} default table schemas", self.data.tables.len());
        self.dirty = true;
    }

    /// Get statistics about the registry
    pub fn stats(&self) -> RegistryStats {
        let canonical_count = self.data.tables.values()
            .filter(|t| t.schema_type == SchemaType::Canonical)
            .count();
        let extensible_count = self.data.tables.values()
            .filter(|t| t.schema_type == SchemaType::Extensible)
            .count();
        let custom_count: usize = self.data.workspaces.values()
            .map(|ws| ws.custom_tables.len())
            .sum();

        RegistryStats {
            total_tables: self.data.tables.len(),
            canonical_tables: canonical_count,
            extensible_tables: extensible_count,
            custom_tables: custom_count,
            workspaces: self.data.workspaces.len(),
        }
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the schema registry
#[derive(Debug, Clone)]
pub struct RegistryStats {
    pub total_tables: usize,
    pub canonical_tables: usize,
    pub extensible_tables: usize,
    pub custom_tables: usize,
    pub workspaces: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_registry() {
        let registry = SchemaRegistry::new();
        assert_eq!(registry.data.tables.len(), 0);
        assert!(!registry.is_dirty());
    }

    #[test]
    fn test_with_defaults() {
        let registry = SchemaRegistry::with_defaults();
        assert!(registry.has_table("SecurityEvent"));
        assert!(registry.has_table("SigninLogs"));
        assert!(registry.has_table("AzureDiagnostics"));
        assert!(registry.is_dirty()); // Defaults mark as dirty
    }

    #[test]
    fn test_add_table() {
        let mut registry = SchemaRegistry::new();
        registry.add_table(
            TableInfo::canonical("TestTable")
                .with_column("Col1", "string")
        );

        assert!(registry.has_table("TestTable"));
        assert!(registry.is_dirty());

        let table = registry.get_table("TestTable").unwrap();
        assert!(table.has_column("Col1"));
    }

    #[test]
    fn test_workspace_operations() {
        let mut registry = SchemaRegistry::with_defaults();

        // Add table to workspace
        registry.add_workspace_table("ws-123", "SecurityEvent");
        assert!(registry.workspace_has_table("ws-123", "SecurityEvent"));
        assert!(!registry.workspace_has_table("ws-123", "NotATtable"));

        // Workspace that doesn't exist
        assert!(!registry.workspace_has_table("ws-999", "SecurityEvent"));
    }

    #[test]
    fn test_custom_tables() {
        let mut registry = SchemaRegistry::new();

        let custom = TableInfo::custom("MyData_CL")
            .with_column("CustomField", "string")
            .with_column("Value", "long");

        registry.add_custom_table("ws-123", custom);

        assert!(registry.workspace_has_table("ws-123", "MyData_CL"));

        let retrieved = registry.get_custom_table("ws-123", "MyData_CL").unwrap();
        assert_eq!(retrieved.columns.len(), 2);
    }

    #[test]
    fn test_table_extensions() {
        let mut registry = SchemaRegistry::with_defaults();

        // Add extensions to AzureDiagnostics for a workspace
        let extensions = vec![
            ColumnDef::string("CustomField1"),
            ColumnDef::long("CustomField2"),
        ];
        registry.add_table_extensions("ws-123", "AzureDiagnostics", extensions);

        // Get columns for workspace (should include extensions)
        let cols = registry.get_columns_for_workspace("AzureDiagnostics", "ws-123").unwrap();
        let col_names: Vec<_> = cols.iter().map(|c| c.name.as_str()).collect();
        assert!(col_names.contains(&"TimeGenerated")); // Base column
        assert!(col_names.contains(&"CustomField1")); // Extension
        assert!(col_names.contains(&"CustomField2")); // Extension
    }

    #[test]
    fn test_validation_schema_conversion() {
        let registry = SchemaRegistry::with_defaults();
        let schema = registry.to_validation_schema(None);

        assert!(!schema.is_empty());
        assert!(schema.get_table("SecurityEvent").is_some());
    }

    #[test]
    fn test_stats() {
        let mut registry = SchemaRegistry::with_defaults();
        registry.add_custom_table("ws-1", TableInfo::custom("Custom_CL"));

        let stats = registry.stats();
        assert!(stats.canonical_tables > 0);
        assert!(stats.extensible_tables > 0);
        assert_eq!(stats.custom_tables, 1);
        assert_eq!(stats.workspaces, 1);
    }
}
