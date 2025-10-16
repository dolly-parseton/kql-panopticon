# KQL Panopticon

[![Crates.io](https://img.shields.io/crates/v/kql-panopticon.svg)](https://crates.io/crates/kql-panopticon)
[![Build Status](https://github.com/dolly-parseton/kql-panopticon/workflows/CI/badge.svg)](https://github.com/dolly-parseton/kql-panopticon/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)

A terminal-based tool for executing Kusto Query Language (KQL) queries across multiple Azure Log Analytics workspaces concurrently. Results are exported to CSV files organized by subscription, workspace, and timestamp.

> Note: Another one-shot attempt at a CLI/TUI app. Might be some bugs but should serve it's purpose.

## Features

- **Multi-workspace querying**: Execute queries across all accessible Log Analytics workspaces in parallel
- **Azure CLI authentication**: Uses existing Azure CLI credentials (no separate login required)
- **Cross-subscription support**: Discovers and queries workspaces across all accessible subscriptions
- **Azure Lighthouse compatible**: Handles cross-tenant scenarios transparently
- **Terminal UI**: Interactive interface for workspace selection, query editing, and job monitoring
- **Vim-style query editor**: Normal, Insert, and Visual modes for efficient text editing
- **Query loading**: Load and reuse queries from previous jobs
- **Job retry**: Re-execute failed or completed jobs with original parameters
- **Session management**: Save and restore complete application state (queries, jobs, settings)
- **Concurrent execution**: All queries run in parallel with real-time status updates
- **Organized output**: CSV/JSON files automatically organized by subscription, workspace, and timestamp

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

## Usage

If installed from crates.io:
```bash
kql-panopticon
```

If running from source:
```bash
cargo run --release
```

The application launches a full-screen terminal interface. On startup:
1. Validates Azure CLI authentication
2. Loads saved sessions from `~/.kql-panopticon/sessions/`
3. Discovers all accessible Log Analytics workspaces
4. Opens to the Query tab

Logs are written to `kql-panopticon.log` in the current directory.

## Interface Overview

The interface consists of five tabs accessible via number keys (1-5) or Tab/Shift+Tab:

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

## Output Format

CSV files are organized hierarchically:

```
output/
└── {subscription_name}/
    └── {workspace_name}/
        └── {timestamp}/
            └── {job_name}.csv
```

Example:
```
output/
└── sentinel_watchlist_dev/
    └── la-sentinelworkspace/
        └── 2025-10-15_18-46-20/
            ├── security_events.csv
            └── signin_logs.csv
```

Subscription and workspace names are normalized (lowercase, alphanumeric + hyphens/underscores only).

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

**Session Information:**
Each session displays:
- Session name
- Status indicator:
  - `[CURRENT]`: Currently active session, saved
  - `[CURRENT*]`: Currently active session, has unsaved changes
  - `[CURRENT - UNSAVED]`: Active session never saved to disk
  - (blank): Loadable session (not currently active)
- Last saved timestamp

Sessions are stored in `~/.kql-panopticon/sessions/` as JSON files.

## Global Keyboard Shortcuts

These shortcuts work from any tab (except when in Insert mode in the Query tab):

- `1`: Switch to Query tab
- `2`: Switch to Workspaces tab
- `3`: Switch to Settings tab
- `4`: Switch to Jobs tab
- `5`: Switch to Sessions tab
- `Tab`: Next tab
- `Shift+Tab`: Previous tab
- `q`: Quit application

## Authentication

The tool uses Azure CLI authentication tokens stored in `~/.azure/msal_token_cache.json`. Ensure you're logged in before running:

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
Ensure session files exist in `~/.kql-panopticon/sessions/`. Press `r` in the Sessions tab to manually refresh the list.

**Loaded session shows no jobs:**
The session may have been saved with an empty job list. Check the session file content in `~/.kql-panopticon/sessions/{session_name}.json`.

## Architecture

The application uses The Elm Architecture (TEA) pattern for the terminal UI:
- **Model**: Application state (settings, workspaces, queries, jobs)
- **Message**: Events that trigger state changes
- **Update**: Pure functions that transform state based on messages
- **View**: Renders the current state to the terminal

Query execution happens asynchronously via Tokio, with results communicated back to the UI through channels.

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

## Limitations

- Only the first result table from each query is exported
- Jobs created before the retry feature was added cannot be retried (missing context)
- Session auto-save is not implemented (must save manually)

## License

MIT License - see LICENSE file for details.

## Contributing

Contributions are welcome. Please open an issue before submitting major changes to discuss the proposed modifications.