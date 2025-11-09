# KQL Panopticon

[![Crates.io](https://img.shields.io/crates/v/kql-panopticon.svg)](https://crates.io/crates/kql-panopticon)
[![Build Status](https://github.com/dolly-parseton/kql-panopticon/workflows/CI/badge.svg)](https://github.com/dolly-parseton/kql-panopticon/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)

A terminal-based tool for executing Kusto Query Language (KQL) queries across multiple Azure Log Analytics workspaces concurrently. Features an interactive TUI, reusable query packs, session management, and organized CSV/JSON exports.

## Features

### Core Capabilities
- **Multi-workspace querying**: Execute queries across all accessible Log Analytics workspaces in parallel
- **Azure CLI authentication**: Uses existing Azure CLI credentials (no separate login required)
- **Cross-subscription support**: Discovers and queries workspaces across all accessible subscriptions
- **Azure Lighthouse compatible**: Handles cross-tenant scenarios transparently
- **Concurrent execution**: All queries run in parallel with real-time status updates
- **Organized output**: CSV/JSON files automatically organized by subscription, workspace, and timestamp

### Query Packs
- **Reusable query definitions**: Create, share, and version control query packs (YAML/JSON)
- **CLI execution**: Run query packs from command line for automation and CI/CD
- **TUI browser**: Browse and execute packs from the interactive interface
- **AI-friendly format**: Minimal YAML format perfect for AI-generated threat hunting queries
- **Session export**: Convert refined queries back to reusable packs
- **Pack origin tracking**: Sessions remember which pack created them

### Terminal UI
- **Vim-style query editor**: Normal, Insert, and Visual modes for efficient text editing
- **Query loading**: Load and reuse queries from previous jobs
- **Job retry**: Re-execute failed or completed jobs with original parameters
- **Session management**: Save and restore complete application state (queries, jobs, settings)
- **Pack browser**: Discover and execute query packs from your library

## Operating System Compatibility

**Tested and supported:**
- macOS (primary development platform)
- Linux

**Theoretical Windows support:**
- The codebase uses cross-platform dependencies and builds successfully on Windows in CI
- All file path handling uses platform-agnostic Rust APIs (`PathBuf`, `dirs` crate)
- Azure CLI credential discovery works on Windows via `azure_identity` crate
- However, Windows has **not been tested by the maintainer**

**Windows-specific notes:**
- Configuration directory: `%USERPROFILE%\.kql-panopticon\` (instead of `~/.kql-panopticon/`)
- Azure CLI tokens: `%USERPROFILE%\.azure\msal_token_cache.json`
- Use Windows Terminal or another modern terminal emulator for best Unicode support
- If you encounter issues on Windows, please open a GitHub issue with details

The CI pipeline builds and tests on `windows-latest`, so basic functionality should work. Community testing and feedback on Windows is welcome.

## Prerequisites

- Rust toolchain (1.70 or later)
- Azure CLI installed and authenticated (`az login`)
- Access to at least one Azure subscription with Log Analytics workspaces
- Terminal with minimum size of 80x24

## Installation

### From crates.io (recommended)

```bash
cargo install kql-panopticon
```

### From source

```bash
git clone https://github.com/dolly-parseton/kql-panopticon.git
cd kql-panopticon
cargo build --release
```

The binary will be located at `target/release/kql-panopticon`.

## Quick Start

### Interactive Mode (TUI)

Launch the terminal interface:
```bash
kql-panopticon
```

The application will:
1. Validate Azure CLI authentication
2. Load saved sessions from the config directory
3. Discover all accessible Log Analytics workspaces
4. Open to the Settings tab

### CLI Mode (Query Packs)

Execute a query pack across workspaces:
```bash
# Run pack on all workspaces
kql-panopticon run-pack security/failed-auth.yaml

# Run pack on specific workspaces
kql-panopticon run-pack security/failed-auth.yaml --workspaces ws-prod-01,ws-prod-02

# Validate pack without executing
kql-panopticon run-pack security/failed-auth.yaml --validate-only

# Export session as reusable pack
kql-panopticon export-pack my-session-name
```

Logs are written to `kql-panopticon.log` in the current directory.

## Query Packs

Query packs separate reusable query definitions from execution results (sessions), enabling version control, team collaboration, and AI-assisted query generation.

### Creating a Query Pack

**Minimal format** (single query):
```yaml
# ~/.kql-panopticon/packs/security/failed-auth.yaml
name: "Failed Authentication Investigation"
query: |
  SecurityEvent
  | where EventID == 4625
  | where TimeGenerated > ago(24h)
  | summarize FailedAttempts=count() by Account, Computer
  | order by FailedAttempts desc
```

**Full format** (multi-query investigation):
```yaml
name: "Failed Authentication Investigation"
description: "Hunt for brute force and credential stuffing patterns"
author: "Security Team"
version: "1.0"

queries:
  - name: "Failed Logins Baseline"
    description: "Last 24h failed login volume"
    query: |
      SecurityEvent
      | where EventID == 4625
      | where TimeGenerated > ago(24h)
      | summarize count() by Account

  - name: "Brute Force Detection"
    description: "Accounts with >10 failures in 5min windows"
    query: |
      SecurityEvent
      | where EventID == 4625
      | summarize Attempts=count() by Account, bin(TimeGenerated, 5m)
      | where Attempts > 10

settings:
  export_csv: true
  export_json: false
  parse_dynamics: true

workspaces:
  scope: all  # "all", "selected", or "pattern"
```

### Executing Query Packs

**From CLI:**
```bash
# Execute pack (creates session with results)
kql-panopticon run-pack security/failed-auth.yaml

# Override workspace selection
kql-panopticon run-pack test.yaml --workspaces all

# JSON output to stdout
kql-panopticon run-pack test.yaml --format stdout --json
```

**From TUI:**
1. Press `6` to go to Packs tab
2. Use `Up/Down` to select pack
3. Press `Enter` to load first query into editor
4. Press `e` to execute entire pack across selected workspaces

### Exporting Sessions as Packs

Convert a refined query session back to a reusable pack:

**From CLI:**
```bash
# Export to default location: ~/.kql-panopticon/packs/<session-name>.yaml
kql-panopticon export-pack my-session-name

# Export to custom path
kql-panopticon export-pack my-session-name --output /path/to/pack.yaml

# Export as JSON
kql-panopticon export-pack my-session-name --format json
```

**From TUI:**
1. Press `5` to go to Sessions tab
2. Use `Up/Down` to select session
3. Press `p` to export as pack
4. Pack saved to `~/.kql-panopticon/packs/` and appears in Packs tab

### AI Workflow Example

1. Ask your AI assistant to generate threat hunting queries
2. Have it output in query pack YAML format
3. Save to `~/.kql-panopticon/packs/security/ransomware.yaml`
4. Execute: `kql-panopticon run-pack security/ransomware.yaml`
5. Review results, refine queries in TUI
6. Export improved version: Press `p` in Sessions tab
7. Version control and share the refined pack

## Interface Overview

The interface consists of six tabs accessible via number keys (1-6) or Tab/Shift+Tab:

### 1. Settings Tab

Configure application behavior.

**Navigation:**
- `Up/Down`: Select setting
- `Enter`: Edit selected setting
- `Esc`: Cancel edit
- `Enter` (while editing): Save changes

**Available Settings:**
- **Output Folder**: Directory for CSV/JSON exports (default: `./output`)
- **Query Timeout (secs)**: Maximum execution time per query (default: 30)
- **Retry Count**: Number of automatic retries on query failure with exponential backoff (default: 0)
- **Validation Interval (secs)**: How often to revalidate Azure authentication (default: 300)
- **Export CSV**: Enable CSV file export (default: true)
- **Export JSON**: Enable JSON file export (default: false)
- **Parse Dynamics**: Parse dynamic columns in JSON results (default: true)

### 2. Workspaces Tab

Select target workspaces for query execution.

**Navigation:**
- `Up/Down`: Navigate workspace list
- `Space`: Toggle selection of current workspace
- `a`: Select all workspaces
- `n`: Deselect all workspaces
- `r`: Refresh workspace list from Azure

**Display Information:**
Each workspace shows:
- Workspace name
- Subscription name
- Resource group
- Azure region

Selected workspaces are marked with `[x]`.

### 3. Query Tab

Write and execute KQL queries using a Vim-style editor.

**Editor Modes:**

**Normal Mode** (default):
- `i`: Enter Insert mode (edit at cursor)
- `a`: Enter Insert mode after cursor
- `A`: Enter Insert mode at end of line
- `o`: Open new line below and enter Insert mode
- `O`: Open new line above and enter Insert mode
- `v`: Enter Visual mode (text selection)
- `h/j/k/l` or Arrow Keys: Move cursor
- `0`: Move to start of line
- `$`: Move to end of line
- `g`: Move to top of document
- `G`: Move to bottom of document
- `x`: Delete character under cursor
- `Ctrl+d`: Delete current line
- `c`: Clear all text
- `Ctrl+u`: Undo
- `Ctrl+r`: Redo

**Insert Mode:**
- `Esc`: Return to Normal mode
- All other keys insert text normally

**Visual Mode:**
- `h/j/k/l` or Arrow Keys: Extend selection
- `y`: Copy (yank) selected text
- `d` or `x`: Delete selected text
- `Esc`: Return to Normal mode

**Query Management:**
- `Ctrl+j`: Execute query (works in any mode)
  - Prompts for job name
  - Creates one job per selected workspace
  - Jobs run concurrently in background
- `l`: Load query from previous job
  - Opens selection panel showing all jobs with saved queries
  - Navigate with Up/Down arrows
  - Tab: Cycle sort order (Chronological → Workspace → Status)
  - `i`: Invert sort order
  - Enter: Load selected query
  - Esc: Cancel and restore original query

**Example Query:**
```kql
SecurityEvent
| where TimeGenerated > ago(24h)
| where EventID == 4624
| summarize LoginCount = count() by Account, Computer
| order by LoginCount desc
| take 100
```

### 4. Jobs Tab

Monitor query execution and view results.

**Navigation:**
- `Up/Down`: Select job
- `Enter`: View job details (status, timing, output path, errors)
- `r`: Retry selected job (failed or completed jobs only)
  - Creates new job with same query, workspace, and settings
  - Executes immediately in background
- `c`: Clear all completed and failed jobs from list
- `Esc` (in details view): Close details popup

**Job Status:**
- **Queued**: Waiting to start
- **Running**: Currently executing
- **Completed**: Finished successfully
- **Failed**: Query execution error

**Job Information:**
Each job displays:
- Status indicator
- Workspace name
- Query preview (first 50 characters)
- Execution time
- Row count (for completed jobs)
- Error message (for failed jobs)

Jobs with full query context can be retried or loaded in the Query tab.

### 5. Sessions Tab

Save and load complete application state including jobs, queries, and settings.

**Navigation:**
- `Up/Down`: Navigate sessions list
- `r`: Refresh sessions list from disk
- `s`: Save current session
  - If session already exists, overwrites it
  - If no current session, prompts for name
- `Shift+S`: Save as new session (prompts for name)
- `n`: Create new session (prompts for name)
- `l`: Load selected session
  - Restores all settings
  - Restores job history with full query context
  - Sets loaded session as current
- `d`: Delete selected session from disk
- `p`: Export selected session as query pack
  - Converts session to reusable pack format
  - Deduplicates queries across workspaces
  - Saves to packs directory

**Session Information:**
Each session displays:
- Session name
- Status indicator:
  - `[CURRENT]`: Currently active session, saved
  - `[CURRENT*]`: Currently active session, has unsaved changes
  - `[CURRENT - UNSAVED]`: Active session never saved to disk
  - (blank): Loadable session (not currently active)
- Last saved timestamp
- Pack origin (if created from a query pack)

Sessions are stored in the config directory's `sessions/` subdirectory as JSON files.

### 6. Packs Tab

Browse and execute query packs from your library.

**Navigation:**
- `Up/Down`: Navigate packs list
- `Enter`: Load first query from pack into Query tab
- `e`: Execute entire pack on selected workspaces
  - Creates one job per query per workspace
  - Saves results as new session
- `r`: Refresh packs list from disk

**Display Information:**
Each pack shows:
- Pack name
- Description (if available)
- Number of queries
- File path

Packs are loaded from the config directory's `packs/` subdirectory (supports subdirectories).

## Output Format

CSV/JSON files are organized hierarchically:

```
output/
└── {subscription_name}/
    └── {workspace_name}/
        └── {timestamp}/
            └── {job_name}_{query_name}.csv
```

Example:
```
output/
└── sentinel_watchlist_dev/
    └── la-sentinelworkspace/
        └── 2025-11-08_18-46-20/
            ├── security-hunt_failed-logins.csv
            └── security-hunt_brute-force-detection.csv
```

Subscription and workspace names are normalized (lowercase, alphanumeric + hyphens/underscores only).

When executing query packs with multiple queries, each query gets its own file with a sanitized query name suffix to prevent conflicts.

## Global Keyboard Shortcuts

These shortcuts work from any tab (except when in Insert mode in the Query tab):

- `1`: Switch to Settings tab
- `2`: Switch to Workspaces tab
- `3`: Switch to Query tab
- `4`: Switch to Jobs tab
- `5`: Switch to Sessions tab
- `6`: Switch to Packs tab
- `Tab`: Next tab
- `Shift+Tab`: Previous tab
- `q`: Quit application

## Command-Line Interface

### Run Query Pack

```bash
kql-panopticon run-pack <pack> [OPTIONS]

Arguments:
  <pack>  Path to query pack file (.yaml, .yml, or .json)
          Can be absolute path or relative to ~/.kql-panopticon/packs/

Options:
  -w, --workspaces <WORKSPACES>  Override workspace selection (comma-separated IDs or 'all')
  -f, --format <FORMAT>          Output format [default: files] [possible values: files, stdout]
      --json                     Print results to stdout as JSON
      --validate-only            Validate pack without executing
  -h, --help                     Print help
```

### Export Session as Pack

```bash
kql-panopticon export-pack <session> [OPTIONS]

Arguments:
  <session>  Session name to export

Options:
  -o, --output <OUTPUT>    Output path (default: ~/.kql-panopticon/packs/<session-name>.yaml)
  -f, --format <FORMAT>    Output format [default: yaml] [possible values: yaml, json]
  -h, --help               Print help
```

## Authentication

The tool uses Azure CLI authentication tokens (stored in `~/.azure/msal_token_cache.json` on macOS/Linux, or `%USERPROFILE%\.azure\msal_token_cache.json` on Windows). Ensure you're logged in before running:

```bash
az login
```

If you have multiple tenants, specify the correct one:

```bash
az login --tenant YOUR_TENANT_ID
```

Authentication is validated on startup and periodically based on the configured validation interval.

## Troubleshooting

**"Terminal too small" error:**
Resize your terminal to at least 80 columns by 24 rows.

**"Authentication failed" on startup:**
Run `az login` to refresh your Azure CLI credentials.

**No workspaces found:**
Ensure your account has `Log Analytics Reader` or higher permissions on at least one workspace.

**Query times out:**
Increase the timeout value in Settings tab or optimize your query.

**Jobs stuck in "Running" state:**
Check `kql-panopticon.log` for error details. The job may have exceeded the timeout or encountered a network error.

**Sessions not appearing on startup:**
Ensure session files exist in the config directory's `sessions/` subdirectory. Press `r` in the Sessions tab to manually refresh the list.

**Query pack validation fails:**
Ensure pack has either `query` field (single query) or `queries` array (multiple queries), but not both. Use `--validate-only` flag to check.

**Pack export shows "no queries to export":**
The session may not have stored query context. Only jobs created with full context (query, workspace, settings) can be exported.

## Architecture

The application uses The Elm Architecture (TEA) pattern for the terminal UI:
- **Model**: Application state (settings, workspaces, queries, jobs, sessions, packs)
- **Message**: Events that trigger state changes
- **Update**: Pure functions that transform state based on messages
- **View**: Renders the current state to the terminal

Query execution happens asynchronously via Tokio, with results communicated back to the UI through channels.

Query packs provide a clean separation of concerns:
- **Query Pack**: Reusable query definition (version controlled, shareable)
- **Session**: Execution record (results, timing, errors, disposable)

## Performance Considerations

- Queries execute concurrently across all selected workspaces (no artificial limits)
- Each query has an independent timeout (configurable in Settings)
- Failed queries are automatically retried with exponential backoff (if retry count > 0):
  - Retry 1: 1 second delay
  - Retry 2: 2 seconds delay
  - Retry 3: 4 seconds delay
  - Retry 4+: 8+ seconds delay
- Failed queries don't affect other jobs
- Pagination is automatically handled for large result sets
- Large result sets (>10,000 rows) may take several seconds to write to CSV/JSON
- Network latency varies based on workspace region
- Multi-query packs execute all queries in parallel for maximum performance

## Limitations

- Only the first result table from each query is exported
- Jobs created before the retry feature was added cannot be retried (missing context)
- Session auto-save is not implemented (must save manually)
- Query pack workspace scope patterns use simple glob-style matching (not full regex)

## Use Cases

### Threat Hunting
Create reusable investigation packs for common threat scenarios. Execute across all workspaces, review results, refine queries, and share improved packs with the team.

### Security Auditing
Build query packs for compliance checks. Schedule execution via CLI in CI/CD pipelines. Track investigation sessions with full context.

### Incident Response
Load pre-built query packs for rapid triage. Execute across affected workspaces. Export refined queries as updated packs for future incidents.

### AI-Assisted Analysis
Generate query packs using AI assistants. Validate and execute in one command. Iteratively improve queries based on results.

## File Organization

Configuration and data stored in home directory (on macOS/Linux: `~/.kql-panopticon/`, on Windows: `%USERPROFILE%\.kql-panopticon\`):

```
.kql-panopticon/
├── packs/                    # Query pack library
│   ├── security/
│   │   ├── failed-auth.yaml
│   │   └── ransomware.yaml
│   └── compliance/
│       └── audit-logs.yaml
└── sessions/                 # Saved sessions
    ├── investigation-2025-01-15.json
    └── baseline-queries.json
```

## License

MIT License - see LICENSE file for details.

## Contributing

Contributions are welcome. Please open an issue before submitting major changes to discuss the proposed modifications.

## Terminal Compatibility

This application uses Unicode box-drawing characters for the TUI interface. For the best experience, use a modern terminal emulator with proper Unicode support:

**Recommended terminals:**
- **Alacritty** (macOS, Linux, Windows) - Excellent Unicode support
- **iTerm2** (macOS) - Full Unicode box-drawing support
- **Windows Terminal** (Windows) - Modern Unicode rendering
- **Kitty** (macOS, Linux) - GPU-accelerated with proper Unicode handling
- **WezTerm** (cross-platform) - Comprehensive Unicode support

**Known issues:**
- **macOS Terminal.app** - Box-drawing characters may not connect properly due to limited Unicode support. The application remains fully functional, but borders may appear segmented rather than continuous.
- **Older terminals** - May have similar Unicode rendering limitations

If you experience visual issues with borders, consider switching to one of the recommended terminal emulators.