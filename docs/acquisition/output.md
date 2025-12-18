# Output Configuration

Configure where acquisition results are stored.

## Schema

```yaml
acquisition:
  output:
    folder: "output/{{meta.pack_name}}/{{meta.timestamp}}"
```

## Fields

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `folder` | No | `./output` | Output folder template |

## Folder Template

The folder path supports Tera template syntax:

```yaml
output:
  folder: "investigations/{{inputs.target_user}}/{{meta.timestamp}}"
```

### Available Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `meta.timestamp` | RFC3339 timestamp | `2024-01-15T10:30:00Z` |
| `meta.pack_name` | Pack name | `IP Investigation` |
| `meta.workspace` | Workspace name | `sentinel-workspace` |
| `meta.workspace_id` | Workspace ID | `abc-123-def` |
| `meta.subscription` | Subscription name | `Production` |
| `meta.subscription_id` | Subscription ID | `xyz-789` |
| `inputs.*` | User input values | `alice@contoso.com` |

## Examples

### By Pack and Timestamp

```yaml
output:
  folder: "output/{{meta.pack_name}}/{{meta.timestamp}}"
```

Result: `output/IP Investigation/2024-01-15T10:30:00Z/`

### By User Input

```yaml
output:
  folder: "investigations/{{inputs.target_user}}"
```

Result: `investigations/alice@contoso.com/`

### By Workspace

```yaml
output:
  folder: "results/{{meta.workspace}}/{{meta.timestamp}}"
```

Result: `results/sentinel-workspace/2024-01-15T10:30:00Z/`

## Output Structure

Inside the output folder:

```
output/
└── {pack_name}/
    └── {timestamp}/
        ├── step1.jsonl        # Step results (JSON Lines)
        ├── step2.jsonl
        ├── summary.md         # Generated reports
        └── data.json
```

### JSONL Format

Each step produces a `.jsonl` file with one JSON object per row:

```json
{"UserPrincipalName":"alice@contoso.com","SigninCount":15}
{"UserPrincipalName":"bob@contoso.com","SigninCount":8}
```

## CLI Override

The CLI `--output` flag overrides the pack configuration:

```bash
panopticon-cli pack.yaml --output ./custom-output
```

## Default Behavior

If no output configuration:
- CLI: Uses `./output`
- TUI: Uses configured output directory from settings

## Related

- [CLI Usage](../cli/usage.md) - Output options
- [Reporting](../reporting/overview.md) - Report output
