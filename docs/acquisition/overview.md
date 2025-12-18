# Acquisition Phase

The acquisition phase collects data from Azure Log Analytics workspaces, external APIs, and local files. This is the only required phase in a pack.

## Structure

```yaml
acquisition:
  inputs: [...]      # User-provided values
  secrets: {...}     # Environment variable references
  steps: [...]       # Data collection steps (required)
  output: {...}      # Output folder configuration
```

## Components

### Inputs

User-provided values at runtime. Can be strings or arrays.

```yaml
inputs:
  - name: target_user
    type: string
    label: Target User
    required: true

  - name: ip_list
    type: array
    label: IP Addresses
    default: "8.8.8.8,1.1.1.1"
```

See: [Inputs](inputs.md)

### Secrets

References to environment variables for sensitive values.

```yaml
secrets:
  api_key: "${THREAT_INTEL_API_KEY}"
  auth_token: "${AZURE_TOKEN}"
```

See: [Secrets](secrets.md)

### Steps

Data collection operations. Three types available:

| Type | Description |
|------|-------------|
| `kql` | Execute KQL queries against Log Analytics |
| `http` | Call external APIs |
| `file` | Read local files (CSV, JSON, YAML) |

```yaml
steps:
  - name: signins
    type: kql
    query: |
      SigninLogs
      | where TimeGenerated > ago(7d)
```

See: [KQL Steps](steps/kql.md), [HTTP Steps](steps/http.md), [File Steps](steps/file.md)

### Output

Configure where results are saved.

```yaml
output:
  folder: "output/{{meta.pack_name}}/{{meta.timestamp}}"
```

See: [Output](output.md)

## Step Execution

Steps execute based on their dependencies:

1. **Topological Sort**: Steps ordered by `depends_on` declarations
2. **Conditional Execution**: `when` clauses evaluated before execution
3. **Error Handling**: `on_error` determines behavior on failure

```yaml
steps:
  - name: initial_search
    type: kql
    query: |
      SigninLogs | take 100

  - name: enrich_users
    type: kql
    depends_on: [initial_search]
    when: "{{initial_search | is_not_empty}}"
    on_error: continue
    query: |
      AADUserRiskEvents
      | where UserPrincipalName in ({{initial_search.UserPrincipalName | array}})
```

See: [Step Options](options.md)

## Result Storage

Each step's results are stored as JSONL (JSON Lines) format:
- One JSON object per row
- Available for subsequent steps via variable syntax
- Persisted to output folder

## Minimal Example

```yaml
acquisition:
  steps:
    - name: usage
      query: |
        Usage
        | summarize TotalGB = sum(Quantity) / 1024 by DataType
        | order by TotalGB desc
```

## Complete Example

```yaml
acquisition:
  inputs:
    - name: target_user
      type: string
      label: Target User
      required: true

    - name: days
      label: Lookback Days
      default: "7"

  secrets:
    threat_api_key: "${THREAT_INTEL_KEY}"

  steps:
    - name: signins
      type: kql
      query: |
        SigninLogs
        | where UserPrincipalName == "{{inputs.target_user}}"
        | where TimeGenerated > ago({{inputs.days}}d)
        | summarize Count = count() by IPAddress
      timespan: P30D

    - name: threat_check
      type: http
      depends_on: [signins]
      when: "{{signins | is_not_empty}}"
      request:
        method: GET
        url: "https://api.threatintel.com/check"
        params:
          ip: "{{signins.IPAddress | for_each}}"
        headers:
          Authorization: "Bearer {{secrets.threat_api_key}}"
      response:
        fields:
          ip: "$.ip"
          score: "$.threat_score"
      rate_limit:
        requests: 10
        per: second

  output:
    folder: "investigations/{{inputs.target_user}}"
```

## Related

- [Inputs](inputs.md) - User-provided values
- [Secrets](secrets.md) - Environment variables
- [KQL Steps](steps/kql.md) - KQL query execution
- [HTTP Steps](steps/http.md) - API calls
- [File Steps](steps/file.md) - Local file data
- [Step Options](options.md) - Dependencies, conditions, error handling
- [Output](output.md) - Output configuration
