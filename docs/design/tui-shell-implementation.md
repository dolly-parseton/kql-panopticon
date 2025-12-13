# TUI Shell Implementation Plan

*Created: December 2024*
*Status: Active Development*

## Overview

Migrate the KQL Panopticon REPL from `clap_repl` + `reedline` to a full ratatui TUI application. The interpreter becomes a component within a managed terminal interface, enabling inline widgets, scrollable output, and unified rendering.

### Goals

1. Unified TUI shell that owns the terminal
2. Scrollable output area with inline widgets
3. User-controlled block cleanup (minimize/remove)
4. Preserve all existing command functionality
5. Enable rich widgets (tables, progress, selectors) inline

### Development Strategy

**Module + Feature Flag approach:**
- Develop TUI shell in `src/tui/` module
- Feature flag `tui` enables new implementation
- Current REPL remains functional throughout development
- Switch default when TUI reaches feature parity

```
cargo run                    # Current REPL (always works)
cargo run --features tui     # New TUI shell (in development)
```

---

## Project Structure

```
crates/kql-panopticon-repl/
├── Cargo.toml                    # Add tui feature flag
├── src/
│   ├── main.rs                   # Feature-gated entry point
│   ├── commands/                 # SHARED - no changes needed
│   │   ├── mod.rs                # CommandResult updated
│   │   ├── authoring.rs
│   │   ├── workspace.rs
│   │   ├── run.rs
│   │   └── ...
│   ├── context.rs                # SHARED - no changes
│   ├── session.rs                # SHARED - no changes
│   ├── state_graph.rs            # SHARED - no changes
│   ├── editor/                   # ADAPT - for inline use later
│   ├── tui/                      # NEW - TUI shell
│   │   ├── mod.rs                # Module exports, run() entry
│   │   ├── app.rs                # App state and main loop
│   │   ├── event.rs              # Event handling
│   │   ├── ui/
│   │   │   ├── mod.rs
│   │   │   ├── status_bar.rs     # Top status bar
│   │   │   ├── output_area.rs    # Scrollable block container
│   │   │   ├── input_line.rs     # Command input
│   │   │   └── block.rs          # OutputBlock rendering
│   │   └── widgets/              # Interactive/static widgets (Phase T5)
│   │       ├── mod.rs
│   │       └── ...
│   ├── completer.rs              # SHARED - reuse for completion
│   ├── prompt.rs                 # REMOVE after migration
│   ├── validator.rs              # SHARED - line continuation logic
│   └── history.rs                # SHARED - execution history
```

---

## Phase T1: App Shell & Event Loop

**Goal:** Basic ratatui application that renders and handles input. No command execution yet.

### T1.1: Project Setup

**Tasks:**
- [ ] Add `tui` feature to `Cargo.toml`
- [ ] Create `src/tui/mod.rs` with feature-gated module
- [ ] Update `main.rs` with feature-gated entry points
- [ ] Verify `cargo run` still works (current REPL)
- [ ] Verify `cargo run --features tui` compiles (empty shell)

**Cargo.toml additions:**
```toml
[features]
default = []
tui = []

[dependencies]
# Already have ratatui, crossterm from editor
```

**main.rs structure:**
```rust
#[cfg(feature = "tui")]
mod tui;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("error")
    ).init();

    #[cfg(feature = "tui")]
    return tui::run().await;

    #[cfg(not(feature = "tui"))]
    return run_legacy_repl().await;
}

#[cfg(not(feature = "tui"))]
async fn run_legacy_repl() -> Result<()> {
    // Current implementation moved here
}
```

### T1.2: Core Types

**Tasks:**
- [ ] Create `src/tui/app.rs` with `App` struct
- [ ] Define `AppState`, `OutputBlock`, `BlockContent` types
- [ ] Define `Focus` enum
- [ ] Define `InputState` struct

