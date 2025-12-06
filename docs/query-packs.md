# Query Packs

Query Packs are reusable KQL query definitions that can be version controlled, shared, and executed across multiple workspaces. Unlike Investigation Packs, query packs execute all queries independently in parallel.

## Overview

Query packs provide:

- **Reusability**: Define once, execute anywhere
- **Version control**: Track changes to queries over time
- **Collaboration**: Share hunting queries with your team
- **Automation**: Execute from CLI for scripting and CI/CD
- **AI integration**: Minimal YAML format ideal for AI-generated queries

## Quick Start

### Minimal Example

```yaml
name: "Failed Authentication Hunt"
query: |
  SecurityEvent
  | where EventID == 4625
  | where TimeGenerated > ago(24h)
  | summarize FailedAttempts=count() by Account, Computer
  | order by FailedAttempts desc
```

### Run from CLI

```bash
# Execute on all workspaces
kql-panopticon run-pack security/failed-auth.yaml

# Execute on specific workspaces
kql-panopticon run-pack security/failed-auth.yaml --workspaces ws-prod-01,ws-prod-02

# Validate without executing
kql-panopticon run-pack security/failed-auth.yaml --validate-only

# Output as JSON to stdout
kql-panopticon run-pack security/failed-auth.yaml --json
```

## File Format

Query packs use YAML (recommended) or JSON format. Files are stored in `~/.kql-panopticon/packs/`.

### Complete Schema

```yaml
# Required: Pack name (used in output file naming)
name: "Pack Name"

# Optional: Human-readable description
description: "What this pack does"

# Optional: Author information
author: "Security Team"

# Optional: Version for tracking changes
version: "1.0"

# Option A: Single query (simple packs)
query: |
  Table
  | where Condition
  | project Columns

# Option B: Multiple queries (complex packs)
queries:
  - name: "Query Name"
    description: "What this query finds"
    query: |
      Table
      | where Condition

# Optional: Execution settings
settings:
  export_csv: true
  export_json: false
  parse_dynamics: true

# Optional: Workspace targeting
workspaces:
  scope: all  # or "selected" or "pattern"
```

---

## Field Reference

### Root Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | - | Pack name, used in output file naming |
| `description` | string | No | - | Human-readable description |
| `author` | string | No | - | Author or team name |
| `version` | string | No | - | Version identifier |
| `query` | string | No* | - | Single query (use this OR `queries`) |
| `queries` | array | No* | - | Multiple queries (use this OR `query`) |
| `settings` | object | No | - | Execution settings |
| `workspaces` | object | No | - | Workspace targeting |

*Either `query` or `queries` is required, but not both.

### Query Object (for `queries` array)

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | - | Query name (used in output file naming) |
| `description` | string | No | - | What this query finds |
| `query` | string | Yes | - | The KQL query |

### Settings Object

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `export_csv` | boolean | No | `true` | Export results to CSV files |
| `export_json` | boolean | No | `false` | Export results to JSON files |
| `parse_dynamics` | boolean | No | `true` | Parse dynamic columns in JSON output |

### Workspace Scope

Three targeting modes are available:

#### All Workspaces (default)

```yaml
workspaces:
  scope: all
```

Executes on every workspace the user has access to.

#### Selected Workspaces

```yaml
workspaces:
  scope: selected
  ids:
    - "workspace-id-1"
    - "workspace-id-2"
    - "workspace-id-3"
```

Executes only on the specified workspace IDs.

#### Pattern Matching

```yaml
workspaces:
  scope: pattern
  pattern: "*-prod-*"
```

Executes on workspaces whose names match the glob pattern.

**Pattern syntax:**
- `*` matches any characters
- `?` matches single character
- Case-insensitive matching

**Examples:**
- `*-prod-*` matches `sentinel-prod-01`, `la-prod-east`
- `sentinel-*` matches `sentinel-dev`, `sentinel-prod`
- `*-security` matches `tenant1-security`, `corp-security`

---

## Validation Rules

| Rule | Error Message |
|------|---------------|
| Must have query or queries | `Query pack must contain either 'query' or 'queries' field` |
| Cannot have both | `Query pack cannot have both 'query' and 'queries' fields` |
| Queries array not empty | `Query pack 'queries' array cannot be empty` |

---

## Output Structure

