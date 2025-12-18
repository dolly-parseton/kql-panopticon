# Deprecated Examples

The examples in this folder use an older pack schema that is not compatible with the current version.

## Schema Differences

These examples use features that have been redesigned:

| Old Schema | Current Schema |
|------------|----------------|
| `kind: investigation` | Not used |
| Top-level `steps:` | `acquisition.steps:` |
| Top-level `secrets:` | `acquisition.secrets:` |
| Top-level `inputs:` | `acquisition.inputs:` |
| Top-level `output:` | `acquisition.output:` |
| `foreach: "step as alias"` | `\| for_each` transform |
| `aggregate: append` | Automatic |
| `{{alias.column}}` | `{{step.column \| for_each}}` |

## Current Examples

See the following for current pack format:

- `../test-pack/` - Folder-based pack with all phases
- `../ip-investigation.yaml` - Single-file pack with dependencies
- `../threat-enrichment.yaml` - HTTP enrichment with for_each
- `../variable-syntax-demo/` - Variable syntax examples

## Documentation

See `docs/` for complete pack format documentation.
