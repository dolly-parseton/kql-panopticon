# Command-Mode Shell Approach for KQL Panopticon

## The Core Idea

Instead of a persistent full-screen TUI with tabs, the interface is primarily a **shell/REPL** with TUI elements that appear contextually:

- **Base state:** A command prompt where you type commands
- **TUI popups:** Appear when you need to browse/select things (packs, workspaces, jobs)
- **TUI panes:** Appear when you need to monitor execution
- **Editor mode:** Full-screen when writing/editing queries
- **Persistent widgets:** Optional status bar, mini-views that stay visible

This is how several modern tools work:
- `fzf` - Fuzzy finder appears inline, returns selection to shell
- `gum` (Charm) - TUI widgets for shell scripts
- Dagger CLI - TUI appears during execution, leaves output in scrollback
- `git add -p` - Interactive prompts inline with shell

---

## What This Looks Like

### Idle State (Shell Prompt)
```
kql-panopticon v0.3.0
Workspace: la-sentinelworkspace (3 more available)
Session: phishing-hunt-dec10 (unsaved)

panopticon> _
```

### Browsing Packs (TUI Popup)
```
panopticon> pack load

┌─Select Pack────────────────────────────────────────┐
│ > azure-sentinel-incidents     [5 queries] LOADED  │
│   defender-for-endpoint        [12 queries]        │
│   active-directory             [8 queries]         │
│   custom/phishing-detection    [3 queries]         │
│                                                    │
│ Filter: _                                          │
└────────────────────────────────────────────────────┘
 ↑/↓:navigate  Enter:select  /:filter  Esc:cancel

```

After selection, returns to shell with context:
```
panopticon> pack load
Loaded: azure-sentinel-incidents (5 queries)
  1. SecurityIncident-DaysSinceLastIncident
  2. SecurityIncident-PlaybookActivities
  3. SecurityIncident-VisualizeSeverity
  4. SecurityIncident-VisualizeIncidentsTrend
  5. SecurityIncident-VisualizeMitreAttack

panopticon> _
```

### Writing a Query (Editor Mode)
```
panopticon> query new

───────────────────────────────────────────────────────────────────
Query Editor                                    [Esc:exit  ^J:run]
───────────────────────────────────────────────────────────────────
  1│ SecurityIncident
  2│ | where TimeGenerated > ago(180d)
  3│ | where Status == "New" and ModifiedBy == "Incident created from alert"
  4│ | summarize arg_max(TimeGenerated, *) by Title
  5│ | extend ['Days Since Last Incident'] = datetime_diff("day", now(), TimeGenerated)
  6│ | project Title, ['Days Since Last Incident']
  7│ | sort by ['Days Since Last Incident'] desc
  8│ _
───────────────────────────────────────────────────────────────────
[NORMAL] Line 8, Col 1                          azure-sentinel-incidents/1
```

This is essentially your current query editor, but it's entered via command and exited back to shell.

### Running a Query (TUI Pane Appears)
```
panopticon> run

───────────────────────────────────────────────────────────────────
Executing on 3 workspaces...
───────────────────────────────────────────────────────────────────
┃ ✓ la-sentinelworkspace    45 rows    2.3s
┃ ⟳ la-production           running    4.1s
┃ ○ la-development          queued
───────────────────────────────────────────────────────────────────
 r:retry  l:logs  Enter:view results  q:dismiss

```

After completion, results summary stays in scrollback:
```
panopticon> run
Executed: SecurityIncident-DaysSinceLastIncident
  la-sentinelworkspace:  45 rows (2.3s) → ./output/sentinel/2024-12-10/job_001.csv
  la-production:         128 rows (5.1s) → ./output/prod/2024-12-10/job_001.csv
  la-development:        0 rows (1.2s)

panopticon> _
```

---

## Investigation Workflows

This is where the command approach really shines. Investigations become **scripted workflows** rather than UI navigation:

### Defining an Investigation
```
panopticon> investigation new phishing-hunt

Investigation: phishing-hunt
Steps: (none)

panopticon> step add url_clicks
Enter query (end with ^D or empty line):
UrlClickEvents
| where Url contains "{{inputs.malicious_url}}"
| project UserPrincipalName, Url, TimeGenerated
^D

Added step: url_clicks
  Extracts: UserPrincipalName, Url, TimeGenerated

panopticon> step add signin_check --depends url_clicks
Enter query:
SigninLogs
| where UserPrincipalName in ({{url_clicks.*.UserPrincipalName}})
| where TimeGenerated > ago(7d)
^D

Added step: signin_check
  Depends on: url_clicks
  Uses: {{url_clicks.*.UserPrincipalName}}

panopticon> investigation show

┌─Investigation: phishing-hunt───────────────────────┐
│                                                    │
│  url_clicks                                        │
│      │                                             │
│      ▼                                             │
│  signin_check                                      │
│                                                    │
│  Inputs required:                                  │
│    - malicious_url (not set)                       │
│                                                    │
└────────────────────────────────────────────────────┘

panopticon> _
```