Results are organized hierarchically:

```
output/
└── {subscription_name}/
    └── {workspace_name}/
        └── {timestamp}/
            ├── {job_name}_{query_name}.csv
            └── {job_name}_{query_name}.json
```

**Example:**
```
output/
└── production_subscription/
    └── sentinel-workspace/
        └── 2025-01-15_10-30-00/
            ├── security-hunt_failed-logins.csv
            ├── security-hunt_failed-logins.json
            ├── security-hunt_brute-force.csv
            └── security-hunt_brute-force.json
```

Names are normalized: lowercase, alphanumeric characters, hyphens, and underscores only.

---

## CLI Reference

### Run Pack

```bash
kql-panopticon run-pack <pack> [OPTIONS]

Arguments:
  <pack>  Path to query pack file (.yaml, .yml, or .json)
          Can be absolute path or relative to ~/.kql-panopticon/packs/

Options:
  -w, --workspaces <LIST>      Override workspace selection (comma-separated IDs or 'all')
  -f, --format <FORMAT>        Output format [default: files] [values: files, stdout]
      --json                   Print results to stdout as JSON
      --validate-only          Validate pack without executing
  -h, --help                   Print help
```

### Export Session as Pack

```bash
kql-panopticon export-pack <session> [OPTIONS]

Arguments:
  <session>  Session name to export

Options:
  -o, --output <PATH>    Output path (default: ~/.kql-panopticon/packs/<session>.yaml)
  -f, --format <FORMAT>  Output format [default: yaml] [values: yaml, json]
  -h, --help             Print help
```

### Examples

```bash
# Basic execution
kql-panopticon run-pack threat-hunt.yaml

# Override workspace targeting
kql-panopticon run-pack audit.yaml --workspaces all
kql-panopticon run-pack audit.yaml --workspaces ws-prod-01,ws-prod-02

# Validate pack syntax
kql-panopticon run-pack new-pack.yaml --validate-only

# JSON output for piping to other tools
kql-panopticon run-pack quick-check.yaml --json | jq '.results'

# Export refined session back to pack
kql-panopticon export-pack my-investigation --output security/refined-hunt.yaml
```

---

## TUI Usage

### Packs Tab (Key: 6)

| Key | Action |
|-----|--------|
| `Up/Down` | Navigate pack list |
| `Enter` | Load first query into Query editor |
| `e` | Execute entire pack on selected workspaces |
| `r` | Refresh pack list from disk |

### Workflow

1. Press `2` to select target workspaces
2. Press `6` to open Packs tab
3. Navigate to desired pack with `Up/Down`
4. Press `e` to execute all queries
5. Press `4` to monitor jobs
6. Results saved to output folder

### Sessions Tab - Export as Pack (Key: 5)

1. Refine queries in Query tab
2. Execute and review results
3. Press `5` for Sessions tab
4. Press `p` to export current session as pack
5. Pack saved to `~/.kql-panopticon/packs/`

---

## Examples

### Security Baseline Assessment

Multi-query pack for security posture assessment.

```yaml
name: "Security Baseline Assessment"
description: "Comprehensive security posture check across key areas"
author: "Security Team"
version: "2.0"

queries:
  - name: "admin-logins"
    description: "Administrative account login activity"
    query: |
      SigninLogs
      | where TimeGenerated > ago(7d)
      | where UserPrincipalName has_any ("admin", "administrator", "svc-")
      | summarize
          TotalLogins=count(),
          UniqueIPs=dcount(IPAddress),
          FailedLogins=countif(ResultType != 0)
        by UserPrincipalName
      | order by TotalLogins desc

  - name: "mfa-gaps"
    description: "Sign-ins without MFA"
    query: |
      SigninLogs
      | where TimeGenerated > ago(7d)
      | where AuthenticationRequirement != "multiFactorAuthentication"
      | where ResultType == 0
      | summarize
          NonMfaLogins=count(),
          Apps=make_set(AppDisplayName)
        by UserPrincipalName
      | where NonMfaLogins > 10
      | order by NonMfaLogins desc

  - name: "risky-sign-ins"
    description: "High-risk sign-in events"
    query: |
      SigninLogs
      | where TimeGenerated > ago(7d)
      | where RiskLevelDuringSignIn in ("high", "medium")
      | project
          TimeGenerated,
          UserPrincipalName,
          IPAddress,
          Location,
          RiskLevelDuringSignIn,
          RiskDetail
      | order by TimeGenerated desc

  - name: "stale-accounts"
    description: "Accounts with no recent activity"
    query: |
      SigninLogs
      | where TimeGenerated > ago(90d)
      | summarize LastLogin=max(TimeGenerated) by UserPrincipalName
      | where LastLogin < ago(30d)
      | order by LastLogin asc

settings:
  export_csv: true
  export_json: true

workspaces:
  scope: all
```

