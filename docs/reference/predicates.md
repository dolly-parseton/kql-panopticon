# Predicate Syntax

Predicates are used inside `any()`, `all()`, and `filter()` transforms to match rows based on field values.

## Syntax

```
field operator value
```

## Operators

| Operator | Description |
|----------|-------------|
| `==` | Equals |
| `!=` | Not equals |
| `>` | Greater than |
| `>=` | Greater than or equal |
| `<` | Less than |
| `<=` | Less than or equal |

## Value Types

| Syntax | Type | Example |
|--------|------|---------|
| `true` / `false` | Boolean | `active == true` |
| `123` / `3.14` / `-10` | Number | `score > 90` |
| `'text'` | Quoted string | `status == 'active'` |
| `text` | Unquoted string | `status == active` |

## Examples

### Basic Comparisons

```yaml
# Numeric comparison
when: "{{signins | any(FailedCount > 5)}}"

# String comparison (quoted)
when: "{{alerts | any(Severity == 'High')}}"

# String comparison (unquoted)
when: "{{alerts | any(Severity == High)}}"

# Boolean comparison
when: "{{ips | any(IsMalicious == true)}}"
```

### Using `any()`

Returns true if any row matches the predicate:

```yaml
# Any row has score above 90
when: "{{enrichment | any(threat_score > 90)}}"

# Any row is marked suspicious
when: "{{activity | any(is_suspicious == true)}}"

# Any row from specific country
when: "{{signins | any(Country == 'RU')}}"
```

### Using `all()`

Returns true if all rows match the predicate:

```yaml
# All rows are active
when: "{{users | all(status == 'active')}}"

# All scores below threshold
when: "{{results | all(risk_score < 50)}}"
```

### Using `filter()`

Reduces the result set to matching rows, then chain with column selection:

```yaml
# Filter to high-risk, select IPs
{{enrichment | filter(score > 80).IPAddress | array}}

# Filter active users, count
{{users | filter(active == true) | length}}

# Filter and check if any remain
when: "{{alerts | filter(Severity == 'Critical') | is_not_empty}}"

# Complex: filter, select column, get first
{{signins | filter(RiskLevel == 'High').UserPrincipalName | first}}
```

## Type Coercion

The predicate evaluator performs intelligent type coercion:

- String `"85"` compared to Number `85` - works
- Number `100` compared to String `"100"` - works
- Null values: equality and inequality comparisons work

## Compound Conditions

For multiple conditions, use boolean operators outside the `{{}}` blocks:

```yaml
# AND
when: "{{step | any(score > 50)}} and {{step | any(active == true)}}"

# OR
when: "{{step1 | is_empty}} or {{step2 | is_empty}}"

# NOT
when: "not {{step | is_empty}}"

# Combined
when: "{{alerts | any(Severity == 'High')}} and not {{resolved | is_not_empty}}"
```

## In Scoring Indicators

Predicates are commonly used in scoring conditions:

```yaml
processing:
  steps:
    - name: risk_assessment
      type: scoring
      indicators:
        - name: high_failure_rate
          condition: "{{signins | any(FailureRate > 0.5)}}"
          weight: 30

        - name: impossible_travel
          condition: "{{geo | any(is_impossible_travel == true)}}"
          weight: 50

        - name: known_malicious_ip
          condition: "{{enrichment | filter(threat_score > 80) | length | gt(0)}}"
          weight: 40
```

## Related

- [Variable Syntax](variable-syntax.md) - Overview and usage patterns
- [Transform Reference](transforms.md) - Complete list of transforms