### Running an Investigation (TUI Monitor)
```
panopticon> investigation run phishing-hunt --set malicious_url=evil.com

┌─Investigation: phishing-hunt────────────────────────────────────┐
│                                                                 │
│ ┃ ✓ url_clicks ──────────────────────────────────────── 3.2s   │
│ │   Query: UrlClickEvents | where Url contains "evil.com"...   │
│ │   Result: 23 rows                                             │
│ │   Extracted: {{.UserPrincipalName}} (23 unique values)       │
│ │                                                               │
│ ┃ ⟳ signin_check ────────────────────────────────── running    │
│ │   Query: SigninLogs | where UPN in ({{url_clicks...}})       │
│ │   Progress: ████████████░░░░░░ 67%                           │
│ │                                                               │
└─────────────────────────────────────────────────────────────────┘
 ↑/↓:select step  Enter:view detail  l:logs  Esc:background

```

**Key insight:** Pressing `Esc` backgrounds the TUI but the investigation keeps running. You return to the shell and can do other things:

```
[Investigation phishing-hunt running in background: 1/2 steps complete]

panopticon> workspace list
  * la-sentinelworkspace (current)
    la-production
    la-development

panopticon> jobs
  phishing-hunt/url_clicks      COMPLETE   23 rows
  phishing-hunt/signin_check    RUNNING    67%

panopticon> fg
[Returning to investigation monitor...]
```

---

## Command Reference (Draft)

### Navigation & Context
```
workspace [list|select|add]     Manage workspaces
session [new|save|load|list]    Manage sessions  
pack [list|load|info|create]    Query pack management
investigation [new|load|show]   Investigation management
config [show|set]               Settings
```

### Query Operations
```
query [new|edit|load]           Enter query editor
run [--workspace] [--all]       Execute current query
validate                        Syntax check without execution
explain                         Show query plan
```

### Investigation Operations
```
step add <name> [--depends]     Add investigation step
step edit <name>                Edit step query
step remove <name>              Remove step
step list                       Show all steps
investigation run [--set]       Execute investigation
investigation export            Save as YAML
```

### Monitoring
```
jobs [list|view|retry|cancel]   Job management
logs <job-id>                   View job logs
results <job-id>                View/export results
fg                              Return to active TUI
```

### TUI Toggles
```
monitor                         Open persistent job monitor
pipeline                        Open investigation DAG view
```

---

## Hybrid Approach: Persistent Status + Shell

You could have a minimal persistent status area while keeping the shell:

```
┌─────────────────────────────────────────────────────────────────┐
│ Workspace: la-sentinel │ Session: phishing-hunt* │ Jobs: 2/3 ✓ │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│ panopticon> investigation run phishing-hunt                     │
│ Starting investigation with 3 steps...                          │
│                                                                 │
│ panopticon> _                                                   │
│                                                                 │
│                                                                 │
│                                                                 │
│                                                                 │
│                                                                 │
│                                                                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

The status bar updates in real-time (Jobs: 2/3 → 3/3 ✓) while you continue working.

---

## Advantages of Command-Mode Approach

### 1. Scriptability
Commands can be scripted, piped, aliased:
```bash
# Run from regular shell
kql-panopticon run --pack azure-sentinel --workspace all --output json | jq '.rows'

# Create aliases
alias kql='kql-panopticon'
alias kql-hunt='kql investigation run threat-hunt --set'
```

### 2. History & Recall
Shell history becomes useful:
```
panopticon> ↑  # Recall last command
panopticon> !run  # Re-run last 'run' command
panopticon> history | grep investigation
```

### 3. Composability
Chain operations naturally:
```
panopticon> pack load azure-sentinel && run --all
panopticon> investigation run phishing --set url=evil.com && results --export csv
```

### 4. Context Preservation
TUI elements appear when needed, dismiss when done, but **output stays in scrollback**. You can scroll up to see what you did.

### 5. Familiar Mental Model
Users already know how shells work. The learning curve is "what commands exist" not "how does this TUI work."

### 6. Better for Complex Authoring
Writing multi-line queries, defining investigation steps, editing YAML - these are all **text editing tasks** that don't benefit from persistent TUI chrome.

---

## Implementation Considerations

### Shell Framework Options for Rust

**Recommended: `clap-repl` or `reedline-repl-rs`**

Both combine reedline (Nushell's readline engine) with clap for command parsing:

```rust
// Using clap-repl (cleaner derive-based API)
use clap::Parser;
use clap_repl::ClapEditor;

