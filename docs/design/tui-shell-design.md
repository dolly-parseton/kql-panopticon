# TUI Shell Design

*Created: December 2024*
*Status: Implemented*

## Overview

The KQL Panopticon REPL uses a full ratatui TUI application that owns the terminal, providing inline widgets, scrollable output, and unified rendering.

### Layout

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

## Design Principles

1. **Scrolling document model** - Output is a stream of blocks, not fixed regions
2. **User-controlled cleanup** - Every block has minimize/remove affordance
3. **Terminal-like scroll** - Auto-scroll at bottom, pause when scrolled up
4. **Focus is binary** - Either input line or a widget has focus
5. **Inline widgets** - Widgets render in output area, not alternate screen

---

## Architecture

### Module Structure

```
src/tui/
├── mod.rs              # Module exports, run() entry point
├── app.rs              # App state, main loop, command handling
├── event.rs            # Crossterm event handling
├── active_job.rs       # Background job tracking
├── undo_stack.rs       # Input undo/redo
├── events/             # TUI event system
│   ├── mod.rs
│   ├── tui_event.rs    # TuiEvent enum
│   └── widget_event.rs # Widget completion events
├── ui/                 # Rendering components
│   ├── mod.rs
│   ├── status_bar.rs   # Top status bar
│   ├── output_area.rs  # Scrollable block container
│   ├── input_line.rs   # Command input with cursor
│   ├── block.rs        # OutputBlock rendering
│   ├── completion_popup.rs  # Tab completion UI
│   ├── syntax_highlight.rs  # Command syntax coloring
│   └── ansi_parser.rs  # ANSI escape code → ratatui styles
└── widgets/            # Interactive/static widgets
    ├── mod.rs
    ├── editor.rs       # KQL/YAML editor widget
    ├── input_def.rs    # Input definition form
    ├── input_form.rs   # Runtime input collection
    ├── results.rs      # Query results table
    ├── run_progress.rs # Execution progress panel
    └── workspace_selector.rs  # Multi-select workspace list
```

### Data Model

```rust
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
    pub focused_block: Option<usize>,
}

pub struct OutputBlock {
    pub id: BlockId,
    pub content: BlockContent,
    pub minimized: bool,
}

pub enum BlockContent {
    Command(String),      // Echoed command (syntax highlighted)
    Text(String),         // Text output (ANSI parsed)
    Error(String),        // Error message
    Widget(WidgetState),  // Interactive widget
}

pub enum Focus {
    Input,
    Block(usize),
}
```

---

## UI Components

### Status Bar

Single line at top showing session state:
- Application name
- Step count, input count
- Selected workspace(s)
- Schema capture status

### Output Area

Scrollable container of `OutputBlock`s:
- Each block has `[−]` minimize and `[×]` remove controls
- Focused block shows `▶` indicator
- Minimized blocks collapse to single line with summary
- ANSI escape codes in output converted to ratatui styles

### Input Line

Command input at bottom:
- Prompt shows `> ` (or continuation `. ` for multi-line)
- Syntax highlighting for commands as typed
- Tab completion with popup
- Command history (up/down arrows)
- Line continuation with trailing `\`

---

## Widget System

### Widget Trait

```rust
pub trait InteractiveWidget {
    fn render(&mut self, frame: &mut Frame, area: Rect);
    fn handle_input(&mut self, event: KeyEvent) -> WidgetAction;
    fn preferred_height(&self) -> u16;
    fn is_active(&self) -> bool;
}

pub enum WidgetAction {
    Consumed,
    Ignored,
    Complete(WidgetResult),
    Cancelled,
}
```

### Available Widgets

| Widget | Command | Purpose |
|--------|---------|---------|
| `TuiEditor` | `query <name>`, `input <name>` | KQL/YAML editing with syntax highlighting |
| `InputDefWidget` | `input <name>` | Define input parameters |
| `InputFormWidget` | `run` | Collect runtime input values |
| `RunProgressWidget` | `run` | Live execution progress |
| `ResultsWidget` | `peek`, `sample` | View query results table |
| `WorkspaceSelectorWidget` | `workspace select` | Multi-select workspace list |

---

## Event Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Crossterm  │────►│    App      │────►│   Render    │
│   Events    │     │  handle()   │     │             │
└─────────────┘     └─────────────┘     └─────────────┘
                          │
                          ▼
              ┌─────────────────────┐
              │   Command Execute   │
              │  (async via tokio)  │
              └─────────────────────┘
                          │
                          ▼
              ┌─────────────────────┐
              │    TuiEvent         │
              │  (progress, widget) │
              └─────────────────────┘
```

### Key Bindings

**Input Mode:**
| Key | Action |
|-----|--------|
| Enter | Execute command |
| Tab | Trigger completion |
| Up/Down | Command history |
| Ctrl+C | Clear input |
| Ctrl+L | Clear output |
| Ctrl+D | Exit |
| `\` + Enter | Line continuation |

**Block Focus:**
| Key | Action |
|-----|--------|
| Tab | Cycle focus |
| Esc | Return to input |
| `-` | Minimize/expand block |
| `x`, `d` | Remove block |
| Enter | Activate widget (if applicable) |

---

## Syntax Highlighting

### Command Input

Commands are syntax highlighted as typed:

| Token | Color | Examples |
|-------|-------|----------|
| Command | Cyan | `workspace`, `query`, `run` |
| Subcommand | Blue | `select`, `load`, `schema` |
| Flag | Yellow | `--show`, `-q`, `--all` |
| Argument | White | values, names |
| String | Green | quoted strings |
| Operator | Magenta | `=` |
| Unknown | Red | invalid commands |

### ANSI Output

Command output with ANSI escape codes is parsed and converted to ratatui styles, supporting:
- Standard colors (30-37, 90-97)
- Bold, dim, italic, underline modifiers
- Reset sequences

---

## Nice-to-Haves / Future Work

### Mouse Support
- Click to focus blocks
- Click minimize/remove buttons
- Scroll wheel for output area
- Click to position cursor in input

### Performance
- Virtual scrolling for large output areas
- Block count limit with auto-pruning oldest
- Lazy rendering for off-screen blocks

### Persistence
- Save/restore output history across sessions
- Persist command history

### Visual Polish
- Animated spinners for async operations
- Smooth scrolling
- Block collapse/expand animations
- Color theme customization

---

## References

- [ratatui documentation](https://ratatui.rs/)
- [crossterm documentation](https://docs.rs/crossterm/)
- [tui-textarea](https://docs.rs/tui-textarea/)
