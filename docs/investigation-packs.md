# Investigation Packs

Investigation Packs enable chained query execution where results from earlier steps feed into subsequent queries. This is essential for threat hunting workflows where you need to pivot from initial indicators through related entities.

## Overview

Unlike standard Query Packs (which execute independent queries in parallel), Investigation Packs:

- Execute steps in dependency order
- Extract values from query results
- Substitute extracted values into subsequent queries
- Maintain per-workspace isolation (values are never merged across workspaces)
- Support user-provided input variables
- Handle large arrays through automatic chunking

## Quick Start

### Minimal Example

```yaml
kind: investigation
name: "Simple IP Investigation"
steps:
  - name: find_connections
    query: |
      NetworkConnections
      | where RemoteIP == "10.0.0.1"
      | distinct SourceIP
    extract:
      source_ips:
        column: SourceIP
        type: array

  - name: related_events
    depends_on: [find_connections]
    query: |
      SecurityEvent
      | where IpAddress in ({{find_connections.source_ips}})
```

### Run from CLI

```bash
# Execute investigation
kql-panopticon run-investigation my-investigation.yaml

# With input variables
kql-panopticon run-investigation my-investigation.yaml --set target_ip=10.0.0.1

# Validate only (no execution)
kql-panopticon run-investigation my-investigation.yaml --validate-only

# Target specific workspaces
kql-panopticon run-investigation my-investigation.yaml --workspaces ws-prod-01,ws-prod-02
```

## File Format

Investigation packs use YAML (recommended) or JSON format. Files are stored in `~/.kql-panopticon/investigations/`.

### Complete Schema

```yaml
# Required: Must be "investigation" to distinguish from query packs
kind: investigation

# Required: Investigation name (used in output folder naming)
name: "Investigation Name"

# Optional: Human-readable description
description: "What this investigation does"

# Optional: Version for tracking changes
version: "1.0"

# Optional: Output folder configuration
output:
  folder: "./investigations/{{name}}/{{timestamp}}"

# Optional: User-provided input variables
inputs:
  - name: variable_name
    description: "Description shown when prompting"
    type: string          # Currently only "string" supported
    required: true        # Default: true
    default: "value"      # Optional default value

# Required: Investigation steps (at least one)
steps:
  - name: step_name                    # Required: Unique identifier
    query: |                           # Required: KQL query
      Table | where Column == "value"
    depends_on:                        # Optional: Steps that must complete first
      - previous_step
    extract:                           # Optional: Values to extract from results
      variable_name:
        column: ColumnName             # Required: Column to extract from
        type: array                    # "array" (default) or "single"
        quote_style: single            # "single" (default), "double", or "verbatim"
        chunk_size: 500                # Optional: Max items per query chunk
        dedupe: true                   # Default: true (remove duplicates)
```

---

## Field Reference

### Root Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `kind` | string | Yes | - | Must be `"investigation"` |
| `name` | string | Yes | - | Investigation name, used in output paths |
| `description` | string | No | - | Human-readable description |
| `version` | string | No | - | Version identifier |
| `output` | object | No | - | Output configuration |
| `inputs` | array | No | `[]` | User-provided input variables |
| `steps` | array | Yes | - | Investigation steps (minimum 1) |

### Output Configuration

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `folder` | string | Yes | `"./investigations/{{name}}/{{timestamp}}"` | Output folder template |

**Template Variables:**
- `{{name}}` - Investigation name (normalized: lowercase, alphanumeric + hyphens)
- `{{timestamp}}` - Execution timestamp (format: `YYYY-MM-DD_HH-MM-SS`)

### Input Variables

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | - | Variable name (referenced as `{{inputs.name}}`) |
| `description` | string | No | - | Description shown when prompting user |
| `type` | string | No | `"string"` | Input type (only `"string"` currently) |
| `required` | boolean | No | `true` | Whether input must be provided |
| `default` | string | No | - | Default value if not provided |

### Steps

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | - | Unique step identifier |
| `query` | string | Yes | - | KQL query (may contain `{{variable}}` placeholders) |
| `depends_on` | array | No | `[]` | Step names that must complete first |
| `extract` | object | No | `{}` | Values to extract from results |