#[derive(Debug, Parser)]
#[command(name = "")]
enum Command {
    /// Browse and load query packs
    Pack {
        #[command(subcommand)]
        action: PackAction,
    },
    /// Manage workspaces
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,
    },
    /// Open query editor
    Query {
        #[arg(short, long)]
        load: Option<String>,
    },
    /// Execute current query
    Run {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Investigation workflows
    Investigation {
        #[command(subcommand)]
        action: InvestigationAction,
    },
    /// View job status
    Jobs,
    /// Bring background task to foreground
    Fg,
}

#[derive(Debug, Parser)]
enum PackAction {
    List,
    Load { name: String },
    Info { name: String },
}

fn main() {
    let editor = ClapEditor::<Command>::builder()
        .with_prompt(Box::new(PanopticonPrompt::new()))
        .build();
    
    editor.repl(|cmd| {
        match cmd {
            Command::Pack { action } => handle_pack(action),
            Command::Query { load } => enter_query_editor(load),
            Command::Run { workspace, all } => execute_query(workspace, all),
            // ... etc
        }
    });
}
```

**Key crates:**
- `clap-repl` - Combines clap + reedline, auto-generates completions from your command enum
- `reedline-repl-rs` - Similar but more manual setup
- `reedline` (direct) - If you need lower-level control

**What reedline gives you for free:**
- Tab completion with graphical menu
- History with fuzzy search (Ctrl+R)
- Emacs/vi keybindings
- Syntax highlighting hooks
- Multi-line input support
- Hints (ghost text suggestions)

### TUI Popup Pattern with Ratatui

The key is **inline rendering** - TUI elements render within the terminal flow rather than taking over the whole screen:

```rust
// Pseudo-code concept
fn browse_packs() -> Option<Pack> {
    // Save cursor position
    // Render TUI popup at current position
    // Handle input until selection or cancel
    // Clear popup area
    // Return selection
}
```

Look at how `fzf` and `skim` (Rust fzf alternative) handle this.

**Concrete implementation pattern:**

```rust
use ratatui::prelude::*;
use crossterm::{execute, terminal, cursor};

/// Temporarily take over terminal for a TUI popup, then restore
fn show_selector_popup<T: Clone>(
    items: &[T],
    display_fn: impl Fn(&T) -> String,
) -> Option<T> {
    // 1. Enter alternate screen (preserves shell scrollback)
    let mut stdout = std::io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen).ok()?;
    terminal::enable_raw_mode().ok()?;
    
    // 2. Create ratatui terminal
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).ok()?;
    
    // 3. Run TUI event loop
    let mut selected_index = 0;
    let result = loop {
        terminal.draw(|f| {
            // Render your selector UI
            let list = List::new(items.iter().map(display_fn))
                .highlight_style(Style::default().reversed());
            f.render_stateful_widget(list, f.size(), &mut ListState::default());
        }).ok()?;
        
        if let Event::Key(key) = event::read().ok()? {
            match key.code {
                KeyCode::Enter => break Some(items[selected_index].clone()),
                KeyCode::Esc => break None,
                KeyCode::Up => selected_index = selected_index.saturating_sub(1),
                KeyCode::Down => selected_index = (selected_index + 1).min(items.len() - 1),
                _ => {}
            }
        }
    };
    
    // 4. Restore terminal state
    terminal::disable_raw_mode().ok();
    execute!(terminal.backend_mut(), terminal::LeaveAlternateScreen).ok();
    
    result
}
```

**For inline popups (like fzf)** that don't use alternate screen:

```rust
/// Render popup inline at current cursor position
fn show_inline_popup<T>(items: &[T], max_height: u16) -> Option<T> {
    // Calculate available space below cursor
    let (_, cursor_y) = cursor::position().ok()?;
    let (_, term_height) = terminal::size().ok()?;
    let available = term_height - cursor_y - 1;
    let popup_height = available.min(max_height).min(items.len() as u16);
    
    // Reserve space by printing newlines
    for _ in 0..popup_height {
        println!();
    }
    
    // Move cursor back up
    execute!(stdout(), cursor::MoveUp(popup_height)).ok()?;
    
    // Now render TUI in that space...
    // When done, clear lines and move cursor back
}
```

**Alternative: Use `skim` crate directly for fuzzy selection**

```rust
use skim::prelude::*;