---

### Threat Hunting - Persistence Mechanisms

Hunt for common persistence techniques.

```yaml
name: "Persistence Hunt"
description: "Detect common persistence mechanisms used by attackers"
author: "Threat Hunting Team"
version: "1.0"

queries:
  - name: "scheduled-tasks"
    description: "Suspicious scheduled task creation"
    query: |
      SecurityEvent
      | where TimeGenerated > ago(24h)
      | where EventID == 4698
      | extend TaskContent = tostring(EventData)
      | where TaskContent contains "powershell"
         or TaskContent contains "cmd.exe"
         or TaskContent contains "wscript"
         or TaskContent contains "cscript"
      | project TimeGenerated, Computer, Account, TaskContent

  - name: "registry-run-keys"
    description: "Registry run key modifications"
    query: |
      DeviceRegistryEvents
      | where TimeGenerated > ago(24h)
      | where RegistryKey has_any (
          @"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
          @"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce",
          @"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run"
        )
      | where ActionType in ("RegistryValueSet", "RegistryKeyCreated")
      | project
          TimeGenerated,
          DeviceName,
          RegistryKey,
          RegistryValueName,
          RegistryValueData,
          InitiatingProcessFileName

  - name: "new-services"
    description: "New service installations"
    query: |
      SecurityEvent
      | where TimeGenerated > ago(24h)
      | where EventID == 7045
      | extend
          ServiceName = tostring(EventData.ServiceName),
          ServicePath = tostring(EventData.ImagePath)
      | where ServicePath !startswith "C:\\Windows\\"
      | project TimeGenerated, Computer, ServiceName, ServicePath, Account

  - name: "startup-folder"
    description: "Files added to startup folders"
    query: |
      DeviceFileEvents
      | where TimeGenerated > ago(24h)
      | where FolderPath has_any (
          "\\Start Menu\\Programs\\Startup",
          "\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup"
        )
      | where ActionType == "FileCreated"
      | project
          TimeGenerated,
          DeviceName,
          FileName,
          FolderPath,
          InitiatingProcessFileName,
          InitiatingProcessAccountName

settings:
  export_csv: true
  export_json: true
```

---

### Compliance Audit - Data Access

Audit data access patterns for compliance.

```yaml
name: "Data Access Audit"
description: "Audit file and data access for compliance reporting"
author: "Compliance Team"
version: "1.0"

queries:
  - name: "sensitive-file-access"
    description: "Access to files in sensitive locations"
    query: |
      OfficeActivity
      | where TimeGenerated > ago(30d)
      | where Operation in ("FileAccessed", "FileDownloaded", "FilePreviewed")
      | where SourceFileName has_any (".xlsx", ".csv", ".pdf", ".docx")
      | where SourceRelativeUrl has_any ("confidential", "hr", "finance", "legal")
      | summarize
          AccessCount=count(),
          Operations=make_set(Operation),
          FirstAccess=min(TimeGenerated),
          LastAccess=max(TimeGenerated)
        by UserId, SourceFileName
      | order by AccessCount desc

  - name: "bulk-downloads"
    description: "Users with high download volumes"
    query: |
      OfficeActivity
      | where TimeGenerated > ago(7d)
      | where Operation in ("FileDownloaded", "FileSyncDownloadedFull")
      | summarize
          DownloadCount=count(),
          UniqueFiles=dcount(SourceFileName),
          TotalSize=sum(FileSize)
        by UserId, bin(TimeGenerated, 1d)
      | where DownloadCount > 100 or TotalSize > 500000000
      | order by TotalSize desc

  - name: "external-sharing"
    description: "Files shared externally"
    query: |
      OfficeActivity
      | where TimeGenerated > ago(30d)
      | where Operation in ("SharingSet", "AnonymousLinkCreated", "SecureLinkCreated")
      | where TargetUserOrGroupType == "Guest"
         or Operation == "AnonymousLinkCreated"
      | project
          TimeGenerated,
          UserId,
          SourceFileName,
          Operation,
          TargetUserOrGroupName
      | order by TimeGenerated desc

  - name: "after-hours-access"
    description: "Data access outside business hours"
    query: |
      OfficeActivity
      | where TimeGenerated > ago(14d)
      | where Operation in ("FileAccessed", "FileDownloaded", "FileModified")
      | extend Hour = datetime_part("hour", TimeGenerated)
      | extend DayOfWeek = dayofweek(TimeGenerated)
      | where Hour < 6 or Hour > 20 or DayOfWeek in (0d, 6d)
      | summarize
          AfterHoursEvents=count(),
          UniqueFiles=dcount(SourceFileName)
        by UserId
      | where AfterHoursEvents > 20
      | order by AfterHoursEvents desc

workspaces:
  scope: pattern
  pattern: "*-compliance-*"
```

