# Step Options

Options control step execution behavior including dependencies, conditions, and error handling.

## depends_on

Declare steps that must complete before this step executes.

```yaml
steps:
  - name: initial
    query: SigninLogs | take 100

  - name: enrichment
    depends_on: [initial]
    query: |
      AADUserRiskEvents
      | where UserPrincipalName in ({{initial.UserPrincipalName | array}})
```

**Multiple dependencies**:

```yaml
- name: correlation
  depends_on: [signins, alerts, users]
  query: |
    // Uses data from all three steps
```

**Dependency rules**:
- Steps execute after all dependencies complete
- Circular dependencies are invalid
- Referenced steps must exist

## when

Conditional execution based on boolean expressions.

```yaml
- name: enrich_users
  depends_on: [signins]
  when: "{{signins | is_not_empty}}"
  query: |
    AADUserRiskEvents | ...
```

### Common Conditions

| Condition | Description |
|-----------|-------------|
| `{{step \| is_not_empty}}` | Step has results |
| `{{step \| is_empty}}` | Step is empty |
| `{{step \| length \| gt(N)}}` | More than N rows |
| `{{step \| any(field > val)}}` | Any row matches |
| `{{step \| all(field == val)}}` | All rows match |

### Compound Conditions

Use `and`, `or`, `not` outside the `{{}}` blocks:

```yaml
# Both conditions
when: "{{step1 | is_not_empty}} and {{step2 | is_not_empty}}"

# Either condition
when: "{{step1 | is_empty}} or {{step2 | is_empty}}"

# Negation
when: "not {{step | is_empty}}"

# Complex
when: "{{alerts | any(Severity == 'High')}} and not {{resolved | is_not_empty}}"
```

### Skipped Steps

When condition is false:
- Step is marked as skipped
- No error is raised
- Dependent steps may still run (with empty upstream data)

## on_error

Control behavior when step execution fails.

| Value | Behavior |
|-------|----------|
| `fail` (default) | Stop pack execution |
| `skip` | Skip step, continue pack |
| `continue` | Record error, continue pack |

```yaml
# Stop on failure (default)
- name: critical_data
  on_error: fail
  query: SigninLogs | ...

# Skip if fails
- name: optional_enrichment
  on_error: skip
  query: OptionalTable | ...

# Continue with error recorded
- name: best_effort
  on_error: continue
  query: MayFailTable | ...
```

### With for_each (HTTP)

For HTTP steps with `for_each`:

```yaml
- name: enrich_ips
  type: http
  on_error: continue
  request:
    params:
      ip: "{{ips.IPAddress | for_each}}"
```

- `fail`: Stop on first failed request
- `skip`: Skip entire step if any request fails
- `continue`: Aggregate successful responses, record failures

## rate_limit (HTTP only)

Control request rate for HTTP steps.

```yaml
rate_limit:
  requests: 10
  per: second
```

| Period | Description |
|--------|-------------|
| `second` | Requests per second |
| `minute` | Requests per minute |
| `hour` | Requests per hour |

**Example**:

```yaml
- name: api_calls
  type: http
  rate_limit:
    requests: 30
    per: minute
  request:
    url: "https://api.example.com/check"
    params:
      ip: "{{ips.IPAddress | for_each}}"
```

## options (Step-level)

Fine-tune step behavior.

```yaml
- name: data
  query: SigninLogs | take 100
  options:
    quote_style: single
    dedupe: true
    chunk_size: 100
```

### quote_style

Control how array values are quoted in substitution.

| Value | Output | Use Case |
|-------|--------|----------|
| `single` (default) | `'value'` | KQL queries |
| `double` | `"value"` | JSON bodies |
| `verbatim` | `@'value'` | KQL with special chars |

```yaml
options:
  quote_style: verbatim
```

Or use the `quote()` transform:

```yaml
query: |
  | where Path in ({{paths.FilePath | array | quote(verbatim)}})
```

### dedupe

Deduplicate values before substitution.

```yaml
options:
  dedupe: true
```

Or use the `unique` transform:

```yaml
{{step.column | unique | array}}
```

### chunk_size

Split large arrays into chunks.

```yaml
options:
  chunk_size: 100
```

Useful when queries have limits on `in()` clause size.

## Execution Order Example

```yaml
steps:
  # Level 0: No dependencies
  - name: signins
    query: SigninLogs | take 100

  - name: alerts
    query: SecurityAlert | take 50

  # Level 1: Depends on level 0
  - name: enrich_users
    depends_on: [signins]
    when: "{{signins | is_not_empty}}"
    query: AADUserRiskEvents | ...

  # Level 2: Depends on level 1
  - name: correlate
    depends_on: [enrich_users, alerts]
    when: "{{enrich_users | is_not_empty}} or {{alerts | is_not_empty}}"
    on_error: continue
    query: |
      // Correlation logic
```

**Execution**:
1. `signins` and `alerts` run in parallel
2. `enrich_users` runs after `signins` (if condition met)
3. `correlate` runs after both `enrich_users` and `alerts`

## Related

- [KQL Steps](steps/kql.md) - KQL step configuration
- [HTTP Steps](steps/http.md) - HTTP step configuration
- [Variable Syntax](../reference/variable-syntax.md) - Condition syntax
- [Predicates](../reference/predicates.md) - Filtering expressions
