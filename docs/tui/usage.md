# TUI Usage

The TUI (Terminal User Interface) provides an interactive shell for exploring and executing packs.

## Starting the TUI

```bash
panopticon-tui
```

### Options

| Option | Description |
|--------|-------------|
| `--pack-dir <PATH>` | Additional pack directory (repeatable) |

```bash
# Add custom pack directories
panopticon-tui --pack-dir ./my-packs --pack-dir /shared/packs
```

Default pack directories:
- `./packs`
- `~/.kql-panopticon/packs`

## Commands

All commands start with `:` (colon).

| Command | Description |
|---------|-------------|
| `:ws` | Open workspace selector |
| `:workspace` | Open workspace selector |
| `:load` | Open pack loader |
| `:run` | Execute loaded pack |
| `:theme` | Open theme selector |
| `:theme <name>` | Switch to specific theme |
| `:logs` | View execution logs |
| `:copy` | Copy all output to clipboard |

### Workspace Selection (`:ws`)

Opens hierarchical workspace selector:

```
┌─ Select Workspaces ─────────────────┐
│ ▼ Production                        │
│   ☑ sentinel-prod                   │
│   ☐ sentinel-backup                 │
│ ▶ Development                       │
│   ☐ sentinel-dev                    │
└─────────────────────────────────────┘
```

**Keys**:
| Key | Action |
|-----|--------|
| `Up/Down` | Navigate |
| `Space` | Toggle selection |
| `Left` | Collapse subscription |
| `Right` | Expand subscription |
| `a` | Select all |
| `n` | Select none |
| `Enter` | Confirm |
| `Esc` | Cancel |

### Pack Loading (`:load`)

Opens pack browser:

```
┌─ Select Pack ───────────────────────┐
│ ▼ ./packs                           │
│   IP Investigation                  │
│   User Analysis                     │
│ ▼ ~/.kql-panopticon/packs           │
│   Threat Hunt                       │
└─────────────────────────────────────┘
```

**Keys**:
| Key | Action |
|-----|--------|
| `Up/Down` | Navigate |
| `Enter` | Select pack |
| `Esc` | Cancel |

### Execution (`:run`)

Executes the loaded pack against selected workspaces.

**Pre-flight checks**:
- Pack must be loaded
- Workspaces must be selected
- Required inputs must be defined

### Theme Selection (`:theme`)

Opens theme selector or switches directly:

```bash
:theme              # Open selector
:theme lcars        # Switch to LCARS theme
```

**Built-in themes**:
- `default` - Standard dark theme
- `lcars` - Star Trek LCARS inspired

### Logs (`:logs`)

Opens execution log viewer:

**Keys**:
| Key | Action |
|-----|--------|
| `j` / `Down` | Scroll down |
| `k` / `Up` | Scroll up |
| `Page Down` | Page down |
| `Page Up` | Page up |
| `g` / `Home` | Jump to top |
| `G` / `End` | Jump to bottom |
| `l` | Cycle log level filter |
| `c` | Clear logs |
| `Esc` | Close |

### Copy to Clipboard (`:copy`)

Copies all interpreter output (commands and results) to the system clipboard.

```
> :copy
Copied 5 block(s) to clipboard (1234 chars)
```

Useful for sharing session output or pasting into reports.

## Setting Inputs

Use `let` syntax to define input values:

```
let target_user:input = alice@contoso.com
let ip_list:input = 8.8.8.8,1.1.1.1
let days:input = 14
```

### Syntax

```
let <name>:input = <value>
```

### Value Formats

```bash
# String
let target_user:input = alice@contoso.com

# Quoted string
let message:input = "Hello World"

# Array (comma-separated)
let ip_list:input = 8.8.8.8,1.1.1.1

# JSON array
let ip_list:input = ["8.8.8.8","1.1.1.1"]
```

## Keyboard Shortcuts

### Interpreter View

| Key | Action |
|-----|--------|
| `Tab` | Trigger autocomplete |
| `Shift+Tab` | Navigate completions backward |
| `Enter` | Submit command / Accept completion |
| `Esc` | Dismiss completion / Cancel |
| `Up/Down` | Navigate command history |
| `Ctrl+k` / `Ctrl+Up` | Scroll output up |
| `Ctrl+j` / `Ctrl+Down` | Scroll output down |
| `Page Up` | Page up output |
| `Page Down` | Page down output |

### Modal Dialogs

| Key | Action |
|-----|--------|
| `Up/Down` | Navigate options |
| `Space` | Toggle selection |
| `Enter` | Confirm |
| `Esc` | Cancel |

## Tab Completion

Context-aware completions:

1. **Commands** - Type `:` then Tab
2. **Variables** - `let ` then Tab shows pack inputs
3. **Values** - After `=` shows history and examples

## Workflow Example

```bash
# 1. Start TUI
panopticon-tui

# 2. Select workspaces
:ws
# (select workspaces, press Enter)

# 3. Load pack
:load
# (select pack, press Enter)

# 4. Set inputs
let target_user:input = alice@contoso.com
let days:input = 14

# 5. Execute
:run

# 6. View logs
:logs

# 7. Copy results (optional)
:copy
```

## Settings

Settings are saved to `~/.kql-panopticon/settings.yaml`:

```yaml
theme_name: "default"
override_terminal_background: false
output_directory: "./output"
pack_directories:
  - "./packs"
  - "~/.kql-panopticon/packs"
```

## Connection Status

Status shown in title bar:

| Status | Description |
|--------|-------------|
| `Authenticating...` | Azure authentication in progress |
| `Discovering...` | Finding workspaces |
| `Ready (N workspaces)` | Ready to use |
| `Error: ...` | Connection failed |

## Execution Results

Results displayed as interpreter blocks:

```
> :run
Executing IP Investigation against sentinel-prod...

  ✓ signin_activity (15 rows)
  ✓ affected_users (8 rows)
  - signin_timeline (skipped)

Risk Assessment:
  Score: 65
  Level: HIGH

Reports written to output/...
```

## Related

- [CLI Usage](../cli/usage.md) - Batch mode
- [Pack Overview](../packs/overview.md) - Pack structure
- [Inputs](../acquisition/inputs.md) - Input configuration