---

### Incident Response - Initial Triage

Quick triage queries for incident response.

```yaml
name: "Incident Triage Pack"
description: "Initial triage queries for rapid incident assessment"
author: "Incident Response Team"
version: "1.2"

queries:
  - name: "recent-alerts"
    description: "Security alerts in last 24 hours"
    query: |
      SecurityAlert
      | where TimeGenerated > ago(24h)
      | summarize
          AlertCount=count(),
          Severities=make_set(AlertSeverity)
        by AlertName, ProviderName
      | order by AlertCount desc

  - name: "endpoint-detections"
    description: "Defender detections"
    query: |
      DeviceEvents
      | where TimeGenerated > ago(24h)
      | where ActionType has "Detection"
      | project
          TimeGenerated,
          DeviceName,
          ActionType,
          FileName,
          FolderPath,
          InitiatingProcessAccountName
      | order by TimeGenerated desc

  - name: "failed-logins-spike"
    description: "Accounts with login failures"
    query: |
      SigninLogs
      | where TimeGenerated > ago(4h)
      | where ResultType != 0
      | summarize
          FailedAttempts=count(),
          UniqueIPs=dcount(IPAddress),
          Apps=make_set(AppDisplayName)
        by UserPrincipalName
      | where FailedAttempts > 5
      | order by FailedAttempts desc

  - name: "unusual-processes"
    description: "Processes from unusual locations"
    query: |
      DeviceProcessEvents
      | where TimeGenerated > ago(24h)
      | where FolderPath !startswith "C:\\Windows"
        and FolderPath !startswith "C:\\Program Files"
        and FolderPath !startswith "C:\\Program Files (x86)"
      | where FileName endswith ".exe"
      | summarize
          ExecutionCount=count(),
          Devices=make_set(DeviceName)
        by FileName, FolderPath
      | where ExecutionCount < 5
      | order by ExecutionCount asc

settings:
  export_csv: true
  export_json: true

workspaces:
  scope: all
```

---

### Network Monitoring

Network traffic analysis queries.

