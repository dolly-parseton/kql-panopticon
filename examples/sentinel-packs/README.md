# Sentinel Queries Example Packs

This directory contains a script to generate KQL Panopticon query packs from the [reprise99/Sentinel-Queries](https://github.com/reprise99/Sentinel-Queries) repository.

## Overview

The `generate_packs.py` script automatically converts `.kql` files from the Sentinel-Queries repository into query pack YAML files compatible with kql-panopticon. It extracts:

- **Query descriptions** from `//` comments at the beginning of files
- **Query names** from filenames
- **Pack names** from folder names
- **Query content** from the KQL code

## Setup

### 1. Add Sentinel-Queries as a Submodule

From the repository root:

```bash
cd examples/sentinel-packs
git submodule add https://github.com/reprise99/Sentinel-Queries.git
git submodule update --init --recursive
```

### 2. Install Dependencies

The script requires Python 3.9+ and PyYAML:

```bash
pip install pyyaml
```

Or using a virtual environment:

```bash
python3 -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
pip install pyyaml
```

## Usage

### Basic Usage

Generate all query packs:

```bash
python3 generate_packs.py
```

This will:
1. Read all `.kql` files from the `Sentinel-Queries/` submodule
2. Group queries by folder
3. Generate query pack YAML files in `generated-packs/`

### Custom Paths

Specify custom source and output directories:

```bash
python3 generate_packs.py /path/to/Sentinel-Queries /path/to/output
```

### Example Output

For a folder like `Azure Active Directory/` containing multiple `.kql` files, the script generates:

**File:** `generated-packs/azure-active-directory.yaml`

```yaml
name: Azure Active Directory
description: Queries from Azure Active Directory category
author: reprise99/Sentinel-Queries
queries:
  - name: AuditLogs-ConditionalAccessChanges
    description: Track changes to conditional access policies in Azure AD
    query: |
      AuditLogs
      | where OperationName contains "Conditional Access"
      | project TimeGenerated, Identity, OperationName, ResultDescription

  - name: SigninLogs-FailedLogins
    description: Detect failed login attempts across the tenant
    query: |
      SigninLogs
      | where ResultType != 0
      | summarize FailureCount=count() by UserPrincipalName
```

## Using Generated Packs

### Copy to kql-panopticon Library

```bash
# Create packs directory if it doesn't exist
mkdir -p ~/.kql-panopticon/packs/sentinel

# Copy generated packs
cp generated-packs/*.yaml ~/.kql-panopticon/packs/sentinel/
```

### Execute from CLI

```bash
# Run a specific pack
kql-panopticon run-pack sentinel/azure-active-directory.yaml

# Validate before running
kql-panopticon run-pack sentinel/azure-active-directory.yaml --validate-only
```

### Use in TUI

1. Launch kql-panopticon TUI: `kql-panopticon`
2. Switch to the **Packs** tab (Tab 6)
3. Navigate to your pack and press **Enter** to load
4. Press **e** to execute the entire pack

## Query Pack Format

### Single Query Pack

When a folder contains only one `.kql` file:

```yaml
name: DNS Query Pack
description: Encoded DNS traffic detection
author: reprise99/Sentinel-Queries
query: |
  DnsEvents
  | where QueryType == "TXT"
  | where strlen(Name) > 50
```

### Multiple Queries Pack

When a folder contains multiple `.kql` files:

```yaml
name: Defender for Endpoint
description: Queries from Defender for Endpoint category
author: reprise99/Sentinel-Queries
queries:
  - name: DeviceEvents-USBActivity
    description: Track USB device connections and data exfiltration
    query: |
      DeviceEvents
      | where ActionType has "UsbDrive"

  - name: DeviceProcessEvents-SuspiciousProcesses
    description: Detect suspicious process executions
    query: |
      DeviceProcessEvents
      | where ProcessCommandLine has_any ("powershell", "cmd")
```

## Customization

### Skip Specific Folders

Edit the `skip_folders` set in `generate_packs.py`:

```python
skip_folders = {'Diagrams', 'Workbooks', '.git', 'Documentation'}
```

### Modify Description Extraction

The script extracts descriptions from `//` comment lines. To change this behavior, modify the `extract_description_and_query()` function.

## Troubleshooting

### Submodule Not Found

```
Error: Sentinel-Queries path not found
```

**Solution:** Initialize the submodule:
```bash
git submodule update --init --recursive
```

### Empty Query Warning

```
Warning: Empty query in <file.kql>, skipping
```

This means the file has no KQL content after comment lines. The file will be skipped.

### PyYAML Not Installed

```
ModuleNotFoundError: No module named 'yaml'
```

**Solution:** Install PyYAML:
```bash
pip install pyyaml
```

## Repository Structure

```
examples/sentinel-packs/
├── README.md              # This file
├── generate_packs.py      # Pack generator script
├── Sentinel-Queries/      # Git submodule (reprise99/Sentinel-Queries)
└── generated-packs/       # Output directory (created by script)
    ├── azure-active-directory.yaml
    ├── defender-for-endpoint.yaml
    ├── dns.yaml
    └── ...
```

## Credits

Query content sourced from [reprise99/Sentinel-Queries](https://github.com/reprise99/Sentinel-Queries).

KQL queries are provided as-is for educational and detection purposes.
