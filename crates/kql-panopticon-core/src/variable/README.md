# Variable Module

This module provides variable substitution and condition evaluation for pack execution.

## Variable Substitution

Variables use the `{{...}}` syntax and are replaced with actual values during execution.

### Syntax Reference

| Syntax | Description | Example |
|--------|-------------|---------|
| `{{inputs.name}}` | User-provided input value | `{{inputs.target_user}}` |
| `{{secrets.name}}` | Environment variable secret | `{{secrets.api_key}}` |
| `{{step.*.column}}` | All values from a column (array) | `{{users.*.Email}}` |
| `{{step.first.column}}` | First row's column value | `{{users.first.Id}}` |
| `{{step[N].column}}` | Nth row's column value (0-indexed) | `{{users[0].Name}}` |
| `{{alias.column}}` | Current row in foreach iteration | `{{u.Email}}` |

### Array Substitution

When using `{{step.*.column}}`, all values are extracted and formatted as a comma-separated list with quotes:

```yaml
# If 'users' step returns rows with Emails: ["alice@example.com", "bob@example.com"]
query: |
  SigninLogs
  | where UserPrincipalName in ({{users.*.Email}})
# Result: ... in ('alice@example.com','bob@example.com')
```

### Quote Styles

Values are quoted according to the configured quote style:

| Style | Output | Use Case |
|-------|--------|----------|
| `single` (default) | `'value'` | KQL string literals |
| `double` | `"value"` | JSON contexts |
| `verbatim` | `@'value'` | KQL verbatim strings |

Special characters are escaped appropriately (e.g., `O'Brien` becomes `'O''Brien'` in single-quote style).

### Examples

```yaml
# User input substitution
inputs:
  - name: target_email
    required: true

steps:
  - name: find_user
    query: |
      SigninLogs
      | where UserPrincipalName == '{{inputs.target_email}}'
      | take 1

  # Use result from previous step
  - name: find_related
    depends_on: [find_user]
    query: |
      SigninLogs
      | where IPAddress == '{{find_user.first.IPAddress}}'
```

---

## Condition Evaluation

Conditions are used in `when` clauses to control step execution, and in verdict rules and scoring indicators.

### Quick Reference

| Condition | Description | Example |
|-----------|-------------|---------|
| `true` / `false` | Literal boolean | `when: "true"` |
| `step is empty` | Step has no results | `when: "users is empty"` |
| `step is not empty` | Step has results | `when: "users is not empty"` |
| `step.length == N` | Row count comparison | `when: "users.length > 0"` |
| `step.first.field == value` | First row field comparison | `when: "users.first.Active == true"` |
| `step.field == value` | First row (shorthand) | `when: "users.Active == true"` |
| `step[N].field == value` | Indexed row comparison | `when: "users[0].Score > 80"` |
| `step.any(field op value)` | Any row matches | `when: "users.any(Score > 90)"` |
| `step.all(field op value)` | All rows match | `when: "users.all(Active == true)"` |

### Boolean Operators

Combine conditions with `and`, `or`, and `not`:

```yaml
# Both conditions must be true
when: "logins is not empty and logins.any(Failed == true)"

# Either condition
when: "high_risk is not empty or medium_risk is not empty"

# Negation
when: "not users is empty"
```

### Comparison Operators

| Operator | Description |
|----------|-------------|
| `==` | Equal |
| `!=` | Not equal |
| `>` | Greater than |
| `<` | Less than |
| `>=` | Greater than or equal |
| `<=` | Less than or equal |

### Value Types

Conditions handle type coercion automatically:

```yaml
# Boolean comparison
when: "step.field == true"
when: "step.field == false"

# Numeric comparison
when: "step.Score > 85"
when: "step.Count >= 100"

# String comparison (quotes optional)
when: "step.Status == Active"
when: "step.Domain == 'microsoft.com'"
```

### Empty vs Length

Two equivalent ways to check for results:

```yaml
# Empty syntax (cleaner)
when: "users is empty"
when: "users is not empty"

# Length syntax (more flexible)
when: "users.length == 0"
when: "users.length > 0"
when: "users.length >= 5"
```

### First Row Syntax

The `step.first.field` syntax aligns with variable substitution `{{step.first.field}}`:

```yaml
# Condition
when: "login.first.RiskLevel == High"

# Corresponding variable substitution
query: |
  SecurityEvents
  | where RiskLevel == '{{login.first.RiskLevel}}'
```

### Predicates: any() and all()

Check conditions across all rows:

```yaml
# True if ANY row matches
when: "users.any(FailedLogins > 10)"

# True if ALL rows match (and at least one row exists)
when: "users.all(Verified == true)"

# Combined
when: "suspicious_ips is not empty and suspicious_ips.any(ThreatScore > 80)"
```

Note: `all()` returns `false` for empty result sets.

---

## Complete Examples

### Conditional Step Execution

```yaml
steps:
  - name: initial_search
    query: |
      SigninLogs
      | where ResultType != 0
      | take 100

  - name: enrich_users
    depends_on: [initial_search]
    # Only run if we found failed logins
    when: "initial_search is not empty"
    query: |
      AADUserRiskEvents
      | where UserPrincipalName in ({{initial_search.*.UserPrincipalName}})

  - name: high_risk_check
    depends_on: [enrich_users]
    # Only if we found high-risk events
    when: "enrich_users.any(RiskLevel == High)"
    query: |
      SecurityIncident
      | where RelatedUsers has_any ({{enrich_users.*.UserPrincipalName}})
```

### Foreach Iteration

```yaml
steps:
  - name: get_users
    query: |
      IdentityInfo
      | where Department == 'Finance'

  - name: per_user_activity
    depends_on: [get_users]
    foreach: "get_users as user"
    when: "get_users is not empty"
    query: |
      SigninLogs
      | where UserPrincipalName == '{{user.UserPrincipalName}}'
      | summarize count() by ResultType
```

### Verdict Rules

```yaml
report:
  verdict_rules:
    - name: critical_threat
      condition: "threats.any(Severity == Critical)"
      level: "CRITICAL"
      summary: "Critical threat detected"

    - name: suspicious_activity
      condition: "failed_logins.length > 10 and failed_logins.any(FromUnknownIP == true)"
      level: "HIGH"
      summary: "Suspicious login pattern"

    - name: no_issues
      condition: "true"  # Default fallback
      level: "LOW"
      summary: "No significant findings"
```

---

## Error Handling

- **Unknown variables**: Substitution returns an error if a variable reference cannot be resolved
- **Unknown conditions**: Condition evaluation logs a warning and returns `false`
- **Missing steps**: References to non-existent steps return empty results (length 0)
- **Type mismatches**: Comparisons attempt type coercion; incompatible types return `false`

---

## API Reference

### Substitution

```rust
use kql_panopticon_core::variable::{substitute, SubstitutionContext};

let context = SubstitutionContext::new()
    .with_input("user", "alice@example.com")
    .with_step_results("logins", vec![
        serde_json::json!({"IP": "10.0.0.1"}),
    ]);

let result = substitute("User: {{inputs.user}}, IP: {{logins.first.IP}}", &context)?;
// Result: "User: alice@example.com, IP: '10.0.0.1'"
```

### Condition Evaluation

```rust
use kql_panopticon_core::variable::evaluate_condition;
use std::collections::HashMap;

let mut step_results = HashMap::new();
step_results.insert("users".to_string(), vec![
    serde_json::json!({"name": "Alice", "score": 85}),
]);

let should_run = evaluate_condition("users is not empty", &step_results);
assert!(should_run);

let high_score = evaluate_condition("users.any(score > 80)", &step_results);
assert!(high_score);
```
