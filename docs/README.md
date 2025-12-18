# kql-panopticon Documentation

KQL Panopticon is a pack-based query execution framework for Azure Log Analytics. Packs define data collection, processing, and reporting workflows that run against one or more workspaces.

## Quick Links

- [Getting Started](getting-started.md) - Installation and first pack
- [CLI Usage](cli/usage.md) - Command-line interface
- [TUI Usage](tui/usage.md) - Terminal user interface

## Pack Development

### Structure

- [Pack Overview](packs/overview.md) - Understanding pack organization
- [Single-File Packs](packs/single-file.md) - Simple YAML format
- [Folder-Based Packs](packs/folder-based.md) - Modular organization

### Phases

**[Acquisition](acquisition/overview.md)** - Data collection

- [Inputs](acquisition/inputs.md) - User-provided values
- [Secrets](acquisition/secrets.md) - Environment variable references
- [KQL Steps](acquisition/steps/kql.md) - KQL query execution
- [HTTP Steps](acquisition/steps/http.md) - API calls with iteration
- [File Steps](acquisition/steps/file.md) - Local file data sources
- [Step Options](acquisition/options.md) - Dependencies, conditions, error handling
- [Output](acquisition/output.md) - Output folder configuration

**[Processing](processing/scoring.md)** - Risk scoring with indicators and thresholds

**[Reporting](reporting/overview.md)** - Report generation

- [Templates](reporting/templates.md) - Tera templating reference

## Reference

- [Variable Syntax](reference/variable-syntax.md) - Pipe-based transform syntax
- [Transform Reference](reference/transforms.md) - Complete transform table
- [Predicates](reference/predicates.md) - Row filtering syntax

## Roadmap

- [Roadmap](roadmap.md) - Future development

---

*Version: 0.4.0*
