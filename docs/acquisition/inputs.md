# Inputs

Inputs define values provided by users at runtime. They can be strings or arrays.

## Schema

```yaml
inputs:
  - name: target_user           # Required: unique identifier
    type: string                # Optional: string (default) | array
    label: Target User          # Optional: human-readable name
    description: User to check  # Optional: help text
    required: true              # Optional: default true
    default: "alice@contoso.com"  # Optional: default value
    example: "bob@contoso.com"  # Optional: for validation
```

## Fields

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `name` | Yes | - | Unique identifier used in `{{inputs.name}}` |
| `type` | No | `string` | Value type: `string` or `array` |
| `label` | No | - | Human-readable display name |
| `description` | No | - | Help text for users |
| `required` | No | `true` | Whether input must be provided |
| `default` | No | - | Default value (makes input optional) |
| `example` | No | - | Example value for pack validation |

## Input Types

### String Type

Single value input. Used directly in substitutions.

```yaml
inputs:
  - name: target_user
    type: string
    label: Target User
    required: true

# Usage in query
query: |
  SigninLogs
  | where UserPrincipalName == "{{inputs.target_user}}"
```

### Array Type

Multiple values, typically comma-separated. Requires array transforms for substitution.

```yaml
inputs:
  - name: ip_list
    type: array
    label: IP Addresses
    description: Comma-separated list of IPs
    example: "8.8.8.8,1.1.1.1"

# Usage in query
query: |
  SigninLogs
  | where IPAddress in ({{inputs.ip_list | array}})
  # Result: in ('8.8.8.8','1.1.1.1')
```

## Providing Input Values

### CLI

```bash
# String input
panopticon-cli pack.yaml --input target_user=alice@contoso.com

# Array input (comma-separated)
panopticon-cli pack.yaml --input ip_list=8.8.8.8,1.1.1.1

# JSON array format
panopticon-cli pack.yaml --input 'ip_list=["8.8.8.8","1.1.1.1"]'
```

### TUI

```
let target_user:input = alice@contoso.com
let ip_list:input = 8.8.8.8,1.1.1.1
```

## Input Value Parsing

Values are parsed with JSON support:

| Input Format | Parsed As |
|--------------|-----------|
| `"value"` (JSON string) | `value` |
| `["a","b"]` (JSON array) | `a,b` |
| `raw_value` (not JSON) | `raw_value` |

**Rejected types**: JSON numbers, booleans, null, objects

## Default Values

Inputs with defaults become optional:

```yaml
inputs:
  - name: days
    label: Lookback Days
    default: "7"            # User doesn't need to provide
    required: false         # Explicit, but redundant with default
```

## Example Values

The `example` field is used during pack validation to substitute realistic values:

```yaml
inputs:
  - name: target_user
    type: string
    required: true
    example: "alice@contoso.com"
```

This allows the pack to be validated without requiring actual input values.

## Using Inputs in Steps

```yaml
# Direct string substitution
query: |
  let user = "{{inputs.target_user}}";

# Array transforms for multiple values
query: |
  let ips = dynamic([{{inputs.ip_list | array}}]);
  SigninLogs | where IPAddress in (ips)

# In HTTP requests
url: "https://api.example.com/user/{{inputs.user_id}}"

# In conditions
when: "{{inputs.days | first | gt(7)}}"
```

## Validation

- Input names must be unique
- Input names cannot be empty
- Required inputs without defaults must be provided

## Related

- [Variable Syntax](../reference/variable-syntax.md) - How to use inputs in steps
- [Transform Reference](../reference/transforms.md) - Available transforms for inputs