### Extract Configuration

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `column` | string | Yes | - | Column name to extract values from |
| `type` | string | No | `"array"` | `"array"` (all rows) or `"single"` (first row only) |
| `quote_style` | string | No | `"single"` | How to quote values in substitution |
| `chunk_size` | integer | No | `500` | Max items per query when chunking arrays |
| `dedupe` | boolean | No | `true` | Remove duplicate values |

### Quote Styles

| Style | Output Example | Escaping | Use Case |
|-------|---------------|----------|----------|
| `single` | `'value'` | `'` becomes `''` | Most KQL string comparisons |
| `double` | `"value"` | `"` becomes `\"`, `\` becomes `\\` | When single quotes in data |
| `verbatim` | `@'value'` | `'` becomes `''` | Paths, regex patterns |

**Array formatting:** Arrays are comma-separated: `'val1','val2','val3'`

---

## Variable References

Variables use double-brace syntax: `{{namespace.variable}}`

### Namespaces

| Namespace | Syntax | Description |
|-----------|--------|-------------|
| `inputs` | `{{inputs.variable_name}}` | User-provided input value |
| Step name | `{{step_name.variable_name}}` | Extracted value from a previous step |

### Rules

1. **Namespace required**: Bare variables like `{{value}}` are invalid
2. **Dependency declaration**: To reference `{{step_a.users}}`, you must include `step_a` in `depends_on`
3. **Extraction must exist**: Referenced variables must be defined in the source step's `extract` block

---

## Execution Model

### Dependency Resolution

Steps execute in topological order based on `depends_on` declarations:

```yaml
steps:
  - name: step_c          # Executes third
    depends_on: [step_a, step_b]

  - name: step_a          # Executes first (no dependencies)

  - name: step_b          # Executes second (depends on step_a)
    depends_on: [step_a]
```

Execution order: `step_a` → `step_b` → `step_c`

### Per-Workspace Isolation

Each workspace maintains its own context:

```
Workspace A: step1.users = ['alice', 'bob']
Workspace B: step1.users = ['charlie', 'dave']
```

When `step2` runs:
- On Workspace A: `{{step1.users}}` = `'alice','bob'`
- On Workspace B: `{{step1.users}}` = `'charlie','dave'`

Values are **never merged** across workspaces.

### Failure Handling

- **All-or-nothing per workspace**: If a step fails, subsequent dependent steps are skipped for that workspace
- **Independent workspaces**: Failure in Workspace A doesn't affect Workspace B
- **Automatic retry**: Queries retry with exponential backoff (configurable)

### Array Chunking

When extracted arrays exceed `chunk_size`:

1. Array is split into chunks
2. Query executes once per chunk
3. Results are merged
4. Extractions are combined and deduped

Example with `chunk_size: 100` and 250 users:
- Query 1: users 1-100
- Query 2: users 101-200
- Query 3: users 201-250
- Results merged into single output

---

## Output Structure

```
investigations/
└── {investigation_name}/
    └── {timestamp}/
        ├── manifest.json           # Execution metadata and status
        ├── inputs.json             # Input variables used
        └── {subscription}/
            └── {workspace}/
                └── {step_name}/
                    ├── results.csv     # Query results
                    ├── results.json    # Results with metadata
                    └── extracts.json   # Extracted values
```

### Manifest Format

```json
{
  "investigation_name": "Malicious URL Investigation",
  "pack_path": "/path/to/pack.yaml",
  "started_at": "2025-01-15T10:30:00Z",
  "completed_at": "2025-01-15T10:35:00Z",
  "status": "success",
  "output_folder": "./investigations/malicious_url/2025-01-15_10-30-00",
  "workspaces": {
    "workspace-id-1": {
      "status": "success",
      "steps": {
        "url_clicks": { "status": "success", "rows": 42, "duration_ms": 1234 },
        "affected_users": { "status": "success", "rows": 15, "duration_ms": 567 }
      }
    }
  }
}
```

---

## Examples

### Phishing URL Investigation

Investigate users who clicked a malicious URL and their subsequent email activity.

