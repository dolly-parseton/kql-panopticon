# File Steps

File steps read data from local files (CSV, JSON, YAML).

## Schema

```yaml
- name: step_name             # Required: unique identifier
  type: file                  # Required: must be "file"

  source:                     # Required: file configuration
    path: "./data/file.csv"   # File path (supports variables)
    format: csv               # Optional: csv | json | yaml
    csv:                      # Optional: CSV options
      delimiter: ","
      has_header: true

  depends_on: [...]           # Optional: dependencies
  when: "{{...}}"             # Optional: condition
  on_error: continue          # Optional: error handling
```

## Fields

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `source.path` | Yes | - | File path (supports variables) |
| `source.format` | No | auto | File format |
| `source.csv` | No | - | CSV-specific options |

## File Formats

Format is auto-detected from extension if not specified:

| Extension | Format |
|-----------|--------|
| `.csv` | CSV |
| `.json` | JSON |
| `.yaml`, `.yml` | YAML |

### CSV Files

```yaml
- name: ioc_list
  type: file
  source:
    path: "./data/indicators.csv"
    csv:
      delimiter: ","      # Default: comma
      has_header: true    # Default: true
```

**CSV with custom delimiter**:

```yaml
source:
  path: "./data/export.csv"
  csv:
    delimiter: ";"
    has_header: true
```

**CSV without header**:

```yaml
source:
  path: "./data/raw.csv"
  csv:
    has_header: false
```

When `has_header: false`, columns are named `column_0`, `column_1`, etc.

### JSON Files

JSON arrays become rows, objects become single-row tables:

```yaml
- name: config
  type: file
  source:
    path: "./config/rules.json"
```

**Array JSON**:
```json
[
  {"ip": "8.8.8.8", "category": "dns"},
  {"ip": "1.1.1.1", "category": "dns"}
]
```
Results in 2 rows.

**Object JSON**:
```json
{"setting": "value", "enabled": true}
```
Results in 1 row.

### YAML Files

Same behavior as JSON:

```yaml
- name: rules
  type: file
  source:
    path: "./rules/scoring.yaml"
```

## Variable Substitution

File paths support variable substitution:

```yaml
- name: custom_data
  type: file
  source:
    path: "./data/{{inputs.dataset}}.csv"
```

## Basic Example

```yaml
steps:
  - name: ioc_list
    type: file
    source:
      path: "./data/known_bad_ips.csv"

  - name: check_iocs
    type: kql
    depends_on: [ioc_list]
    query: |
      let bad_ips = dynamic([{{ioc_list.ip | array}}]);
      SigninLogs
      | where IPAddress in (bad_ips)
```

## Using Results

File step results are available like other step results:

```yaml
# In conditions
when: "{{ioc_list | is_not_empty}}"

# In queries
query: |
  let indicators = dynamic([{{ioc_list.indicator | array}}]);
  SecurityAlert | where Entities has_any (indicators)

# Column operations
{{file_data.column | first}}
{{file_data.column | unique | array}}
```

## Error Handling

| Value | Behavior |
|-------|----------|
| `fail` (default) | Stop if file not found or parse error |
| `skip` | Skip step on error |
| `continue` | Record error, continue with empty result |

```yaml
- name: optional_data
  type: file
  on_error: continue
  source:
    path: "./data/optional.csv"
```

## Type Inference

CSV values are automatically typed:

| Value | Inferred Type |
|-------|---------------|
| `123` | Integer |
| `3.14` | Float |
| `true`, `false` | Boolean |
| `null`, empty | Null |
| Everything else | String |

## Complete Example

```yaml
acquisition:
  inputs:
    - name: dataset
      label: Dataset Name
      default: "default"

  steps:
    - name: known_threats
      type: file
      source:
        path: "./data/{{inputs.dataset}}_threats.csv"
        csv:
          delimiter: ","
          has_header: true

    - name: correlate
      type: kql
      depends_on: [known_threats]
      when: "{{known_threats | is_not_empty}}"
      query: |
        let threat_ips = dynamic([{{known_threats.ip | array}}]);
        SigninLogs
        | where IPAddress in (threat_ips)
        | summarize Count = count() by IPAddress, UserPrincipalName
```

## Limitations

- Local filesystem only (no remote URLs)
- No `for_each` support (unlike HTTP steps)
- File must exist at execution time

## Related

- [Step Options](../options.md) - Dependencies, conditions
- [Variable Syntax](../../reference/variable-syntax.md) - Path substitution
