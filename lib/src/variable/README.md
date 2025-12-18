# Variable Module

This module provides unified variable substitution and condition evaluation for pack execution using a pipe-based transform syntax.

## Design Overview

The variable system uses a unified `{{...}}` syntax with composable transforms for:
- **Value substitution** in KQL queries, HTTP requests, and file paths
- **Condition evaluation** in `when:` clauses, verdict rules, and scoring indicators
- **Iteration** for HTTP steps via `for_each` transform

### Design Principles

1. **Explicit transforms** - Column references require explicit transforms (`first`, `array`, etc.) to ensure pack authors understand data shapes
2. **Composable pipelines** - Transforms chain via `|` operator, inspired by Polars DataFrame operations
3. **Unified syntax** - Conditions and substitutions use the same `{{...}}` syntax
4. **Type safety** - Validation ensures output types match context requirements

---

## Syntax Reference

### Sources

| Syntax | Description |
|--------|-------------|
| `{{inputs.name}}` | User-provided input (string or array) |
| `{{secrets.name}}` | Environment variable secret |
| `{{step}}` | Step result (for step-level transforms) |
| `{{step.column}}` | Column selection (requires terminal transform) |

### Step-Level Transforms

Operate on the step result without column selection.

| Transform | Output | Description |
|-----------|--------|-------------|
| `{{step \| is_empty}}` | Boolean | True if step has no rows |
| `{{step \| is_not_empty}}` | Boolean | True if step has rows |
| `{{step \| length}}` | Integer | Row count |
| `{{step \| any(predicate)}}` | Boolean | True if any row matches predicate |
| `{{step \| all(predicate)}}` | Boolean | True if all rows match predicate |
| `{{step \| filter(predicate)}}` | Filtered step | Filter rows, chain with `.column` |

### Column Transforms

Operate after column selection (`{{step.column | ...}}`).

| Transform | Output | Description |
|-----------|--------|-------------|
| `{{step.column \| first}}` | Scalar | First row's value |
| `{{step.column \| at(N)}}` | Scalar | Nth row's value (0-indexed) |
| `{{step.column \| array}}` | Quoted array | All values as quoted list |
| `{{step.column \| unique}}` | Series | Deduplicated values (chain with array/str_join) |
| `{{step.column \| str_join(sep)}}` | Scalar | Values joined with separator |
| `{{step.column \| for_each}}` | Iterator | Iteration trigger (HTTP only) |

### Comparison Transforms

Chain after `length` or scalar values for boolean output.

| Transform | Output | Description |
|-----------|--------|-------------|
| `\| eq(val)` | Boolean | Equals |
| `\| neq(val)` | Boolean | Not equals |
| `\| gt(val)` | Boolean | Greater than |
| `\| gte(val)` | Boolean | Greater than or equal |
| `\| lt(val)` | Boolean | Less than |
| `\| lte(val)` | Boolean | Less than or equal |

### Formatting Transforms

Terminal transforms for output formatting.

| Transform | Output | Description |
|-----------|--------|-------------|
| `\| quote(single)` | Quoted | Single quotes (default): `'val1','val2'` |
| `\| quote(double)` | Quoted | Double quotes: `"val1","val2"` |
| `\| quote(verbatim)` | Quoted | KQL verbatim: `@'val1',@'val2'` |

---

## Transform Chains

### Filter to Column Selection

```
{{step | filter(score > 50).column | array}}
{{step | filter(active == true).IPAddress | first}}
{{step | filter(country == 'US') | length}}
```

### Deduplication

```
{{step.column | unique | array}}
{{step.column | unique | str_join(',')}}
```

### Comparison Chains

```
{{step | length | gt(5)}}
{{step.column | first | eq('expected')}}
{{step | filter(score > 90) | length | gte(3)}}
```

---

## Input Types

Inputs are defined with explicit types in pack definitions.

### Pack Definition

```yaml
inputs:
  target_user:
    type: string
    description: "Single user to investigate"

  ip_addresses:
    type: array
    description: "List of IPs to check"
```

### Usage

Inputs use the same transform syntax as step columns:

```
{{inputs.target_user}}                    # Scalar (string input)
{{inputs.ip_addresses | array}}           # Quoted array
{{inputs.ip_addresses | first}}           # First element
{{inputs.ip_addresses | str_join(',')}}   # Joined string
{{inputs.ip_addresses | for_each}}        # Iteration (HTTP only)
```

---

## Condition Syntax

Conditions in `when:` clauses must evaluate to boolean.

### Simple Conditions

