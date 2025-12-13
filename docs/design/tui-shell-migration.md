# TUI Shell Migration Plan

*Created: December 2024*
*Status: Planning*

## Overview

Migrate from `clap_repl` + `reedline` to a full ratatui TUI application. The REPL becomes a component within a managed terminal interface, enabling inline widgets, scrollable output, and unified rendering.

### Motivation

The current architecture mixes `println!` output with TUI widgets (editor, input form), causing:
- Screen coordination conflicts between reedline and ratatui
- Inability to render inline widgets (tables, progress, selectors) in command output
- Progress updates fighting with prompt redraw

### Target Architecture

```
┌─ Status Bar ────────────────────────────────────────────┐
│ KQL Panopticon │ Z steps │ W inputs │ X ws │ Y schemas │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  > workspace list                        [−] [×]        │  ← OutputBlock::Command
│  ....                                                   │  ← OutputBlock::Text
│                                                         │
│  > workspace select                      [−] [×]        │
│  ┌─────────────────────────────────────┐               │  ← OutputBlock::Widget
│  │ [x] sentinel-prod                   │               │
│  │ [ ] sentinel-dev                    │               │
│  │ [ ] sentinel-test                   │               │
│  └─────────────────────────────────────┘               │
│                                                         │
│  > run --all                             [−] [×]        │
│  ┌─ Running / Completed ───────────────┐               │
│  │ ✓ step1  234 rows  1.2s             │               │
│  │ ⟳ step2  ...                        │               │
│  └─────────────────────────────────────┘               │
│                                                         │  ↕ Scrollable
├─────────────────────────────────────────────────────────┤
│ > _                                                     │  ← Input line
└─────────────────────────────────────────────────────────┘
```

### Design Principles

1. **Scrolling document model** - Output is a stream of blocks, not fixed regions
2. **User-controlled cleanup** - Every block has minimize/remove affordance
3. **Terminal-like scroll** - Auto-scroll at bottom, pause when scrolled up
4. **Focus is binary** - Either input line or a widget has focus
5. **Text-first migration** - Migrate with text blocks, add widgets incrementally

---

## Phase T1: App Shell & Event Loop

**Goal:** Basic ratatui application with status bar, output area, and input line. No command execution yet.

### Data Model

```rust
// src/tui/app.rs
pub struct App {
    /// Application state
    state: AppState,
    /// Shared REPL context (workspaces, session, etc.)
    ctx: SharedContext,
    /// Should quit
    should_quit: bool,
}

pub struct AppState {
    /// Output blocks (scrollable)
    output: Vec<OutputBlock>,
    /// Current scroll position (0 = bottom)
    scroll_offset: usize,
    /// Input line state
    input: InputState,
    /// Current focus
    focus: Focus,
}

pub struct OutputBlock {
    /// Unique identifier
    id: BlockId,
    /// Block content
    content: BlockContent,
    /// Display state
    minimized: bool,
}

pub enum BlockContent {
    /// Command that was entered (echo)
    Command(String),
    /// Simple text output (multi-line supported)
    Text(StyledText),
    /// Interactive widget (captures focus when active)
    Widget(Box<dyn InteractiveWidget>),
    /// Static rendered widget (table, etc.)
    Rendered(Box<dyn StaticWidget>),
}

pub enum Focus {
    /// Input line has focus
    Input,
    /// A widget has focus (by block id)
    Widget(BlockId),
}

pub struct InputState {
    /// Text buffer
    buffer: String,
    /// Cursor position
    cursor: usize,
    /// Command history
    history: Vec<String>,
    /// History navigation index
    history_index: Option<usize>,
}
```

### Implementation Tasks

- [ ] **T1.1** Create `src/tui/mod.rs` module structure
- [ ] **T1.2** Create `src/tui/app.rs` with `App` struct and `run()` method
- [ ] **T1.3** Implement basic event loop (crossterm events → app state)
- [ ] **T1.4** Create `src/tui/status_bar.rs` - render status bar from context
- [ ] **T1.5** Create `src/tui/output_area.rs` - render scrollable output blocks
- [ ] **T1.6** Create `src/tui/input_line.rs` - basic text input with cursor
- [ ] **T1.7** Implement scroll behavior (terminal-like: stick to bottom unless scrolled)
- [ ] **T1.8** Wire up quit command (Ctrl+D or `exit`)
- [ ] **T1.9** Handle terminal resize events

### Files Created

