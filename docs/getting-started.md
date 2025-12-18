# Getting Started

This guide walks you through setting up KQL Panopticon and running your first pack.

## Prerequisites

### Azure CLI

Install and authenticate with Azure:

```bash
# Install Azure CLI
# macOS
brew install azure-cli

# Windows
winget install Microsoft.AzureCLI

# Linux
curl -sL https://aka.ms/InstallAzureCLIDeb | sudo bash

# Authenticate
az login
```

### Rust Toolchain (for building)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify
rustc --version
cargo --version
```

## Installation

### Build from Source

```bash
# Clone repository
git clone https://github.com/your-org/kql-panopticon.git
cd kql-panopticon

# Build release binaries
cargo build --release

# Binaries are in target/release/
./target/release/panopticon-cli --help
./target/release/panopticon-tui --help
```

### Add to PATH (optional)

```bash
# Add to ~/.bashrc or ~/.zshrc
export PATH="$PATH:/path/to/kql-panopticon/target/release"
```

## First Pack

Create a simple pack to query Usage data:

### 1. Create Pack File

Create `my-first-pack.yaml`:

```yaml
name: "My First Pack"
description: "Query workspace usage data"
version: "1.0"

acquisition:
  inputs:
    - name: days
      label: Lookback Days
      default: "7"
      description: Number of days to query

  steps:
    - name: usage
      type: kql
      query: |
        Usage
        | where TimeGenerated > ago({{inputs.days}}d)
        | summarize TotalGB = sum(Quantity) / 1024 by DataType
        | order by TotalGB desc
        | take 10
      timespan: P30D

reporting:
  reports:
    - name: summary
      format: markdown
      output: "usage-report.md"
      template: |
        # Workspace Usage Report

        **Generated**: {{ meta.timestamp }}
        **Workspace**: {{ meta.workspace }}

        ## Top Data Types (Last {{ inputs.days }} Days)

        | Data Type | Size (GB) |
        |-----------|-----------|
        {% for row in usage %}
        | {{ row.DataType }} | {{ row.TotalGB | round(precision=2) }} |
        {% endfor %}
```

### 2. Validate Pack

```bash
panopticon-cli my-first-pack.yaml --dry-run
```

### 3. List Workspaces

```bash
panopticon-cli my-first-pack.yaml --list-workspaces
```

### 4. Run Pack

```bash
# Run against all workspaces
panopticon-cli my-first-pack.yaml

# Or specific workspace
panopticon-cli my-first-pack.yaml -w your-workspace-name

# With custom lookback
panopticon-cli my-first-pack.yaml -i days=14
```

### 5. View Results

```bash
cat output/My\ First\ Pack/your-workspace-name/*/usage-report.md
```

## Interactive Mode

Use the TUI for interactive exploration:

```bash
# Start TUI
panopticon-tui

# In TUI:
:ws                    # Select workspaces
:load                  # Load your pack
let days:input = 14    # Set input
:run                   # Execute
```

## Example Packs

Explore included examples:

```
examples/
├── test-pack/              # Folder-based pack (uses datatable)
├── ip-investigation.yaml   # Single-file pack
└── variable-syntax-demo/   # Variable syntax examples
```

Run the test pack (works without real data):

```bash
panopticon-cli examples/test-pack
```

## Next Steps

### Learn Pack Structure

- [Pack Overview](packs/overview.md) - Three-phase model
- [Single-File Packs](packs/single-file.md) - Simple format
- [Folder-Based Packs](packs/folder-based.md) - Modular organization

### Understand Variable Syntax

- [Variable Syntax](reference/variable-syntax.md) - Pipe-based transforms
- [Transform Reference](reference/transforms.md) - Available transforms
- [Predicates](reference/predicates.md) - Filtering rows

### Build Acquisition Steps

- [KQL Steps](acquisition/steps/kql.md) - Query execution
- [HTTP Steps](acquisition/steps/http.md) - API enrichment
- [Inputs](acquisition/inputs.md) - User-provided values

### Add Processing and Reporting

- [Scoring](processing/scoring.md) - Risk assessment
- [Templates](reporting/templates.md) - Report generation

## Troubleshooting

### "No workspaces found"

Verify Azure authentication and permissions:

```bash
az account show
az monitor log-analytics workspace list
```

### "Pack validation failed"

Check pack syntax:

```bash
panopticon-cli pack.yaml --dry-run --verbose
```

### "Step failed: Query timeout"

Reduce query scope or increase timespan:

```yaml
- name: large_query
  query: |
    SigninLogs
    | where TimeGenerated > ago(1d)  # Reduce scope
    | take 1000
  timespan: P1D
```

## Getting Help

- Check the [documentation](README.md)
- Review [example packs](../examples/)
- Report issues at the project repository