```yaml
name: "Network Analysis Pack"
description: "Network traffic monitoring and anomaly detection"
version: "1.0"

queries:
  - name: "external-connections"
    description: "High-volume external connections"
    query: |
      DeviceNetworkEvents
      | where TimeGenerated > ago(24h)
      | where RemoteIPType == "Public"
      | summarize
          ConnectionCount=count(),
          BytesSent=sum(SentBytes),
          BytesReceived=sum(ReceivedBytes)
        by DeviceName, RemoteIP, RemotePort
      | where BytesSent > 100000000
      | order by BytesSent desc

  - name: "rare-ports"
    description: "Connections on uncommon ports"
    query: |
      DeviceNetworkEvents
      | where TimeGenerated > ago(24h)
      | where RemotePort !in (80, 443, 22, 53, 445, 139, 3389, 25, 587, 993, 995)
      | where RemoteIPType == "Public"
      | summarize
          Connections=count(),
          UniqueDevices=dcount(DeviceName)
        by RemotePort
      | where UniqueDevices < 3
      | order by Connections desc

  - name: "dns-queries"
    description: "Unusual DNS query patterns"
    query: |
      DnsEvents
      | where TimeGenerated > ago(24h)
      | where QueryType in ("A", "AAAA", "TXT")
      | extend DomainParts = split(Name, ".")
      | extend TLD = tostring(DomainParts[-1])
      | extend Domain = strcat(tostring(DomainParts[-2]), ".", TLD)
      | summarize
          QueryCount=count(),
          UniqueSubdomains=dcount(Name),
          Clients=make_set(ClientIP)
        by Domain
      | where UniqueSubdomains > 50
      | order by UniqueSubdomains desc

  - name: "beaconing"
    description: "Potential C2 beaconing patterns"
    query: |
      DeviceNetworkEvents
      | where TimeGenerated > ago(24h)
      | where RemoteIPType == "Public"
      | summarize
          ConnectionTimes=make_list(TimeGenerated),
          ConnectionCount=count()
        by DeviceName, RemoteIP
      | where ConnectionCount > 20
      | extend Intervals = array_sort_asc(ConnectionTimes)
      | mv-apply Intervals on (
          extend NextTime = next(Intervals)
          | extend Interval = datetime_diff('second', NextTime, Intervals)
          | summarize AvgInterval=avg(Interval), StdDev=stdev(Interval)
        )
      | where StdDev < 60 and AvgInterval between (60 .. 3600)
      | project DeviceName, RemoteIP, ConnectionCount, AvgInterval, StdDev
```

---

## Best Practices

### Query Design

1. **Always include time bounds**: Use `TimeGenerated > ago(Xd)` to limit data scanned
2. **Project early**: Select only needed columns to reduce data transfer
3. **Summarize when possible**: Aggregate data to reduce result size
4. **Order results**: Use `order by` for consistent, useful output

### Pack Organization

```
~/.kql-panopticon/packs/
├── security/
│   ├── threat-hunting.yaml
│   ├── persistence.yaml
│   └── lateral-movement.yaml
├── compliance/
│   ├── data-access.yaml
│   └── audit-logs.yaml
├── incident-response/
│   ├── triage.yaml
│   └── forensics.yaml
└── monitoring/
    ├── network.yaml
    └── endpoints.yaml
```

### Version Control

1. Store packs in git repository
2. Use semantic versioning (`version: "1.0"`)
3. Include author and description for documentation
4. Review changes before deployment

---

## Troubleshooting

### Common Issues

**"Query pack must contain either 'query' or 'queries' field"**
- Pack is missing both `query` and `queries` fields
- Add at least one query

**"Query pack cannot have both 'query' and 'queries' fields"**
- Pack has both single and multi-query format
- Use `query` for single queries, `queries` for multiple

**"Query pack 'queries' array cannot be empty"**
- The `queries` array exists but has no entries
- Add at least one query object or switch to single `query` format

**Pack not appearing in TUI**
- Ensure file is in `~/.kql-panopticon/packs/` or subdirectory
- Check file extension is `.yaml`, `.yml`, or `.json`
- Press `r` to refresh the pack list

**Results not saved**
- Check `settings.export_csv` and `settings.export_json` values
- Verify output folder permissions
- Review job status in Jobs tab

---

## Integration with AI Assistants

When generating query packs with AI:

1. Provide this documentation as context
2. Specify the target log sources (Sentinel tables, MDE, etc.)
3. Describe the security scenario or compliance requirement
4. Request complete, valid YAML output

### Prompt Template

```
Generate a KQL query pack for [SECURITY SCENARIO].

Requirements:
- Use YAML format with 'name' at the root
- Include 'description' and 'version' fields
- For multiple queries, use 'queries' array with 'name', 'description', and 'query' for each
- Include appropriate time bounds in all queries (ago(Xd))
- Target these tables: [TABLE_LIST]
- Add 'settings' with export_csv: true

Output complete, valid YAML that passes validation.
```

### Example Prompt

```
Generate a KQL query pack for detecting credential theft techniques.

Requirements:
- Use YAML format with 'name' at the root
- Include 'description' and 'version' fields
- Use 'queries' array for multiple detection queries
- Include appropriate time bounds (24h for real-time, 7d for historical)
- Target: SecurityEvent, DeviceProcessEvents, DeviceLogonEvents
- Add 'settings' with export_csv: true

Output complete, valid YAML that passes validation.
```