| File | Purpose |
|------|---------|
| `src/tui/mod.rs` | Module exports |
| `src/tui/app.rs` | Main application state and loop |
| `src/tui/status_bar.rs` | Status bar widget |
| `src/tui/output_area.rs` | Scrollable output region |
| `src/tui/input_line.rs` | Command input handling |
| `src/tui/block.rs` | `OutputBlock`, `BlockContent` types |
| `src/tui/focus.rs` | Focus management |

### Acceptance Criteria

```
TEST T1.1: App launches and renders
──────────────────────────────────
$ cargo run
Expected: Full-screen TUI with:
  - Status bar at top
  - Empty output area (or welcome message)
  - Input line at bottom with cursor
Verify: Clean render, no artifacts

TEST T1.2: Text input works
───────────────────────────
# Type "hello world"
Expected: Text appears in input line, cursor moves
# Press backspace
Expected: Characters deleted

TEST T1.3: Scroll behavior
──────────────────────────
# Add many output blocks (manually for now)
# Scroll up with arrow keys or mouse
Expected: Output scrolls, input line stays fixed
# New output arrives
Expected: Does NOT auto-scroll (user is reading)
# Scroll to bottom
# New output arrives
Expected: Auto-scrolls to show new content

TEST T1.4: Quit works
─────────────────────
# Press Ctrl+D (or type 'exit' once wired)
Expected: App exits cleanly, terminal restored

TEST T1.5: Resize handling
──────────────────────────
# Resize terminal window
Expected: Layout adjusts, no corruption
```

---

## Phase T2: Command Parsing & Execution

**Goal:** Wire up clap command parsing. Commands execute and return text output.

### Changes to Command System

```rust
// src/commands/mod.rs - MODIFIED

/// Result of command execution
pub enum CommandResult {
    /// Output to display (replaces String-based variants)
    Output(CommandOutput),
    /// Clear screen (keep - just clears output blocks)
    Clear,
    /// Exit the REPL
    Exit,
}

/// Command output that becomes an OutputBlock
pub enum CommandOutput {
    /// Simple text (current behavior - easy migration)
    Text(String),
    /// Styled text with formatting
    Styled(StyledText),
    /// Widget that renders inline
    Widget(Box<dyn StaticWidget>),
    /// Interactive widget that captures focus
    Interactive(Box<dyn InteractiveWidget>),
    /// Multiple outputs (e.g., progress + results)
    Multi(Vec<CommandOutput>),
    /// No output (silent success)
    None,
}
```

### Implementation Tasks

- [ ] **T2.1** Update `CommandResult` enum (add `CommandOutput` type)
- [ ] **T2.2** Implement `CommandOutput::Text` conversion from existing handlers
- [ ] **T2.3** Wire clap parsing to input line (on Enter)
- [ ] **T2.4** Push command echo to output area (`OutputBlock::Command`)
- [ ] **T2.5** Execute command via existing `commands::execute()`
- [ ] **T2.6** Convert `CommandResult` → `OutputBlock` and push to output
- [ ] **T2.7** Implement command history (up/down arrows)
- [ ] **T2.8** Handle clap errors (help, parse errors) as output blocks
- [ ] **T2.9** Update `main.rs` to launch TUI app instead of clap_repl

### Migration Strategy

**Minimal changes to command handlers:**
- Existing handlers return `CommandResult::message(String)` or `CommandResult::output(String)`
- These convert directly to `CommandOutput::Text(String)`
- No handler changes needed for Phase T2

### Acceptance Criteria

```
TEST T2.1: Command execution
────────────────────────────
> status
Expected: Output block appears with status info

> help
Expected: Help text appears in output area

TEST T2.2: Command echo
───────────────────────
> workspace list
Expected: Both the command AND its output appear:
  > workspace list           ← echoed command
  Workspaces (3):            ← command output
    sentinel-prod
    ...

TEST T2.3: Command history
──────────────────────────
> input x = "1"
> input y = "2"
# Press Up arrow
Expected: Input line shows "input y = \"2\""
# Press Up arrow again
Expected: Input line shows "input x = \"1\""
# Press Down arrow
Expected: Returns to "input y = \"2\""

TEST T2.4: Parse errors
───────────────────────
> notacommand
Expected: Error output block with clap error

TEST T2.5: All existing commands work
─────────────────────────────────────
# Run through existing command suite
> input test = "value"
> query step = "T | take 10"
> inputs
> steps
> workspace list
> status
> help
Expected: All produce appropriate output blocks
```

---

## Phase T3: Block Controls & Focus

**Goal:** Add minimize/remove controls to blocks. Implement focus management for future widgets.

### Implementation Tasks