```yaml
kind: investigation
name: "Phishing URL Investigation"
description: "Trace impact of phishing URL clicks through email and authentication events"
version: "1.0"

inputs:
  - name: malicious_url
    description: "The phishing URL to investigate"
    required: true
  - name: lookback_days
    description: "Days to look back"
    default: "7"

steps:
  - name: url_clicks
    query: |
      UrlClickEvents
      | where TimeGenerated > ago({{inputs.lookback_days}}d)
      | where Url contains "{{inputs.malicious_url}}"
      | project TimeGenerated, UserPrincipalName, Url, NetworkMessageId
    extract:
      affected_users:
        column: UserPrincipalName
        type: array
        quote_style: single
        dedupe: true
      message_ids:
        column: NetworkMessageId
        type: array

  - name: user_emails
    depends_on: [url_clicks]
    query: |
      EmailEvents
      | where TimeGenerated > ago({{inputs.lookback_days}}d)
      | where RecipientEmailAddress in ({{url_clicks.affected_users}})
      | project TimeGenerated, SenderFromAddress, RecipientEmailAddress, Subject, NetworkMessageId
    extract:
      senders:
        column: SenderFromAddress
        type: array
        dedupe: true

  - name: user_sign_ins
    depends_on: [url_clicks]
    query: |
      SigninLogs
      | where TimeGenerated > ago({{inputs.lookback_days}}d)
      | where UserPrincipalName in ({{url_clicks.affected_users}})
      | project TimeGenerated, UserPrincipalName, AppDisplayName, IPAddress, ResultType, Location

  - name: sender_investigation
    depends_on: [user_emails]
    query: |
      EmailEvents
      | where TimeGenerated > ago(30d)
      | where SenderFromAddress in ({{user_emails.senders}})
      | summarize
          TotalEmails=count(),
          UniqueRecipients=dcount(RecipientEmailAddress),
          FirstSeen=min(TimeGenerated),
          LastSeen=max(TimeGenerated)
        by SenderFromAddress
```

**Run:**
```bash
kql-panopticon run-investigation phishing.yaml --set malicious_url=evil.com --set lookback_days=14
```

---

### Lateral Movement Detection

Track potential lateral movement from a compromised host.

```yaml
kind: investigation
name: "Lateral Movement Investigation"
description: "Investigate potential lateral movement from a source host"
version: "1.0"

inputs:
  - name: source_host
    description: "Hostname of potentially compromised machine"
    required: true
  - name: timeframe
    description: "Timeframe to investigate"
    default: "24h"

steps:
  - name: outbound_connections
    query: |
      DeviceNetworkEvents
      | where TimeGenerated > ago({{inputs.timeframe}})
      | where DeviceName =~ "{{inputs.source_host}}"
      | where ActionType == "ConnectionSuccess"
      | where RemoteIPType == "Private"
      | distinct RemoteIP, RemotePort
    extract:
      target_ips:
        column: RemoteIP
        type: array
        chunk_size: 200

  - name: auth_to_targets
    depends_on: [outbound_connections]
    query: |
      SecurityEvent
      | where TimeGenerated > ago({{inputs.timeframe}})
      | where EventID in (4624, 4625)
      | where IpAddress in ({{outbound_connections.target_ips}})
      | project TimeGenerated, Account, Computer, EventID, IpAddress, LogonType
    extract:
      target_hosts:
        column: Computer
        type: array
        dedupe: true

  - name: processes_on_targets
    depends_on: [auth_to_targets]
    query: |
      DeviceProcessEvents
      | where TimeGenerated > ago({{inputs.timeframe}})
      | where DeviceName in ({{auth_to_targets.target_hosts}})
      | where InitiatingProcessAccountName !in ("system", "local service", "network service")
      | project TimeGenerated, DeviceName, FileName, ProcessCommandLine, InitiatingProcessAccountName

  - name: file_transfers
    depends_on: [auth_to_targets]
    query: |
      DeviceFileEvents
      | where TimeGenerated > ago({{inputs.timeframe}})
      | where DeviceName in ({{auth_to_targets.target_hosts}})
      | where ActionType in ("FileCreated", "FileModified")
      | where FileName endswith ".exe" or FileName endswith ".dll" or FileName endswith ".ps1"
      | project TimeGenerated, DeviceName, FileName, FolderPath, InitiatingProcessAccountName
```

---

### Compromised Account Investigation

Investigate activity from a known-compromised user account.

