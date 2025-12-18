# CLI Usage

The CLI tool runs packs against Azure Log Analytics workspaces in batch mode.

## Basic Command

```bash
panopticon-cli <PACK_PATH> [OPTIONS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `PACK_PATH` | Path to pack file (YAML) or folder |

## Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--workspace` | `-w` | all | Workspace filter (comma-separated) |
| `--output` | `-o` | `./output` | Output directory |
| `--input` | `-i` | - | Input values (repeatable) |
| `--verbose` | `-v` | false | Enable debug logging |
| `--list-workspaces` | - | - | List available workspaces and exit |
| `--dry-run` | - | - | Validate pack without executing |

## Prerequisites

**Azure CLI Authentication**

```bash
az login
```

The CLI uses Azure CLI credentials to access workspaces.

## Examples

### Run Against All Workspaces

```bash
panopticon-cli pack.yaml
```

### Specific Workspace

```bash
panopticon-cli pack.yaml -w sentinel-workspace
```

### Multiple Workspaces

```bash
panopticon-cli pack.yaml -w workspace1,workspace2,workspace3
```

### With Input Values

```bash
# String input
panopticon-cli pack.yaml -i target_user=alice@contoso.com

# Multiple inputs
panopticon-cli pack.yaml -i target_user=alice@contoso.com -i days=14

# Array input (JSON format)
panopticon-cli pack.yaml -i 'ip_list=["8.8.8.8","1.1.1.1"]'

# Array input (comma-separated)
panopticon-cli pack.yaml -i ip_list=8.8.8.8,1.1.1.1
```

### Custom Output Directory

```bash
panopticon-cli pack.yaml -o ./results
```

### Dry Run (Validation Only)

```bash
panopticon-cli pack.yaml --dry-run
```

Validates:
- Pack structure
- Input definitions
- Step configurations
- Dependencies (no circular references)

Does not execute queries or connect to workspaces.

### List Available Workspaces

```bash
panopticon-cli pack.yaml --list-workspaces
```

### Verbose Mode

```bash
panopticon-cli pack.yaml -v
```

Enables DEBUG level logging to see detailed execution.

## Output Structure

```
output/
└── {pack_name}/
    └── {workspace}/
        └── {timestamp}/
            ├── step1.jsonl        # Step results
            ├── step2.jsonl
            ├── summary.md         # Generated reports
            └── data.json
```

### JSONL Format

Each step produces a `.jsonl` file with one JSON object per line:

```json
{"UserPrincipalName":"alice@contoso.com","SigninCount":15}
{"UserPrincipalName":"bob@contoso.com","SigninCount":8}
```

## Execution Output

The CLI displays execution progress:

```
KQL Panopticon CLI v0.4.0

Authenticating with Azure...
Discovering workspaces...

Found 3 workspaces:
  - sentinel-prod (subscription: Production)
  - sentinel-dev (subscription: Development)
  - sentinel-test (subscription: Testing)

Running pack: IP Investigation
  Against: sentinel-prod

Execution Results:
  [OK] sentinel-prod
    + signin_activity (15 rows, 1.2s)
    + affected_users (8 rows, 0.8s)
    + signin_timeline (100 rows, 1.5s)

Risk Assessment:
  Score: 65
  Level: HIGH

Reports generated:
  - output/IP Investigation/sentinel-prod/2024-01-15T10:30:00Z/summary.md
```

## Status Icons

| Icon | Meaning |
|------|---------|
| `[OK]` | Success |
| `[FAIL]` | Failed |
| `[PARTIAL]` | Some steps failed |
| `+` | Step succeeded |
| `x` | Step failed |
| `-` | Step skipped |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Execution failed |

## Logging

- **Console**: INFO level (DEBUG with `-v`)
- **File**: Trace logs written to disk (location shown at startup)

## Workflow Example

```bash
# 1. Validate pack
panopticon-cli investigation.yaml --dry-run

# 2. List workspaces
panopticon-cli investigation.yaml --list-workspaces

# 3. Run against specific workspace
panopticon-cli investigation.yaml \
  -w sentinel-prod \
  -i target_user=alice@contoso.com \
  -i days=14 \
  -o ./investigations

# 4. Check results
cat ./investigations/IP\ Investigation/sentinel-prod/*/summary.md
```

## Related

- [Pack Overview](../packs/overview.md) - Pack structure
- [Inputs](../acquisition/inputs.md) - Input configuration
- [TUI Usage](../tui/usage.md) - Interactive mode