- [ ] **T3.1** Add minimize/remove affordance to block rendering (`[−] [×]`)
- [ ] **T3.2** Implement block minimize toggle (collapse to single line)
- [ ] **T3.3** Implement block remove (delete from output)
- [ ] **T3.4** Add keyboard shortcuts for block control (when block focused?)
- [ ] **T3.5** Implement focus system (`Focus::Input` vs `Focus::Widget`)
- [ ] **T3.6** Tab or click to focus blocks (optional - may defer mouse)
- [ ] **T3.7** Escape returns focus to input from widget
- [ ] **T3.8** Visual indication of focused block (border highlight)

### Acceptance Criteria

```
TEST T3.1: Minimize block
─────────────────────────
# Click [−] on output block (or keyboard shortcut)
Expected: Block collapses to single line showing summary
# Click [−] again (or [+])
Expected: Block expands to full content

TEST T3.2: Remove block
───────────────────────
# Click [×] on output block
Expected: Block removed from output area

TEST T3.3: Focus indication
───────────────────────────
# When widget block has focus
Expected: Border changes color/style to indicate focus
# Press Escape
Expected: Focus returns to input line
```

---

## Phase T4: Input Line Enhancement

**Goal:** Full-featured input line with completion and multi-line support.

### Implementation Tasks