```yaml
kind: investigation
name: "Compromised Account Investigation"
description: "Full investigation of a compromised user account"
version: "1.0"

inputs:
  - name: user_upn
    description: "User Principal Name of compromised account"
    required: true

steps:
  - name: sign_in_history
    query: |
      SigninLogs
      | where TimeGenerated > ago(30d)
      | where UserPrincipalName =~ "{{inputs.user_upn}}"
      | project TimeGenerated, IPAddress, Location, AppDisplayName, ResultType, DeviceDetail
    extract:
      ip_addresses:
        column: IPAddress
        type: array
        dedupe: true

  - name: other_users_same_ips
    depends_on: [sign_in_history]
    query: |
      SigninLogs
      | where TimeGenerated > ago(30d)
      | where IPAddress in ({{sign_in_history.ip_addresses}})
      | where UserPrincipalName !~ "{{inputs.user_upn}}"
      | summarize
          SignInCount=count(),
          Apps=make_set(AppDisplayName)
        by UserPrincipalName, IPAddress
      | where SignInCount > 1
    extract:
      potentially_compromised:
        column: UserPrincipalName
        type: array
        dedupe: true

  - name: mailbox_rules
    query: |
      OfficeActivity
      | where TimeGenerated > ago(30d)
      | where UserId =~ "{{inputs.user_upn}}"
      | where Operation in ("New-InboxRule", "Set-InboxRule", "Enable-InboxRule")
      | project TimeGenerated, Operation, Parameters

  - name: mail_forwarding
    query: |
      EmailEvents
      | where TimeGenerated > ago(30d)
      | where SenderFromAddress =~ "{{inputs.user_upn}}"
      | summarize
          TotalSent=count(),
          ExternalRecipients=countif(RecipientEmailAddress !endswith "@yourdomain.com")
        by bin(TimeGenerated, 1d)

  - name: file_access
    query: |
      OfficeActivity
      | where TimeGenerated > ago(30d)
      | where UserId =~ "{{inputs.user_upn}}"
      | where Operation in ("FileDownloaded", "FileAccessed", "FileSyncDownloadedFull")
      | summarize
          Downloads=count(),
          UniqueFiles=dcount(SourceFileName)
        by bin(TimeGenerated, 1h)
      | where Downloads > 50

  - name: investigate_related_users
    depends_on: [other_users_same_ips]
    query: |
      SigninLogs
      | where TimeGenerated > ago(7d)
      | where UserPrincipalName in ({{other_users_same_ips.potentially_compromised}})
      | summarize
          TotalSignIns=count(),
          FailedSignIns=countif(ResultType != 0),
          UniqueIPs=dcount(IPAddress),
          Countries=make_set(Location)
        by UserPrincipalName
```

---

### Threat Intelligence Enrichment

Investigate IoCs from threat intelligence with progressive enrichment.

```yaml
kind: investigation
name: "Threat Intel Enrichment"
description: "Enrich threat intelligence indicators with local telemetry"
version: "1.0"

inputs:
  - name: ioc_type
    description: "Type of IoC: ip, domain, hash"
    required: true
  - name: ioc_value
    description: "The indicator value to investigate"
    required: true

steps:
  - name: network_hits
    query: |
      DeviceNetworkEvents
      | where TimeGenerated > ago(90d)
      | where RemoteIP == "{{inputs.ioc_value}}"
         or RemoteUrl contains "{{inputs.ioc_value}}"
      | project TimeGenerated, DeviceName, RemoteIP, RemoteUrl, InitiatingProcessFileName
    extract:
      affected_devices:
        column: DeviceName
        type: array
        dedupe: true
      initiating_processes:
        column: InitiatingProcessFileName
        type: array
        dedupe: true

  - name: dns_queries
    query: |
      DeviceEvents
      | where TimeGenerated > ago(90d)
      | where ActionType == "DnsQueryResponse"
      | extend DnsQuery = extractjson("$.query", AdditionalFields)
      | where DnsQuery contains "{{inputs.ioc_value}}"
      | project TimeGenerated, DeviceName, DnsQuery
    extract:
      dns_devices:
        column: DeviceName
        type: array
        dedupe: true

  - name: device_timeline
    depends_on: [network_hits]
    query: |
      DeviceProcessEvents
      | where TimeGenerated > ago(90d)
      | where DeviceName in ({{network_hits.affected_devices}})
      | where InitiatingProcessFileName in ({{network_hits.initiating_processes}})
      | project TimeGenerated, DeviceName, FileName, ProcessCommandLine, InitiatingProcessFileName
      | order by TimeGenerated asc

  - name: file_hashes
    depends_on: [network_hits]
    query: |
      DeviceFileEvents
      | where TimeGenerated > ago(90d)
      | where DeviceName in ({{network_hits.affected_devices}})
      | where InitiatingProcessFileName in ({{network_hits.initiating_processes}})
      | distinct SHA256, FileName
    extract:
      related_hashes:
        column: SHA256
        type: array
        dedupe: true

  - name: vt_correlation
    depends_on: [file_hashes]
    query: |
      // Query your threat intel table for hash matches
      ThreatIntelligenceIndicator
      | where TimeGenerated > ago(90d)
      | where FileHashValue in ({{file_hashes.related_hashes}})
      | project IndicatorId, Description, ThreatType, Confidence, FileHashValue
```