fn select_pack(packs: &[Pack]) -> Option<Pack> {
    let options = SkimOptionsBuilder::default()
        .height(Some("40%"))
        .multi(false)
        .build()
        .unwrap();
    
    let items: Vec<_> = packs.iter()
        .map(|p| format!("{}\t{}", p.name, p.description))
        .collect();
    
    let (tx, rx) = unbounded();
    for item in items {
        tx.send(Arc::new(item)).unwrap();
    }
    drop(tx);
    
    let result = Skim::run_with(&options, Some(rx))?;
    if result.is_abort {
        return None;
    }
    
    result.selected_items.first()
        .and_then(|item| {
            let idx = items.iter().position(|i| i == &item.output().to_string())?;
            Some(packs[idx].clone())
        })
}
```

### Backgrounding Long Operations

```rust
// Spawn investigation execution in background
let handle = tokio::spawn(run_investigation(inv));

// Store handle for later
background_jobs.insert(inv.id, handle);

// User can:
// - `fg` to open monitor TUI
// - `jobs` to check status  
// - Continue typing other commands
```

**More complete background job pattern:**

```rust
use tokio::sync::mpsc;
use std::collections::HashMap;

struct AppContext {
    background_jobs: HashMap<String, BackgroundJob>,
    event_rx: mpsc::Receiver<JobEvent>,
    event_tx: mpsc::Sender<JobEvent>,
}

struct BackgroundJob {
    id: String,
    name: String,
    status: JobStatus,
    handle: tokio::task::JoinHandle<Result<JobResult>>,
}

enum JobEvent {
    Started { id: String },
    Progress { id: String, percent: u8, message: String },
    Completed { id: String, result: JobResult },
    Failed { id: String, error: String },
}

impl AppContext {
    fn spawn_investigation(&mut self, inv: Investigation) {
        let id = uuid::Uuid::new_v4().to_string();
        let tx = self.event_tx.clone();
        
        let handle = tokio::spawn(async move {
            tx.send(JobEvent::Started { id: id.clone() }).await.ok();
            
            for (i, step) in inv.steps.iter().enumerate() {
                tx.send(JobEvent::Progress {
                    id: id.clone(),
                    percent: (i * 100 / inv.steps.len()) as u8,
                    message: format!("Running step: {}", step.name),
                }).await.ok();
                
                // Execute step...
                let result = execute_step(step).await?;
            }
            
            tx.send(JobEvent::Completed { id, result }).await.ok();
            Ok(result)
        });
        
        self.background_jobs.insert(id, BackgroundJob { /* ... */ });
    }
    
    /// Called in main loop to handle background events
    fn poll_background_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                JobEvent::Progress { id, percent, message } => {
                    // Update status bar or print inline update
                    eprintln!("[{}] {}% - {}", id, percent, message);
                }
                JobEvent::Completed { id, result } => {
                    println!("✓ Job {} complete: {} rows", id, result.row_count);
                }
                // ...
            }
        }
    }
}
```

**Shell prompt integration:**

```rust
struct PanopticonPrompt {
    context: Arc<RwLock<AppContext>>,
}

impl reedline::Prompt for PanopticonPrompt {
    fn render_prompt_left(&self) -> Cow<str> {
        let ctx = self.context.read().unwrap();
        let running = ctx.background_jobs.values()
            .filter(|j| matches!(j.status, JobStatus::Running))
            .count();
        
        if running > 0 {
            format!("panopticon [{}⟳]> ", running).into()
        } else {
            "panopticon> ".into()
        }
    }
    