- [ ] **T4.1** Integrate tab completion (reuse `PanopticonCompleter`)
- [ ] **T4.2** Render completion popup above input line
- [ ] **T4.3** Implement line continuation (`\` at end of line)
- [ ] **T4.4** Multi-line input rendering (continuation prompt)
- [ ] **T4.5** Syntax highlighting in input line (optional - nice to have)
- [ ] **T4.6** Ctrl+C to cancel current input
- [ ] **T4.7** Ctrl+L to clear output area

### Acceptance Criteria

```
TEST T4.1: Tab completion
─────────────────────────
> work<TAB>
Expected: Completes to "workspace"

> workspace sel<TAB>
Expected: Completes to "workspace select"

TEST T4.2: Completion popup
───────────────────────────
> workspace <TAB>
Expected: Popup shows available subcommands:
  list
  select
  schema

TEST T4.3: Line continuation
────────────────────────────
> query test = "T \
. | where A == 1"
Expected: Multi-line input with continuation prompt

TEST T4.4: Clear output
───────────────────────
# Press Ctrl+L
Expected: Output area cleared, input line preserved
```

---

## Phase T5: Widget Migration

**Goal:** Migrate existing TUI components to inline widgets.

### Widgets to Migrate

| Widget | Source | Inline Behavior |
|--------|--------|-----------------|
| KQL Editor | `editor/widget.rs` | Opens inline, captures focus |
| Input Form | `input_form/widget.rs` | Opens inline for `run` inputs |
| Workspace Selector | New | Multi-select checklist |
| Progress Display | `progress_display.rs` | Live-updating inline |
| Data Table | New | Scrollable table view |

### Implementation Tasks

- [ ] **T5.1** Define `InteractiveWidget` trait (focus, input handling, render)
- [ ] **T5.2** Define `StaticWidget` trait (render only)
- [ ] **T5.3** Migrate KQL Editor to `InteractiveWidget`
- [ ] **T5.4** Update `query <name>` (no value) to return `CommandOutput::Interactive`
- [ ] **T5.5** Create `WorkspaceSelector` widget
- [ ] **T5.6** Update `workspace select` to use selector widget
- [ ] **T5.7** Migrate `InputForm` to inline widget
- [ ] **T5.8** Create `DataTable` widget for results display
- [ ] **T5.9** Update `sample` command to return table widget
- [ ] **T5.10** Create `ProgressPanel` widget for `run` command

### Widget Trait Design

```rust
pub trait InteractiveWidget {
    /// Render the widget to a frame area
    fn render(&mut self, frame: &mut Frame, area: Rect);

    /// Handle input event, return true if consumed
    fn handle_input(&mut self, event: KeyEvent) -> WidgetAction;

    /// Get preferred height (for layout)
    fn preferred_height(&self) -> u16;

    /// Is the widget still active (wants focus)?
    fn is_active(&self) -> bool;

    /// Get final result when widget completes
    fn result(&self) -> Option<WidgetResult>;
}

pub enum WidgetAction {
    /// Event consumed, continue
    Consumed,
    /// Event not consumed, pass to parent
    Ignored,
    /// Widget completed (success)
    Complete(WidgetResult),
    /// Widget cancelled
    Cancelled,
}

pub trait StaticWidget {
    /// Render the widget
    fn render(&self, frame: &mut Frame, area: Rect);

    /// Get preferred height
    fn preferred_height(&self) -> u16;

    /// Get summary line for minimized view
    fn summary(&self) -> String;
}
```

### Acceptance Criteria

```
TEST T5.1: Inline editor
────────────────────────
> query analysis
Expected: KQL editor appears INLINE in output area
  - Not full-screen alternate buffer
  - Other output blocks still visible above
  - Editor has focus, input line dimmed

# Press Ctrl+D to save
Expected: Editor replaced with success message
  - Or editor collapses to summary

# Press Esc to cancel
Expected: Editor removed, no step created

TEST T5.2: Workspace selector
─────────────────────────────
> workspace select
Expected: Inline checkbox list appears
  [ ] sentinel-prod
  [ ] sentinel-dev
  [x] sentinel-test
Expected: Space toggles, Enter confirms

TEST T5.3: Progress widget
──────────────────────────
> run --all
Expected: Inline progress panel shows:
  ┌─ Running ─────────────────────┐
  │ ✓ step1  234 rows  1.2s       │
  │ ⟳ step2  running...           │
  │ ○ step3  pending              │
  └───────────────────────────────┘
Expected: Updates in real-time

TEST T5.4: Data table
─────────────────────
> sample events
Expected: Inline scrollable table
  ┌─────────────┬───────────┬─────────┐
  │ Account     │ IpAddress │ Count   │
  ├─────────────┼───────────┼─────────┤
  │ admin       │ 10.0.0.1  │ 23      │
  │ user1       │ 10.0.0.2  │ 12      │
  └─────────────┴───────────┴─────────┘
Expected: Arrow keys scroll if data exceeds height
```

---

## Phase T6: Polish & Feature Parity

**Goal:** Ensure all current functionality works, add finishing touches.

### Implementation Tasks

- [ ] **T6.1** Verify all commands from current REPL work
- [ ] **T6.2** Background workspace discovery with notification
- [ ] **T6.3** External notifications (job complete, etc.)
- [ ] **T6.4** Mouse support (optional - click blocks, scroll)
- [ ] **T6.5** Status bar dynamic updates (context changes)
- [ ] **T6.6** Performance optimization (large output areas)
- [ ] **T6.7** Clean up / remove old clap_repl code
- [ ] **T6.8** Update documentation

### Feature Parity Checklist

All current commands must work:

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
- [ ] `workspace select`
- [ ] `workspace schema [--capture]`
- [ ] `pack load <path>`

**Execution:**
- [ ] `run [--query] [--all]`
- [ ] `sample <step>`
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
| T1 | App Shell | Basic TUI with status bar, output area, input line |
| T2 | Commands | Clap parsing, command execution, text output |
| T3 | Block Controls | Minimize/remove, focus management |
| T4 | Input Enhancement | Completion, history, multi-line |
| T5 | Widget Migration | Inline editor, selector, progress, table |
| T6 | Polish | Feature parity, cleanup, documentation |

### Dependencies

```
T1 ──► T2 ──► T3 ──► T4
              │
              └──► T5 ──► T6
```

- T1-T2: Sequential (need app shell before commands)
- T3-T4: Can partially parallelize (block controls vs input features)
- T5: Requires T3 (focus management)
- T6: Requires all previous phases

### What Stays vs Changes

**Keeps (no changes):**
- `clap` derive command definitions (`commands/mod.rs` enum)
- Command handler logic (`commands/*.rs` internal logic)
- `SharedContext` and all context state
- `PackSession`, `StateGraph`, validation logic
- Core library integration

**Adapts:**
- `CommandResult` → adds `CommandOutput` type
- Existing handlers return `CommandOutput::Text(String)` initially
- Editor widget → becomes inline `InteractiveWidget`

**Replaces:**
- `clap_repl::ClapEditor` → `tui::App`
- `reedline` prompt/input → `tui::InputLine`
- `println!` output → `OutputBlock` stream

**Removes (after migration):**
- `clap_repl` dependency
- `reedline` dependency (unless keeping for input line)
- `ExternalPrinter` usage
- Alternate screen editor mode

---

## Open Questions

1. **Mouse support priority?** - Nice for clicking blocks, but keyboard-first is fine for v1

2. **reedline vs tui-textarea for input?** - tui-textarea is simpler and already in use. reedline has better completion/history but is designed for standalone use.

3. **Block persistence?** - Should blocks survive app restart? (Probably not for v1)

4. **Maximum output blocks?** - Need a limit to prevent memory growth? (Auto-prune oldest?)