**Types:**
```rust
// src/tui/app.rs

use crate::context::SharedContext;

pub struct App {
    pub state: AppState,
    pub ctx: SharedContext,
    pub should_quit: bool,
}

pub struct AppState {
    pub output: Vec<OutputBlock>,
    pub scroll_offset: usize,
    pub input: InputState,
    pub focus: Focus,
}

// src/tui/ui/block.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u64);

pub struct OutputBlock {
    pub id: BlockId,
    pub content: BlockContent,
    pub minimized: bool,
}

pub enum BlockContent {
    /// Echoed command
    Command(String),
    /// Text output (supports ANSI or styled)
    Text(String),
    /// Error output
    Error(String),
    // Widget variants added in Phase T5
}

// src/tui/app.rs

pub enum Focus {
    Input,
    Block(BlockId),
}

pub struct InputState {
    pub buffer: String,
    pub cursor: usize,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
}
```

### T1.3: Application Loop

**Tasks:**
- [ ] Create `src/tui/event.rs` for event handling
- [ ] Implement terminal setup/teardown in `tui::run()`
- [ ] Implement main event loop (poll events, update state, render)
- [ ] Handle Ctrl+C / Ctrl+D for quit
- [ ] Handle terminal resize

**Entry point:**
```rust
// src/tui/mod.rs

mod app;
mod event;
mod ui;

pub use app::App;

pub async fn run() -> anyhow::Result<()> {
    // Setup terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    // Create app
    let ctx = crate::context::create_shared_context();
    let mut app = App::new(ctx);

    // Run event loop
    let result = app.run(&mut terminal).await;

    // Restore terminal
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;

    result
}
```

### T1.4: Basic UI Rendering

**Tasks:**
- [ ] Create `src/tui/ui/mod.rs`
- [ ] Create `src/tui/ui/status_bar.rs` - renders top bar
- [ ] Create `src/tui/ui/output_area.rs` - renders block list
- [ ] Create `src/tui/ui/input_line.rs` - renders input with cursor
- [ ] Create `src/tui/ui/block.rs` - renders individual blocks
- [ ] Implement `App::render()` composing all UI pieces

**Layout:**
```
┌─────────────────────────────────────────────────┐
│ Status Bar (1 line)                             │
├─────────────────────────────────────────────────┤
│                                                 │
│ Output Area (flexible height)                   │
│                                                 │
├─────────────────────────────────────────────────┤
│ > Input Line (1-3 lines)                        │
└─────────────────────────────────────────────────┘
```

### T1.5: Input Handling

**Tasks:**
- [ ] Handle character input (append to buffer)
- [ ] Handle backspace (delete character)
- [ ] Handle left/right arrows (move cursor)
- [ ] Handle Home/End (cursor to start/end)
- [ ] Handle Enter (placeholder - just clear buffer for now)

### T1.6: Scroll Behavior

**Tasks:**
- [ ] Track scroll offset (0 = viewing bottom)
- [ ] Handle Up/Down or PageUp/PageDown for scrolling (when not in input)
- [ ] Implement "stick to bottom" logic
- [ ] Auto-scroll on new content only if already at bottom

### Acceptance Tests - Phase T1

```
T1-TEST-1: Shell launches
─────────────────────────
$ cargo run --features tui
Expected: Full-screen TUI appears with:
  - Status bar showing "KQL Panopticon"
  - Empty output area
  - Input line with "> " prompt and cursor

T1-TEST-2: Typing works
───────────────────────
# Type "hello world"
Expected: Characters appear in input line
# Press Backspace
Expected: Characters deleted
# Press Left/Right arrows
Expected: Cursor moves

T1-TEST-3: Quit works
─────────────────────
# Press Ctrl+D
Expected: App exits, terminal restored cleanly

T1-TEST-4: Resize works
───────────────────────
# Resize terminal window
Expected: UI adjusts, no visual corruption

T1-TEST-5: Legacy REPL still works
──────────────────────────────────
$ cargo run
Expected: Original clap_repl REPL launches (unchanged)
```

---

## Phase T2: Command Execution

**Goal:** Wire clap command parsing. Commands execute and produce text output blocks.

### T2.1: CommandResult Adaptation

