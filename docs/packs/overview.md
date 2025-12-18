# Pack Overview

A pack is a self-contained definition of a data collection and analysis workflow. Packs define what data to collect, how to process it, and what reports to generate.

## Three-Phase Model

Every pack execution follows three sequential phases:

```
Acquisition ──► Processing ──► Reporting
(required)      (optional)     (optional)
```

### Acquisition Phase

**Purpose**: Collect data from workspaces and external sources.

- Define user inputs and environment secrets
- Execute steps: KQL queries, HTTP API calls, or file reads
- Chain steps with dependencies and conditional execution
- Store results for subsequent phases

See: [Acquisition Overview](../acquisition/overview.md)

### Processing Phase

**Purpose**: Transform and analyze collected data.

- Calculate risk scores based on indicators
- Apply weighted scoring with configurable thresholds
- Generate structured assessment results

See: [Scoring](../processing/scoring.md)

### Reporting Phase

**Purpose**: Generate formatted output reports.

- Define multiple report formats (Markdown, HTML, JSON)
- Use Tera templates with full access to all phase results
- Conditional report generation

See: [Reporting Overview](../reporting/overview.md)

## Pack Organization

Packs can be organized in two ways:

### Single-File Packs

All phases defined in one YAML file:

```yaml
name: "My Investigation"
description: "Investigate user activity"
version: "1.0"

acquisition:
  inputs: [...]
  steps: [...]

processing:
  steps: [...]

reporting:
  reports: [...]
```

See: [Single-File Packs](single-file.md)

### Folder-Based Packs

Phases split across multiple files:

```
my-pack/
├── pack.yaml           # Metadata only
├── acquisition.yaml    # Acquisition phase
├── processing.yaml     # Processing phase (optional)
└── reporting.yaml      # Reporting phase (optional)
```

See: [Folder-Based Packs](folder-based.md)

## Minimal Pack Example

The simplest valid pack requires only a name and at least one acquisition step:

```yaml
name: "Simple Query"

acquisition:
  steps:
    - name: data
      query: |
        Usage
        | take 10
```

## Pack Metadata

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Pack identifier |
| `description` | No | Human-readable description |
| `version` | No | Pack version string |

## Execution Flow

1. **Load**: Pack file(s) parsed and validated
2. **Inputs**: User provides required input values
3. **Acquisition**: Steps execute in dependency order
4. **Processing**: Scoring and analysis applied
5. **Reporting**: Templates rendered with all results

## Related

- [Single-File Packs](single-file.md) - Simple YAML format
- [Folder-Based Packs](folder-based.md) - Modular organization
- [Acquisition Overview](../acquisition/overview.md) - Data collection phase
