# KQL Steps

KQL steps execute Kusto Query Language queries against Azure Log Analytics workspaces.

## Schema

```yaml
- name: step_name             # Required: unique identifier
  type: kql                   # Optional: default is kql
  query: |                    # Required: KQL query
    SigninLogs
    | where TimeGenerated > ago(7d)
  timespan: P7D               # Optional: ISO 8601 duration
  depends_on: [prev_step]     # Optional: execution dependencies
  when: "{{prev | is_not_empty}}"  # Optional: conditional execution
  on_error: continue          # Optional: error handling
  options:                    # Optional: step options
    quote_style: single
```

## Fields

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `name` | Yes | - | Unique step identifier |
| `type` | No | `kql` | Step type |
| `query` | Yes | - | KQL query to execute |
| `timespan` | No | - | Query time range (ISO 8601) |
| `depends_on` | No | `[]` | Steps that must complete first |
| `when` | No | - | Condition for execution |
| `on_error` | No | `fail` | Error handling behavior |
| `options` | No | - | Step-level options |

## Basic Example

```yaml
steps:
  - name: signins
    query: |
      SigninLogs
      | where TimeGenerated > ago(7d)
      | summarize Count = count() by UserPrincipalName
      | order by Count desc
      | take 100
    timespan: P7D
```

## Variable Substitution

Use `{{...}}` syntax to inject values into queries:

### Input Values

```yaml
query: |
  SigninLogs
  | where UserPrincipalName == "{{inputs.target_user}}"
  | where TimeGenerated > ago({{inputs.days}}d)
```

### Previous Step Results

```yaml
query: |
  let users = dynamic([{{signins.UserPrincipalName | array}}]);
  AADUserRiskEvents
  | where UserPrincipalName in (users)
```

### Array Formatting

```yaml
# Default single quotes
query: |
  SigninLogs
  | where IPAddress in ({{ips.IPAddress | array}})
  # Result: in ('10.0.0.1','192.168.1.1')

# Double quotes
query: |
  let values = dynamic([{{ips.IPAddress | array | quote(double)}}]);
  # Result: dynamic(["10.0.0.1","192.168.1.1"])
```

## Timespan

ISO 8601 duration format:

| Format | Duration |
|--------|----------|
| `PT1H` | 1 hour |
| `P1D` | 1 day |
| `P7D` | 7 days |
| `P30D` | 30 days |
| `P1M` | 1 month |

```yaml
- name: recent_activity
  query: |
    SigninLogs | take 100
  timespan: P7D
```

## Dependencies

Declare steps that must complete before this step:

```yaml
steps:
  - name: users
    query: |
      SigninLogs | distinct UserPrincipalName | take 10

  - name: user_details
    depends_on: [users]
    query: |
      let targets = dynamic([{{users.UserPrincipalName | array}}]);
      AADUserInfo | where UserPrincipalName in (targets)
```

## Conditional Execution

Only execute if condition is true:

```yaml
- name: enrichment
  depends_on: [initial_search]
  when: "{{initial_search | is_not_empty}}"
  query: |
    AADUserRiskEvents
    | where UserPrincipalName in ({{initial_search.UserPrincipalName | array}})
```

Common conditions:

```yaml
when: "{{step | is_not_empty}}"        # Step has results
when: "{{step | is_empty}}"            # Step is empty
when: "{{step | any(score > 50)}}"     # Any row matches
when: "{{step | length | gt(10)}}"     # More than 10 rows
```

## Error Handling

Control behavior when query fails:

| Value | Behavior |
|-------|----------|
| `fail` (default) | Stop pack execution |
| `skip` | Skip step, continue pack |
| `continue` | Record error, continue pack |

```yaml
- name: optional_data
  on_error: continue
  query: |
    OptionalTable | take 10
```

## Step Options

```yaml
- name: data
  query: |
    SigninLogs | take 100
  options:
    quote_style: verbatim    # KQL verbatim strings
    dedupe: true             # Deduplicate values
    chunk_size: 100          # Chunk large arrays
```

## Using Results in Later Steps

Step results are available via variable syntax:

```yaml
steps:
  - name: ips
    query: |
      SigninLogs | distinct IPAddress | take 10

  - name: correlate
    depends_on: [ips]
    query: |
      # First value only
      let first_ip = "{{ips.IPAddress | first}}";

      # All values as array
      let all_ips = dynamic([{{ips.IPAddress | array}}]);

      # Unique values
      let unique_ips = dynamic([{{ips.IPAddress | unique | array}}]);

      SigninLogs | where IPAddress in (all_ips)
```

## Datatable for Testing

Use datatable to create test data without actual logs:

```yaml
- name: test_data
  query: |
    datatable(User: string, Score: int) [
      "alice@contoso.com", 85,
      "bob@contoso.com", 42
    ]
    | extend RiskLevel = iff(Score > 75, "High", "Low")
```

## Allowed Transforms

In KQL queries, these output types are allowed:

| Transform | Output | Example |
|-----------|--------|---------|
| `first` | Scalar | `{{step.col \| first}}` |
| `at(N)` | Scalar | `{{step.col \| at(0)}}` |
| `array` | Array | `{{step.col \| array}}` |
| `unique` | Column | `{{step.col \| unique \| array}}` |
| `str_join(sep)` | Scalar | `{{step.col \| str_join(',')}}` |
| `quote(style)` | Array | `{{step.col \| array \| quote(double)}}` |

**Not allowed in KQL**: `for_each` (HTTP only)

## Related

- [Step Options](../options.md) - Dependencies, conditions, error handling
- [Variable Syntax](../../reference/variable-syntax.md) - Substitution syntax
- [Transform Reference](../../reference/transforms.md) - Available transforms