**Tasks:**
- [ ] Add `CommandOutput` enum to `commands/mod.rs`
- [ ] Update `CommandResult` to use `CommandOutput`
- [ ] Create conversion from current string-based returns
- [ ] Ensure all existing handlers still compile

**Changes to commands/mod.rs:**
```rust
/// Command output that becomes an OutputBlock
pub enum CommandOutput {
    /// Simple text
    Text(String),
    /// Error message
    Error(String),
    /// No visible output
    None,
    // Future: Widget, Interactive, etc.
}

/// Result of command execution
pub enum CommandResult {
    /// Output to display
    Output(CommandOutput),
    /// Clear the output area
    Clear,
    /// Exit the application
    Exit,
}

// Convenience constructors (preserve existing API)
impl CommandResult {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Output(CommandOutput::Text(msg.into()))
    }

    pub fn output(msg: impl Into<String>) -> Self {
        Self::Output(CommandOutput::Text(msg.into()))
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self::Output(CommandOutput::Error(msg.into()))
    }

    pub fn none() -> Self {
        Self::Output(CommandOutput::None)
    }
}
```

### T2.2: Command Parsing Integration

**Tasks:**
- [ ] On Enter, take input buffer content
- [ ] Parse with clap (reuse `ReplCommand` parser)
- [ ] Push `BlockContent::Command(input)` to output (echo)
- [ ] Execute command via `commands::execute()`
- [ ] Convert `CommandResult` to `OutputBlock` and push
- [ ] Clear input buffer
- [ ] Handle clap parse errors as error blocks

### T2.3: Command History

**Tasks:**
- [ ] Store executed commands in `InputState.history`
- [ ] Up arrow: navigate to previous command
- [ ] Down arrow: navigate to next command
- [ ] Esc: cancel history navigation, restore current input

### T2.4: Special Commands

**Tasks:**
- [ ] Handle `CommandResult::Exit` - set `should_quit = true`
- [ ] Handle `CommandResult::Clear` - clear output blocks

### Acceptance Tests - Phase T2

```
T2-TEST-1: Command execution
────────────────────────────
> help
Expected: Help text appears as output block

> status
Expected: Status info appears

T2-TEST-2: Command echo
───────────────────────
> workspace list
Expected: Output shows:
  "> workspace list"        ← Echoed command block
  "Discovering..."          ← Command output block

T2-TEST-3: Error handling
─────────────────────────
> notacommand
Expected: Error block with clap error message

T2-TEST-4: Command history
──────────────────────────
> input x = "1"
> input y = "2"
# Press Up
Expected: Input shows "input y = \"2\""
# Press Up again
Expected: Input shows "input x = \"1\""

T2-TEST-5: Exit command
───────────────────────
> exit
Expected: App quits cleanly

T2-TEST-6: Clear command
────────────────────────
> help
> status
> clear
Expected: Output area emptied
```

---

## Phase T3: Block Controls & Focus

**Goal:** Add minimize/remove to blocks. Basic focus management.

### T3.1: Block Controls UI

**Tasks:**
- [ ] Render `[−]` and `[×]` on each block (right-aligned)
- [ ] Track which block is "hovered" or "selected"
- [ ] Visual distinction for selected block (border color)

### T3.2: Block Interactions

**Tasks:**
- [ ] Tab key cycles focus: Input → Block 1 → Block 2 → ... → Input
- [ ] When block focused: `-` key minimizes, `x` or `d` removes
- [ ] Esc returns focus to input
- [ ] Enter on minimized block expands it

### T3.3: Minimize/Remove Logic

**Tasks:**
- [ ] `OutputBlock.minimized` toggle
- [ ] Minimized render: single line with summary
- [ ] Remove: delete block from `output` vec

### Acceptance Tests - Phase T3