---

### Data Exfiltration Investigation

Detect potential data exfiltration patterns.

```yaml
kind: investigation
name: "Data Exfiltration Investigation"
description: "Investigate potential data exfiltration from cloud and endpoint"
version: "1.0"

inputs:
  - name: threshold_mb
    description: "Data transfer threshold in MB"
    default: "100"

steps:
  - name: large_uploads
    query: |
      DeviceNetworkEvents
      | where TimeGenerated > ago(7d)
      | where ActionType == "NetworkSend"
      | where RemoteIPType == "Public"
      | summarize
          TotalBytesSent=sum(SentBytes),
          UniqueDestinations=dcount(RemoteIP)
        by DeviceName, InitiatingProcessFileName
      | where TotalBytesSent > ({{inputs.threshold_mb}} * 1024 * 1024)
      | project DeviceName, InitiatingProcessFileName, TotalMB=TotalBytesSent/1024/1024, UniqueDestinations
    extract:
      suspicious_devices:
        column: DeviceName
        type: array
        dedupe: true
      suspicious_processes:
        column: InitiatingProcessFileName
        type: array
        dedupe: true

  - name: cloud_downloads
    query: |
      OfficeActivity
      | where TimeGenerated > ago(7d)
      | where Operation in ("FileDownloaded", "FileSyncDownloadedFull")
      | summarize
          DownloadCount=count(),
          UniqueFiles=dcount(SourceFileName),
          TotalSize=sum(FileSize)
        by UserId
      | where TotalSize > ({{inputs.threshold_mb}} * 1024 * 1024)
    extract:
      high_download_users:
        column: UserId
        type: array
        dedupe: true

  - name: device_file_activity
    depends_on: [large_uploads]
    query: |
      DeviceFileEvents
      | where TimeGenerated > ago(7d)
      | where DeviceName in ({{large_uploads.suspicious_devices}})
      | where InitiatingProcessFileName in ({{large_uploads.suspicious_processes}})
      | where ActionType in ("FileCreated", "FileModified", "FileRenamed")
      | project TimeGenerated, DeviceName, FileName, FolderPath, FileSize, InitiatingProcessFileName
      | order by TimeGenerated desc

  - name: user_email_attachments
    depends_on: [cloud_downloads]
    query: |
      EmailAttachmentInfo
      | where TimeGenerated > ago(7d)
      | where SenderFromAddress in ({{cloud_downloads.high_download_users}})
      | summarize
          AttachmentCount=count(),
          TotalSize=sum(FileSize),
          FileTypes=make_set(FileType)
        by SenderFromAddress, RecipientEmailAddress
      | where TotalSize > ({{inputs.threshold_mb}} * 1024 * 1024)

  - name: usb_activity
    depends_on: [large_uploads]
    query: |
      DeviceEvents
      | where TimeGenerated > ago(7d)
      | where DeviceName in ({{large_uploads.suspicious_devices}})
      | where ActionType == "PnpDeviceConnected"
      | where DeviceClass == "USB"
      | project TimeGenerated, DeviceName, DeviceDescription, DeviceId
```

---

## Validation

### CLI Validation

```bash
# Validate pack structure without executing
kql-panopticon run-investigation pack.yaml --validate-only
```

### Validation Checks

| Check | Error Message |
|-------|---------------|
| Invalid kind | `Invalid kind 'X', expected 'investigation'` |
| No steps | `Investigation pack must have at least one step` |
| Duplicate step names | `Duplicate step name: 'X'` |
| Missing dependency | `Step 'X' depends on non-existent step 'Y'` |
| Circular dependency | `Circular dependency detected involving step 'X'` |
| Invalid variable syntax | `Invalid variable reference '{{X}}'` |
| Undefined input | `Step 'X' references undefined input 'Y'` |
| Missing depends_on | `Step 'X' references 'Y' but does not declare it in depends_on` |
| Missing extraction | `Step 'X' references '{{Y.Z}}' but step 'Y' does not extract 'Z'` |

