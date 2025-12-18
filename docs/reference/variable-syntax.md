# Variable Syntax

KQL Panopticon uses a pipe-based variable syntax for value substitution and condition evaluation throughout pack definitions.

## Syntax Overview

All variables follow the pattern:

```
{{source | transform | transform | ...}}
```

Variables can be used in:
- KQL queries
- HTTP request URLs, parameters, headers, and bodies
- `when:` conditional clauses
- Scoring indicator conditions
- Report templates

## Sources

| Syntax | Description |
|--------|-------------|
| `{{inputs.name}}` | User-provided input value |
| `{{secrets.name}}` | Environment variable secret |
| `{{step}}` | Step result (for step-level transforms) |
| `{{step.column}}` | Column from step result |

### Input Sources

Inputs are values provided by users at runtime:

```yaml
# In queries
query: |
  SigninLogs
  | where UserPrincipalName == "{{inputs.target_user}}"

# Array inputs require transforms
query: |
  SigninLogs
  | where IPAddress in ({{inputs.ip_list | array}})
```

### Secret Sources

Secrets reference environment variables:

```yaml
headers:
  Authorization: "Bearer {{secrets.api_key}}"
```

### Step Sources

Step results from previous acquisition steps:

```yaml
# Step-level: operates on entire result
when: "{{signins | is_not_empty}}"

# Column-level: selects specific column
query: |
  AADUserRiskEvents
  | where UserPrincipalName in ({{signins.UserPrincipalName | array}})
```

## Transform Chains

Transforms are chained with the `|` operator and evaluated left to right:

```yaml
# Select column, dedupe, convert to array
{{step.column | unique | array}}

# Filter rows, select column, get first value
{{step | filter(score > 50).column | first}}

# Count rows and compare
{{step | length | gt(5)}}
```

## Context Rules

Different contexts allow different output types:

| Context | Allowed Output |
|---------|---------------|
| KQL query | Scalar, Array |
| HTTP request | Scalar, Array, Iterator |
| `when:` clause | Boolean |
| Scoring condition | Boolean |

### KQL Queries

```yaml
# Scalar substitution
query: |
  let user = "{{inputs.target_user}}";

# Array substitution
query: |
  SigninLogs
  | where IPAddress in ({{ips.IPAddress | array}})
```

### HTTP Requests

```yaml
# Scalar in URL
url: "https://api.example.com/user/{{inputs.user_id}}"

# for_each iteration (one request per value)
params:
  ip: "{{suspicious_ips.IPAddress | for_each}}"
```

### Conditions

```yaml
# Step-level checks
when: "{{step | is_not_empty}}"
when: "{{step | any(score > 90)}}"

# Compound conditions
when: "{{step1 | is_empty}} or {{step2 | is_empty}}"
when: "{{step | is_not_empty}} and {{step | any(active == true)}}"
when: "not {{step | is_empty}}"
```

## Common Patterns

### Conditional Step Execution

```yaml
- name: enrich_users
  depends_on: [initial_search]
  when: "{{initial_search | is_not_empty}}"
  query: |
    AADUserRiskEvents
    | where UserPrincipalName in ({{initial_search.UserPrincipalName | array}})
```

### Filtered Column Selection

```yaml
# Filter to high-risk rows, then select column
query: |
  let malicious = dynamic([{{enrichment | filter(score > 80).IPAddress | array}}]);
```

### HTTP Iteration

```yaml
- name: threat_enrichment
  type: http
  request:
    url: "https://api.threatintel.com/check"
    params:
      ip: "{{suspicious_ips.IPAddress | for_each}}"
```

### Scoring Conditions

```yaml
indicators:
  - name: high_risk_ips
    condition: "{{enrichment | any(threat_score > 80)}}"
    weight: 40
```

## Related

- [Transform Reference](transforms.md) - Complete list of available transforms
- [Predicates](predicates.md) - Row filtering syntax for `any()`, `all()`, `filter()`