```
T3-TEST-1: Block selection
──────────────────────────
# Run some commands to create blocks
> help
> status
# Press Tab
Expected: First block highlighted (different border)
# Press Tab again
Expected: Second block highlighted

T3-TEST-2: Minimize block
─────────────────────────
# With block selected, press '-'
Expected: Block collapses to single summary line
# Press '-' or Enter on minimized block
Expected: Block expands

T3-TEST-3: Remove block
───────────────────────
# With block selected, press 'x'
Expected: Block removed from output

T3-TEST-4: Return to input
──────────────────────────
# With block selected, press Esc
Expected: Focus returns to input line
```

---

## Phase T4: Input Line Enhancement

**Goal:** Full-featured input with completion and multi-line support.

### T4.1: Tab Completion

**Tasks:**
- [ ] Integrate `PanopticonCompleter` from existing code
- [ ] On Tab, get completions for current input
- [ ] Single completion: insert directly
- [ ] Multiple completions: show popup above input
- [ ] Render completion popup
- [ ] Arrow keys navigate popup, Enter/Tab accepts

### T4.2: Line Continuation

**Tasks:**
- [ ] Detect trailing `\` in input
- [ ] On Enter with `\`: add newline, show continuation prompt
- [ ] Multi-line input rendering
- [ ] Join continuation lines before parsing

### T4.3: Input Polish

**Tasks:**
- [ ] Ctrl+A: select all / move to start
- [ ] Ctrl+E: move to end
- [ ] Ctrl+U: clear line
- [ ] Ctrl+W: delete word backward
- [ ] Ctrl+L: clear output area

### Acceptance Tests - Phase T4

```
T4-TEST-1: Tab completion
─────────────────────────
> work<Tab>
Expected: Completes to "workspace"

> workspace <Tab>
Expected: Popup shows: list, select, schema

T4-TEST-2: Line continuation
────────────────────────────
> query test = "T \
Expected: Continuation prompt appears
. | take 10"
Expected: Command parsed as single multi-line query

T4-TEST-3: Ctrl shortcuts
─────────────────────────
> some text here
# Ctrl+A
Expected: Cursor moves to start
# Ctrl+U
Expected: Line cleared
```

---

## Phase T5: Widget Migration

**Goal:** Migrate existing TUI components to inline widgets.

### T5.1: Widget Traits

**Tasks:**
- [ ] Define `InteractiveWidget` trait
- [ ] Define `StaticWidget` trait
- [ ] Add `BlockContent::Widget` and `BlockContent::Interactive` variants
- [ ] Update `CommandOutput` with widget variants

### T5.2: Editor Widget

**Tasks:**
- [ ] Adapt `editor/widget.rs` to implement `InteractiveWidget`
- [ ] Render inline (not alternate screen)
- [ ] Capture focus when active
- [ ] Return result on Ctrl+D, cancel on Esc
- [ ] Update `query <name>` (no value) to return editor widget

### T5.3: Workspace Selector Widget

**Tasks:**
- [ ] Create multi-select checkbox list widget
- [ ] Implement for `workspace select` command
- [ ] Space toggles selection, Enter confirms

### T5.4: Progress Widget

**Tasks:**
- [ ] Create live-updating progress panel
- [ ] Show step status (pending, running, complete, failed)
- [ ] Update via channel from execution
- [ ] Use for `run` command

### T5.5: Data Table Widget

**Tasks:**
- [ ] Create scrollable table widget
- [ ] Column headers, row data
- [ ] Horizontal and vertical scroll
- [ ] Use for `sample` and `results` commands

### Acceptance Tests - Phase T5

```
T5-TEST-1: Inline editor
────────────────────────
> query analysis
Expected: Editor appears inline in output area
Expected: Other blocks still visible above
# Type query, press Ctrl+D
Expected: Editor replaced with success message

T5-TEST-2: Workspace selector
─────────────────────────────
> workspace select
Expected: Inline checkbox list
# Space to toggle, Enter to confirm
Expected: Selection applied, widget closes

T5-TEST-3: Progress display
───────────────────────────
> run
Expected: Inline progress panel with live updates

