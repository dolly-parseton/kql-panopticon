# Single-File Packs

Single-file packs contain all phases in one YAML file. This is the simplest format for smaller investigations.

## Structure

```yaml
name: "Pack Name"
description: "Optional description"
version: "1.0"

acquisition:
  inputs: [...]
  secrets: {...}
  steps: [...]
  output: {...}

processing:
  steps: [...]

reporting:
  reports: [...]
```

## Required Fields

| Field | Description |
|-------|-------------|
| `name` | Pack identifier (non-empty string) |
| `acquisition.steps` | At least one step required |

## Optional Fields

| Field | Description |
|-------|-------------|
| `description` | Human-readable description |
| `version` | Version string |
| `acquisition.inputs` | User-provided values |
| `acquisition.secrets` | Environment variable references |
| `acquisition.output` | Output folder configuration |
| `processing` | Processing phase configuration |
| `reporting` | Reporting phase configuration |

## Minimal Example

```yaml
name: "Simple Query"

acquisition:
  steps:
    - name: usage
      query: |
        Usage
        | summarize TotalBytes = sum(Quantity) by DataType
        | order by TotalBytes desc
```

## Complete Example

```yaml
name: "IP Investigation"
description: |
  Investigate IP addresses across Azure sign-in logs.
  Correlates sign-ins, users, and security alerts.
version: "1.0"

acquisition:
  inputs:
    - name: ip_list
      type: array
      label: IP Addresses
      description: Comma-separated list of IP addresses
      required: true
      example: "10.0.0.1,192.168.1.100"

    - name: days
      label: Lookback Days
      description: Number of days to search back
      default: "7"

  steps:
    - name: signin_activity
      type: kql
      query: |
        let target_ips = dynamic([{{inputs.ip_list | array}}]);
        SigninLogs
        | where TimeGenerated > ago({{inputs.days}}d)
        | where IPAddress in (target_ips)
        | summarize
            SigninCount = count(),
            FailedCount = countif(ResultType != "0"),
            Users = make_set(UserPrincipalName, 10)
          by IPAddress, Location
        | order by SigninCount desc
      timespan: P30D

    - name: affected_users
      type: kql
      depends_on: [signin_activity]
      when: "{{signin_activity | is_not_empty}}"
      query: |
        let target_ips = dynamic([{{signin_activity.IPAddress | array}}]);
        SigninLogs
        | where TimeGenerated > ago({{inputs.days}}d)
        | where IPAddress in (target_ips)
        | summarize SigninCount = count() by UserPrincipalName
        | order by SigninCount desc
      timespan: P30D

processing:
  steps:
    - name: risk_assessment
      type: scoring
      indicators:
        - name: high_failures
          condition: "{{signin_activity | any(FailedCount > 10)}}"
          weight: 30
          description: "High failed login count"

        - name: multiple_users
          condition: "{{affected_users | length | gt(5)}}"
          weight: 20
          description: "IP used by many users"

      thresholds:
        - level: HIGH
          min_score: 40
          summary: "Suspicious IP activity detected"
        - level: MEDIUM
          min_score: 20
          summary: "Moderate concern"
        - level: LOW
          min_score: 0
          summary: "No significant findings"

reporting:
  reports:
    - name: summary
      format: markdown
      output: "ip-investigation-{{meta.timestamp}}.md"
      template: |
        # IP Investigation Report

        **Generated**: {{ meta.timestamp }}
        **Workspace**: {{ meta.workspace }}

        ## Risk Assessment

        **Score**: {{ processing.risk_assessment.score }}
        **Level**: {{ processing.risk_assessment.level }}

        {% if processing.risk_assessment.matched_indicators %}
        ### Matched Indicators
        {% for ind in processing.risk_assessment.matched_indicators %}
        - {{ ind.name }} (+{{ ind.weight }})
        {% endfor %}
        {% endif %}

        ## Sign-in Activity

        | IP Address | Location | Sign-ins | Failed |
        |------------|----------|----------|--------|
        {% for row in signin_activity %}
        | {{ row.IPAddress }} | {{ row.Location }} | {{ row.SigninCount }} | {{ row.FailedCount }} |
        {% endfor %}

        ## Affected Users

        {% for row in affected_users %}
        - {{ row.UserPrincipalName }}: {{ row.SigninCount }} sign-ins
        {% endfor %}
```

## When to Use Single-File

Single-file packs are ideal for:

- Small investigations with few steps
- Self-contained packs that don't share components
- Quick prototypes and testing

## When to Use Folder-Based

Consider [folder-based packs](folder-based.md) when:

- Pack has many steps that benefit from organization
- You want to reuse step definitions
- Better version control visibility is needed

## Related

- [Pack Overview](overview.md) - Three-phase model explanation
- [Folder-Based Packs](folder-based.md) - Modular organization
- [Acquisition Overview](../acquisition/overview.md) - Data collection phase
