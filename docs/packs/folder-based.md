# Folder-Based Packs

Folder-based packs split phases across multiple files for better organization and maintainability.

## Directory Structure

```
my-pack/
├── pack.yaml           # Metadata (required)
├── acquisition.yaml    # Acquisition phase (required)
├── processing.yaml     # Processing phase (optional)
└── reporting.yaml      # Reporting phase (optional)
```

## pack.yaml

The `pack.yaml` file contains only metadata. Phase definitions go in separate files.

```yaml
name: "My Investigation Pack"
description: |
  Multi-file pack demonstrating folder-based organization.
  Phases are defined in separate files for clarity.
version: "1.0"
```

## acquisition.yaml

Contains inputs, secrets, steps, and output configuration:

```yaml
inputs:
  - name: target_user
    type: string
    label: Target User
    description: User to investigate
    required: true

  - name: days
    type: string
    label: Lookback Days
    default: "7"

steps:
  - name: signins
    type: kql
    query: |
      SigninLogs
      | where UserPrincipalName == "{{inputs.target_user}}"
      | where TimeGenerated > ago({{inputs.days}}d)
      | summarize Count = count() by IPAddress, Location
    timespan: P30D

  - name: alerts
    type: kql
    depends_on: [signins]
    when: "{{signins | is_not_empty}}"
    query: |
      SecurityAlert
      | where Entities has "{{inputs.target_user}}"
      | summarize AlertCount = count() by AlertName, Severity
    timespan: P30D
```

## processing.yaml

Contains processing steps (currently scoring):

```yaml
steps:
  - name: risk_assessment
    type: scoring
    indicators:
      - name: multiple_locations
        condition: "{{signins | length | gt(3)}}"
        weight: 20
        description: "Sign-ins from many locations"

      - name: high_severity_alerts
        condition: "{{alerts | any(Severity == 'High')}}"
        weight: 40
        description: "High severity alerts present"

    thresholds:
      - level: CRITICAL
        min_score: 80
        summary: "Critical risk detected"
      - level: HIGH
        min_score: 50
        summary: "High risk detected"
      - level: MEDIUM
        min_score: 20
        summary: "Moderate risk"
      - level: LOW
        min_score: 0
        summary: "Low risk"
```

## reporting.yaml

Contains report definitions:

```yaml
reports:
  - name: summary
    format: markdown
    output: "investigation-{{meta.timestamp}}.md"
    template: |
      # Investigation Report

      **User**: {{ inputs.target_user }}
      **Generated**: {{ meta.timestamp }}

      ## Risk Assessment
      - Score: {{ processing.risk_assessment.score }}
      - Level: {{ processing.risk_assessment.level }}

      ## Sign-in Activity
      {% for row in signins %}
      - {{ row.IPAddress }} ({{ row.Location }}): {{ row.Count }} sign-ins
      {% endfor %}

  - name: json_export
    format: json
    output: "data-{{meta.timestamp}}.json"
    when: "{{signins | is_not_empty}}"
```

## Complete Example

See `examples/test-pack/` for a complete folder-based pack example:

```
examples/test-pack/
├── pack.yaml           # Metadata
├── acquisition.yaml    # Inputs and 4 KQL steps with dependencies
├── processing.yaml     # Scoring with 6 indicators and 4 thresholds
└── reporting.yaml      # 4 conditional reports
```

## Benefits of Folder-Based Organization

| Benefit | Description |
|---------|-------------|
| **Readability** | Each phase in its own file, easier to navigate |
| **Version control** | Changes to one phase don't affect others |
| **Reusability** | Phase files can potentially be shared |
| **Collaboration** | Different team members can work on different phases |

## Loading Behavior

When loading a folder-based pack:

1. `pack.yaml` is loaded first for metadata
2. `acquisition.yaml` is loaded and merged
3. `processing.yaml` is loaded if present
4. `reporting.yaml` is loaded if present

If a phase file is missing, that phase is not executed (except acquisition which is required).

## Related

- [Pack Overview](overview.md) - Three-phase model explanation
- [Single-File Packs](single-file.md) - Simple YAML format
- [Acquisition Overview](../acquisition/overview.md) - Data collection phase