T5-TEST-4: Data table
─────────────────────
> sample events
Expected: Scrollable table with results
```

---

## Phase T6: Polish & Parity

**Goal:** Feature parity with current REPL, cleanup.

### T6.1: Feature Parity Verification

**Tasks:**
- [ ] Test all commands from checklist
- [ ] Fix any behavioral differences
- [ ] Match error handling and messages

### T6.2: Background Operations

**Tasks:**
- [ ] Background workspace discovery on startup
- [ ] Notifications for async completions
- [ ] Job completion notifications

### T6.3: Cleanup

**Tasks:**
- [ ] Remove `clap_repl` dependency
- [ ] Remove `reedline` dependency (if not used)
- [ ] Delete legacy REPL code
- [ ] Remove feature flag, make TUI default
- [ ] Update documentation

### Command Parity Checklist

**Pack Authoring:**
- [ ] `input <name> [= "<value>"]`
- [ ] `query <name> [= "<kql>"]`
- [ ] `inputs`
- [ ] `steps [--show <name>]`
- [ ] `remove <name>`
- [ ] `edit <name>`
- [ ] `info`
- [ ] `new [--name]`

**Exploration:**
- [ ] `revert [N]`
- [ ] `checkpoint save|restore|list|delete`
- [ ] `trace [--tree]`

**Workspace & Pack:**
- [ ] `workspace list`
- [ ] `workspace select <name>`
- [ ] `workspace schema [--capture]`
- [ ] `pack load <path>`

**Execution:**
- [ ] `run [path] [--query] [--all]`
- [ ] `sample <step> [--limit]`
- [ ] `validate [step] [--query]`
- [ ] `jobs`
- [ ] `results [job_id]`
- [ ] `history`

**Other:**
- [ ] `config [key] [value]`
- [ ] `status`
- [ ] `clear`
- [ ] `help [command]`
- [ ] `exit`

---

## Summary

| Phase | Focus | Key Deliverable |
|-------|-------|-----------------|
| **T1** | App Shell | Ratatui app, status bar, output area, input line |
| **T2** | Commands | Clap parsing, execution, text output blocks |
| **T3** | Block Controls | Minimize/remove, focus cycling |
| **T4** | Input Enhancement | Completion popup, history, multi-line |
| **T5** | Widgets | Inline editor, selector, progress, table |
| **T6** | Polish | Feature parity, cleanup legacy code |

### Phase Dependencies

```
T1 ──► T2 ──► T3 ──┬──► T4
                   │
                   └──► T5 ──► T6
```

### Estimated Scope

| Phase | New Files | Modified Files | Complexity |
|-------|-----------|----------------|------------|
| T1 | ~8 | 2 (main.rs, Cargo.toml) | Medium |
| T2 | 0 | ~3 (commands/mod.rs, tui/app.rs) | Low |
| T3 | 0 | ~2 (tui/ui/block.rs, tui/app.rs) | Low |
| T4 | ~1 | ~2 (tui/ui/input_line.rs) | Medium |
| T5 | ~4 | ~4 (commands that return widgets) | High |
| T6 | 0 | Cleanup only | Low |

---

## Open Questions

Resolved during planning:
- ✅ Development strategy: Module + feature flag
- ✅ Block lifecycle: User-controlled minimize/remove
- ✅ Scroll behavior: Terminal-like (stick to bottom)
- ✅ Focus during widget: Input non-interactive
- ✅ Progress indicators: Removed println! from commands; proper progress widgets in Phase T5

Still open:
- [ ] Mouse support? (Defer to post-T6)
- [ ] Block count limit? (Start unlimited, add pruning if needed)
- [ ] Persist output across restart? (No for v1)

## Technical Notes

### Progress Indicators (Phase T5)

Commands with long-running operations (workspace discovery, query execution) previously used `println!` for progress feedback. These have been removed as they conflict with TUI rendering.

In Phase T5, implement progress as inline widgets:
- `workspace list` / `workspace select`: Show spinner during discovery
- `run` / `sample`: Show step-by-step progress panel with status icons
- Long operations should update a progress block rather than print

TODO markers added in:
- `commands/workspace.rs` (3 locations)
- `commands/run.rs` (3 locations)