```yaml
when: "{{step | is_empty}}"
when: "{{step | is_not_empty}}"
when: "{{step | length | gt(5)}}"
when: "{{step | any(score > 90)}}"
when: "{{step | all(active == true)}}"
```

### Field Comparisons

```yaml
when: "{{step.field | first | eq('expected')}}"
when: "{{step.score | first | gt(80)}}"
```

### Compound Conditions

Boolean operators (`and`, `or`, `not`) combine conditions outside `{{}}`:

```yaml
when: "{{step | is_not_empty}} and {{step | any(score > 50)}}"
when: "{{step1 | is_empty}} or {{step2 | is_empty}}"
when: "not {{step | is_empty}}"
```

---

## For-Each Iteration

The `for_each` transform triggers iteration for HTTP steps only.

### Behavior

1. Substitution builder identifies `| for_each` in request
2. Extracts all values from the source column
3. Yields one substituted request per value
4. Results aggregate automatically (append by default)

### Example

```yaml
- name: enrich_ips
  type: http
  depends_on: [suspicious_ips]
  request:
    url: "https://api.threatintel.com/check"
    params:
      ip: "{{suspicious_ips.IPAddress | for_each}}"
    headers:
      Authorization: "Bearer {{secrets.api_key}}"
```

If `suspicious_ips` contains 3 IPs, this generates 3 API calls with results appended.

### Restrictions

- **HTTP only** - KQL steps cannot use `for_each`
- **Single iterator** - Only one `for_each` per step
- **No branching** - Downstream steps see aggregated results

### Aggregation

Results aggregate using the step's `aggregate` setting:

| Strategy | Behavior |
|----------|----------|
| `append` (default) | Concatenate all iteration results |
| `merge` | Deep merge result objects |
| `replace` | Keep only last iteration |
| `collect` | Wrap each with `_iteration`, `_source_row` metadata |

---

## Predicate Syntax

Predicates in `any()`, `all()`, and `filter()` use field comparisons:

```
field == value     # Equality
field != value     # Inequality
field > value      # Greater than
field >= value     # Greater than or equal
field < value      # Less than
field <= value     # Less than or equal
```

### Value Types

```
field == true           # Boolean
field == false          # Boolean
field == 123            # Number
field == 'string'       # String (quoted)
field == string         # String (unquoted)
```

### Examples

```yaml
# Any row has score above 90
when: "{{step | any(score > 90)}}"

# All rows are active
when: "{{step | all(active == true)}}"

# Filter to high-risk, then count
when: "{{step | filter(risk_level == 'high') | length | gt(0)}}"
```

---

## Validation Rules

### Context Requirements

| Context | Allowed Output Types |
|---------|---------------------|
| KQL query | Scalar, Array |
| HTTP URL/params/body | Scalar, Array, for_each |
| `when:` clause | Boolean |
| Scoring condition | Boolean |

### Validation Errors

| Condition | Error |
|-----------|-------|
| `{{step.column}}` without transform | "Column reference requires transform (first, array, etc.)" |
| `{{step.column \| for_each}}` in KQL | "for_each not supported in KQL steps" |
| `{{inputs.string \| array}}` | "Cannot use array transform on string input" |
| Multiple `\| for_each` in step | "Only one for_each allowed per step" |
| `{{step \| array}}` without column | "array transform requires column selection" |
| Non-boolean in `when:` clause | "Condition must evaluate to boolean" |

---

## Quote Styles

Values in arrays are quoted according to the configured style:

| Style | Input | Output |
|-------|-------|--------|
| `single` (default) | `O'Brien` | `'O''Brien'` |
| `double` | `O'Brien` | `"O'Brien"` |
| `verbatim` | `O'Brien` | `@'O'Brien'` |

### Usage

```yaml
# Default single quotes
query: |
  SigninLogs
  | where UserPrincipalName in ({{users.Email | array}})
  # Result: in ('alice@example.com','bob@example.com')

# Explicit double quotes for JSON
body: |
  {"users": [{{users.Email | array | quote(double)}}]}
  # Result: {"users": ["alice@example.com","bob@example.com"]}
```

---

## Complete Examples

### Conditional Step Execution

```yaml
steps:
  - name: initial_search
    type: kql
    query: |
      SigninLogs
      | where ResultType != 0
      | take 100

  - name: enrich_users
    type: kql
    depends_on: [initial_search]
    when: "{{initial_search | is_not_empty}}"
    query: |
      AADUserRiskEvents
      | where UserPrincipalName in ({{initial_search.UserPrincipalName | array}})

  - name: high_risk_check
    type: kql
    depends_on: [enrich_users]
    when: "{{enrich_users | any(RiskLevel == 'High')}}"
    query: |
      SecurityIncident
      | where RelatedUsers has_any ({{enrich_users.UserPrincipalName | unique | array}})
```