---

## CLI Reference

```bash
kql-panopticon run-investigation <pack> [OPTIONS]

Arguments:
  <pack>  Path to investigation pack file (.yaml, .yml, or .json)
          Can be absolute path or relative to ~/.kql-panopticon/investigations/

Options:
  -s, --set <KEY=VALUE>        Set input variable (can be used multiple times)
  -w, --workspaces <LIST>      Workspace selection (comma-separated IDs or 'all')
  -o, --output <PATH>          Override output base directory
      --validate-only          Validate pack without executing
      --json                   Print results to stdout as JSON
  -h, --help                   Print help
```

### Examples

```bash
# Basic execution
kql-panopticon run-investigation threat-hunt.yaml

# With multiple input variables
kql-panopticon run-investigation phishing.yaml \
  --set malicious_url=evil.com \
  --set lookback_days=14

# Target specific workspaces
kql-panopticon run-investigation investigation.yaml \
  --workspaces sentinel-prod,sentinel-dev

# Custom output directory
kql-panopticon run-investigation investigation.yaml \
  --output /path/to/output

# Validate then execute
kql-panopticon run-investigation investigation.yaml --validate-only && \
kql-panopticon run-investigation investigation.yaml
```

---

## TUI Usage

### Investigations Tab (Key: 7)

| Key | Action |
|-----|--------|
| `Up/Down` | Navigate investigation list |
| `Enter` | View investigation details |
| `e` | Execute selected investigation |
| `r` | Refresh list from disk |

### Execution Flow

1. Press `7` to open Investigations tab
2. Select investigation with `Up/Down`
3. Press `e` to execute
4. If inputs required, popup prompts for values
5. Investigation runs across selected workspaces
6. Results appear in Jobs tab with step-by-step progress

---

## Best Practices

### Step Design

1. **Single responsibility**: Each step should answer one question
2. **Meaningful names**: Use descriptive step names (they appear in output paths)
3. **Extract only what's needed**: Don't extract columns you won't reference
4. **Consider chunk_size**: For large datasets, set appropriate chunk sizes

### Variable Usage

1. **Always dedupe arrays**: Prevents duplicate queries (default: true)
2. **Use appropriate quote_style**: Match your KQL comparison operators
3. **Handle empty arrays**: Queries fail if substituting empty arrays

### Performance

1. **Minimize dependencies**: Parallel steps execute faster
2. **Use appropriate timeframes**: Shorter timeframes = faster queries
3. **Leverage chunking**: Large arrays should use chunk_size < 1000

### Error Handling

1. **Validate first**: Always run with `--validate-only` before execution
2. **Check manifests**: Review `manifest.json` for failure details
3. **Independent workspaces**: Failures in one workspace don't affect others

---

## Troubleshooting

### Common Issues

**"Variable 'X.Y' not found in context"**
- Ensure the step defining the extraction is in `depends_on`
- Verify the extraction name matches exactly

**"Empty array substitution"**
- The query returned no results for that extraction
- Check the source step's results for the expected data

**"Circular dependency detected"**
- Review your `depends_on` declarations for loops
- Use the validation command to identify the cycle

**"Column 'X' not found in results"**
- Verify the column name in your extraction matches the query output
- Column names are case-sensitive

### Debug Tips

1. Check step outputs in `{workspace}/{step}/results.json`
2. Review extractions in `{workspace}/{step}/extracts.json`
3. Examine `manifest.json` for execution timing and errors
4. Use `--validate-only` to catch configuration issues early

---

## Integration with AI Assistants

When generating investigation packs with AI:

1. Provide this documentation as context
2. Specify the target data sources (Sentinel, MDE, etc.)
3. Describe the investigation goal and expected pivots
4. Request validation-ready YAML output

### Prompt Template

```
Generate a KQL investigation pack for [GOAL].

Requirements:
- Kind must be "investigation"
- Use inputs for user-provided values like [IP, user, hostname]
- Extract relevant fields for pivoting between steps
- Use depends_on to chain related queries
- Include appropriate quote_style for string comparisons
- Target these tables: [TABLE_LIST]

Output complete, valid YAML that passes validation.
```
