# Transform Reference

Complete reference for all available transforms in the variable syntax.

## Step-Level Transforms

Operate on the entire step result without column selection.

| Transform | Output | Description |
|-----------|--------|-------------|
| `is_empty` | Boolean | True if step has no rows |
| `is_not_empty` | Boolean | True if step has rows |
| `length` | Integer | Row count |
| `any(predicate)` | Boolean | True if any row matches predicate |
| `all(predicate)` | Boolean | True if all rows match predicate |
| `filter(predicate)` | Filtered step | Filter rows, chain with `.column` |

### Examples

```yaml
# Check if step has results
when: "{{signins | is_not_empty}}"

# Count rows
when: "{{alerts | length | gt(10)}}"

# Check for matching rows
when: "{{signins | any(FailedCount > 5)}}"

# Filter then select column
{{signins | filter(RiskLevel == 'High').UserPrincipalName | array}}
```

## Column Transforms

Operate after column selection (`{{step.column | ...}}`).

| Transform | Output | Description |
|-----------|--------|-------------|
| `first` | Scalar | First row's value |
| `at(N)` | Scalar | Nth row's value (0-indexed) |
| `array` | Array | All values as comma-separated quoted list |
| `unique` | Column | Deduplicated values (chain with `array` or `str_join`) |
| `str_join(sep)` | Scalar | Values joined with separator |
| `for_each` | Iterator | Iteration trigger (HTTP steps only) |

### Examples

```yaml
# Get first value
let user = "{{signins.UserPrincipalName | first}}";

# Get specific row
let third_ip = "{{ips.IPAddress | at(2)}}";

# All values as array
| where IPAddress in ({{ips.IPAddress | array}})
# Result: in ('10.0.0.1','192.168.1.1','8.8.8.8')

# Deduplicated array
| where User in ({{users.Email | unique | array}})

# Join with custom separator
let users = "{{signins.UserPrincipalName | str_join('; ')}}";

# HTTP iteration
params:
  ip: "{{suspicious_ips.IPAddress | for_each}}"
```

## Comparison Transforms

Convert values to boolean for conditions. Chain after `length` or scalar values.

| Transform | Output | Description |
|-----------|--------|-------------|
| `eq(value)` | Boolean | Equals |
| `neq(value)` | Boolean | Not equals |
| `gt(value)` | Boolean | Greater than |
| `gte(value)` | Boolean | Greater than or equal |
| `lt(value)` | Boolean | Less than |
| `lte(value)` | Boolean | Less than or equal |

### Examples

```yaml
# Compare row count
when: "{{alerts | length | gt(5)}}"

# Compare first value
when: "{{config.status | first | eq('active')}}"

# Chain with filter
when: "{{signins | filter(Failed == true) | length | gte(3)}}"
```

## Formatting Transforms

Control output formatting for arrays. Terminal transforms applied after `array`.

| Transform | Output | Description |
|-----------|--------|-------------|
| `quote(single)` | Array | Single quotes: `'val1','val2'` |
| `quote(double)` | Array | Double quotes: `"val1","val2"` |
| `quote(verbatim)` | Array | KQL verbatim: `@'val1',@'val2'` |

### Examples

```yaml
# Default single quotes (KQL standard)
| where User in ({{users.Email | array}})
# Result: in ('alice@example.com','bob@example.com')

# Double quotes for JSON
body: |
  {"users": [{{users.Email | array | quote(double)}}]}
# Result: {"users": ["alice@example.com","bob@example.com"]}

# KQL verbatim for special characters
| where Path in ({{paths.FilePath | array | quote(verbatim)}})
# Result: in (@'C:\Users\file.txt',@'D:\Data\log.csv')
```

## Transform Chains

Transforms can be chained to build complex operations:

```yaml
# Dedupe, then array
{{step.column | unique | array}}

# Filter, select column, get first
{{step | filter(active == true).column | first}}

# Filter, count, compare
{{step | filter(score > 90) | length | gte(3)}}

# Column, unique, join
{{step.column | unique | str_join(',')}}
```

## Type Flow

Each transform has an expected input and output type:

```
Step ─── is_empty ──────► Boolean
     ├── is_not_empty ──► Boolean
     ├── length ────────► Integer ─── eq/gt/lt/etc ──► Boolean
     ├── any(pred) ─────► Boolean
     ├── all(pred) ─────► Boolean
     └── filter(pred) ──► FilteredStep ─── .column ──► Column

Column ─── first ───────► Scalar ─── eq/gt/lt/etc ──► Boolean
       ├── at(N) ───────► Scalar
       ├── array ───────► Array ─── quote(style) ──► Array
       ├── unique ──────► Column (chain with array/str_join)
       ├── str_join(s) ─► Scalar
       └── for_each ────► Iterator (HTTP only)
```

## Related

- [Variable Syntax](variable-syntax.md) - Overview and usage patterns
- [Predicates](predicates.md) - Filtering syntax for `any()`, `all()`, `filter()`