    fn render_prompt_right(&self) -> Cow<str> {
        let ctx = self.context.read().unwrap();
        format!("{}", ctx.current_workspace.name).into()
    }
}
```

---

## Reference Implementations to Study

### Tools that do shell + TUI well:

1. **Nushell** - The whole shell is structured data aware, TUI elements for completion
   - https://github.com/nushell/nushell
   - Uses reedline (which you'd also use)

2. **Harlequin** (SQL IDE in terminal) - Great example of editor mode + results viewer
   - https://github.com/tconbeer/harlequin
   - Textual-based (Python) but patterns apply

3. **Posting** (HTTP client) - Shell-like with TUI for request/response
   - https://github.com/darrenburns/posting
   - Good example of form input + results display

4. **gobang** (DB TUI) - Component architecture in Ratatui
   - https://github.com/TaKO8Ki/gobang
   - Shows how to structure complex DB interactions

5. **skim** (fuzzy finder) - Inline popup rendering
   - https://github.com/lotabout/skim
   - Rust fzf clone, shows inline TUI popup pattern

---

## Comparison: Full TUI vs Command Mode

| Aspect | Full-Screen TUI | Command-Mode Shell |
|--------|-----------------|-------------------|
| Observation | Excellent | Good (with `monitor` command) |
| Authoring | Awkward (modal editing in TUI) | Natural (shell + editor) |
| Scriptability | Poor | Excellent |
| Learning curve | Steeper | Familiar shell patterns |
| Context switching | Loses scrollback | Preserves history |
| Discoverability | Better (visible UI) | Needs `help`, completions |
| Accessibility | Harder | Screen readers handle shells |

---

## Suggested Hybrid Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│                     kql-panopticon                              │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   Shell REPL Layer                       │   │
│  │  - Command parsing (clap)                                │   │
│  │  - History & completion (reedline)                       │   │
│  │  - Inline output                                         │   │
│  └───────────────────────────┬─────────────────────────────┘   │
│                              │                                  │
│         ┌────────────────────┼────────────────────┐            │
│         ▼                    ▼                    ▼            │
│  ┌─────────────┐    ┌──────────────┐    ┌──────────────┐      │
│  │ TUI Popups  │    │  TUI Panes   │    │ Editor Mode  │      │
│  │ (ratatui)   │    │  (ratatui)   │    │ (ratatui)    │      │
│  │             │    │              │    │              │      │
│  │ - Pack      │    │ - Job        │    │ - Query      │      │
│  │   browser   │    │   monitor    │    │   editor     │      │
│  │ - Workspace │    │ - Pipeline   │    │ - Step       │      │
│  │   selector  │    │   DAG        │    │   editor     │      │
│  │ - Fuzzy     │    │ - Results    │    │ - Config     │      │
│  │   finder    │    │   viewer     │    │   editor     │      │
│  └─────────────┘    └──────────────┘    └──────────────┘      │
│         │                    │                    │            │
│         └────────────────────┴────────────────────┘            │
│                              │                                  │
│                    ┌─────────▼─────────┐                       │
│                    │   Core Engine     │                       │
│                    │  - Azure API      │                       │
│                    │  - Query exec     │                       │
│                    │  - State mgmt     │                       │
│                    └───────────────────┘                       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

The shell REPL is the primary interface. TUI elements are **tools the shell invokes** rather than the shell being a feature of the TUI.

---

## Example Session

```
$ kql-panopticon
kql-panopticon v0.4.0
Discovering workspaces... found 4 across 2 subscriptions.

panopticon> workspace select
┌─Select Workspace───────────────────────────────────┐
│ > la-sentinelworkspace    (sentinel-subscription)  │
│   la-production           (prod-subscription)      │
│   la-development          (dev-subscription)       │
│   la-staging              (dev-subscription)       │
└────────────────────────────────────────────────────┘
Selected: la-sentinelworkspace

panopticon> pack load azure-sentinel-incidents
Loaded: azure-sentinel-incidents (5 queries)

panopticon> query edit 1
[Opens editor with SecurityIncident-DaysSinceLastIncident]
[User edits, saves, exits editor]
Query updated.

panopticon> run
Executing on la-sentinelworkspace...
┃ ⟳ running (2.1s)
✓ Complete: 45 rows (3.2s)
Output: ./output/sentinel/2024-12-10_15-42-08/job_001.csv

panopticon> results
┌─Results: job_001 (45 rows)──────────────────────────────────────┐
│ Title                              │ Days Since Last Incident   │
├────────────────────────────────────┼────────────────────────────┤
│ Suspicious login from TOR          │ 3                          │
│ Multiple failed logins             │ 7                          │
│ Unusual outbound traffic           │ 12                         │
│ ...                                │                            │
└─────────────────────────────────────────────────────────────────┘
 ↑/↓:scroll  Enter:detail  e:export  q:close

panopticon> session save dec10-sentinel-review
Session saved: dec10-sentinel-review

panopticon> exit
$
```

This feels like a natural workflow: commands when you know what you want, TUI when you need to browse/select/monitor.