### HTTP Iteration with Enrichment

```yaml
steps:
  - name: suspicious_ips
    type: kql
    query: |
      SigninLogs
      | where FailedAttempts >= {{inputs.min_failures | first}}
      | distinct IPAddress

  - name: threat_enrichment
    type: http
    depends_on: [suspicious_ips]
    when: "{{suspicious_ips | is_not_empty}}"
    request:
      url: "https://api.abuseipdb.com/api/v2/check"
      params:
        ipAddress: "{{suspicious_ips.IPAddress | for_each}}"
      headers:
        Key: "{{secrets.abuseipdb_key}}"

  - name: correlate
    type: kql
    depends_on: [suspicious_ips, threat_enrichment]
    when: "{{threat_enrichment | any(abuseConfidenceScore > 50)}}"
    query: |
      let malicious = dynamic([{{threat_enrichment | filter(abuseConfidenceScore > 50).IPAddress | array}}]);
      SigninLogs
      | where IPAddress in (malicious)
```

### Verdict Rules

```yaml
report:
  verdict_rules:
    - name: critical_threat
      condition: "{{threats | any(Severity == 'Critical')}}"
      level: "CRITICAL"
      summary: "Critical threat detected"

    - name: suspicious_activity
      condition: "{{failed_logins | length | gt(10)}} and {{failed_logins | any(FromUnknownIP == true)}}"
      level: "HIGH"
      summary: "Suspicious login pattern"

    - name: elevated_risk
      condition: "{{enrichment | filter(risk_score > 75) | length | gt(0)}}"
      level: "MEDIUM"
      summary: "Elevated risk indicators found"

    - name: no_issues
      condition: "true"
      level: "LOW"
      summary: "No significant findings"
```

### Scoring Indicators

```yaml
processing:
  - name: risk_scoring
    type: scoring
    indicators:
      - name: high_failure_rate
        condition: "{{login_analysis | any(failure_rate > 0.5)}}"
        weight: 30
        description: "User has high login failure rate"

      - name: impossible_travel
        condition: "{{geo_analysis | any(is_impossible_travel == true)}}"
        weight: 50
        description: "Impossible travel detected"

      - name: known_bad_ip
        condition: "{{threat_enrichment | filter(threat_score > 80) | length | gt(0)}}"
        weight: 40
        description: "Connection from known malicious IP"
```

---

## Migration from Previous Syntax

| Old Syntax | New Syntax |
|------------|------------|
| `{{step.*.column}}` | `{{step.column \| array}}` |
| `{{step.first.column}}` | `{{step.column \| first}}` |
| `{{step[N].column}}` | `{{step.column \| at(N)}}` |
| `{{alias.column}}` (foreach) | `{{step.column \| for_each}}` in HTTP |
| `foreach: "step as alias"` | `\| for_each` transform |
| `step is empty` | `{{step \| is_empty}}` |
| `step is not empty` | `{{step \| is_not_empty}}` |
| `step.length == N` | `{{step \| length \| eq(N)}}` |
| `step.any(field > val)` | `{{step \| any(field > val)}}` |
| `step.all(field == val)` | `{{step \| all(field == val)}}` |
| `step.field == val` | `{{step.field \| first \| eq(val)}}` |

---

## Implementation Notes

### SubstitutionBuilder

The `SubstitutionBuilder` handles parsing and expansion:

1. Parse all `{{...}}` references in source text
2. Identify transform pipelines for each reference
3. If `for_each` present, yield iterator of substituted variants
4. Otherwise, substitute all references and return single result

### Polars Integration

Transforms leverage Polars DataFrame operations:
- `unique` → `Series::unique()`
- `filter` → `DataFrame::filter()`
- `str_join` → `Series::str().join()`
- Array extraction → `Series::to_list()`

### Type System

```rust
enum TransformResult {
    Scalar(String),           // Single value
    Array(Vec<String>),       // Multiple values, apply quote style
    Boolean(bool),            // For conditions
    Iterator(RowIterator),    // For for_each expansion
}
```

---

## Error Handling

- **Unknown variables**: Substitution returns error if reference cannot be resolved
- **Invalid transforms**: Validation errors for incompatible transform chains
- **Missing steps**: References to non-existent steps return empty results
- **Type mismatches**: Comparisons attempt coercion; incompatible types return false
