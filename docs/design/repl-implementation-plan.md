# REPL Pack Authoring: Implementation Plan

*Created: December 2024*
*Design: [REPL Pack Authoring](repl-pack-authoring.md)*

## Overview

This plan breaks the REPL pack authoring implementation into incremental phases. Each phase delivers testable functionality with explicit user acceptance tests to validate both correctness and UX quality (readability, visual feedback, discoverability).

---

## Phase 1: Core State Model & Basic Commands ✅

**Goal:** Establish the session state model and basic `input`/`query` commands with inline values.

**Status:** Complete (December 2024)

### Implementation Tasks

- [x] **1.1** Define `PackSession` struct with `inputs: IndexMap<String, InputDef>` and `steps: IndexMap<String, StepDef>`
- [x] **1.2** Implement `InputDef` and `StepDef` types with serialization
- [x] **1.3** Add `input <name> = "<value>"` command (inline only)
- [x] **1.4** Add `query <name> = "<kql>"` command (inline only, single line)
- [x] **1.5** Add `inputs` command to list defined inputs
- [x] **1.6** Add `steps` command to list defined steps
- [x] **1.7** Add `remove <name>` command
- [x] **1.8** Add `info` command showing session summary
- [x] **1.9** Implement basic variable reference parsing (`{{inputs.x}}`, `{{step.x}}`)
- [x] **1.10** Auto-detect dependencies from `{{references}}`

### Files Created/Modified

| File | Changes |
|------|---------|
| `src/session.rs` | New - `PackSession`, `InputDef`, `StepDef`, `VarRef`, reference parsing |
| `src/commands/authoring.rs` | New - command handlers for all authoring commands |
| `src/commands/mod.rs` | Added commands to enum and execute dispatch |
| `src/context.rs` | Added `PackSession` to `ReplContext` |
| `Cargo.toml` | Added `indexmap`, `regex` dependencies |

### User Tests

```
TEST 1.1: Define inline input
─────────────────────────────
> input threat_ip = "10.0.0.1"
Expected: ✓ Input defined: threat_ip (string)
Verify: Clear success message, type inferred

TEST 1.2: Define inline query
─────────────────────────────
> query events = "SecurityEvent | take 10"
Expected: ✓ Step defined: events
Verify: Clear success message

TEST 1.3: List inputs
─────────────────────
> inputs
Expected:
  Inputs (1):
    threat_ip  string  value: "10.0.0.1"
Verify: Readable table format, aligned columns

TEST 1.4: List steps
────────────────────
> steps
Expected:
  Steps (1):
    1. events  →  (no dependencies)
Verify: Numbered list, dependency info shown

TEST 1.5: Query referencing input
─────────────────────────────────
> query filtered = "SecurityEvent | where IpAddress == '{{inputs.threat_ip}}'"
Expected: ✓ Step defined: filtered
         refs: [inputs.threat_ip]
Verify: Reference detected and displayed

TEST 1.6: Query referencing step
────────────────────────────────
> query events = "SecurityEvent | take 100"
> query summary = "{{events}} | summarize count()"
Expected: ✓ Step defined: summary
         depends: [events]
Verify: Dependency chain detected

TEST 1.7: Remove input
──────────────────────
> remove threat_ip
Expected: ✓ Removed: threat_ip
> inputs
Expected: Inputs (0): (none defined)

TEST 1.8: Info command
──────────────────────
> info
Expected:
  Pack: (untitled)
  Inputs: 1 defined
  Steps: 2 defined
  Workspace: (none selected)
Verify: Clear summary, actionable info
```

### Exit Criteria
- [ ] All 8 user tests pass
- [ ] Visual output reviewed for readability
- [ ] Help text updated for new commands

---

## Phase 2: Multi-line Input & Line Continuation ✅

**Goal:** Support multi-line queries using `\` continuation.

**Status:** Complete (December 2024)

### Implementation Tasks

- [x] **2.1** Implement line continuation parser (trailing `\` continues to next line)
- [x] **2.2** Update prompt to show continuation indicator (`. `)
- [x] **2.3** Handle multi-line input in reedline/clap_repl integration
- [x] **2.4** Preserve formatting in stored queries

### Files Created/Modified

| File | Changes |
|------|---------|
| `src/validator.rs` | New - `LineContinuationValidator`, `join_continuation_lines()` |
| `src/main.rs` | Added validator integration with ClapEditor |
| `src/prompt.rs` | Updated multiline indicator to `. ` |
| `src/commands/mod.rs` | Added `join_continuation_lines` processing, `--show` flag to `steps` |
| `src/commands/authoring.rs` | Updated `list_steps` to support `--show <name>` |

### User Tests

```
TEST 2.1: Line continuation
───────────────────────────
> query complex = "SecurityEvent \
.   | where EventID == 4625 \
.   | where TimeGenerated > ago(1h) \
.   | project Account, IpAddress"
Expected: ✓ Step defined: complex
Verify: Continuation prompt shown (. or ...)
Verify: Query stored with proper formatting

TEST 2.2: Continuation in middle of line
─────────────────────────────────────────
> query test = "T | where \
.   A == 1"
Expected: Works correctly, whitespace handled

TEST 2.3: Cancel continuation
─────────────────────────────
> query broken = "T | where \
. <Ctrl+C>
Expected: Input cancelled, back to normal prompt

TEST 2.4: View multi-line step
──────────────────────────────
> steps --show complex
Expected: Query displayed with preserved formatting
```

### Automated Tests

```rust
// validator::tests (8 tests)
test_needs_continuation_trailing_backslash  // Detects "test\" as incomplete
test_needs_continuation_no_backslash        // "test" is complete
test_needs_continuation_escaped_backslash   // "test\\" is complete (escaped)
test_join_simple                            // Basic continuation joining
test_join_multiple_lines                    // Multiple continuation lines
test_join_with_indentation                  // Strips leading whitespace
test_join_preserves_non_continuation_backslash // Keeps \\ intact
test_join_windows_line_endings              // Handles \r\n
```

### Exit Criteria
- [x] All 4 user tests pass
- [x] Continuation prompt visually distinct (`. `)
- [x] Ctrl+C cancellation works cleanly (handled by reedline)
- [x] 8 automated validator tests pass

---

## Phase 3: Exploring Interpreter (State Graph) ✅

**Goal:** Implement state snapshots, backtracking, and checkpoints.

**Status:** Complete (December 2024)

### Implementation Tasks

- [x] **3.1** Implement `StateGraph` with snapshot storage
- [x] **3.2** Create snapshot on each state-modifying command
- [x] **3.3** Update prompt to show state ID (`#N`)
- [x] **3.4** Implement `revert [N]` command
- [x] **3.5** Implement `checkpoint save <name>` command
- [x] **3.6** Implement `checkpoint restore <name>` command
- [x] **3.7** Implement `checkpoint list` command
- [x] **3.8** Implement `trace` command showing history (linear and `--tree` modes)

### Files Created/Modified

| File | Changes |
|------|---------|
| `src/state_graph.rs` | New - `StateGraph`, `StateId`, `Snapshot`, `TraceEntry`, ancestry tracking |
| `src/commands/exploration.rs` | New - `revert`, `checkpoint`, `trace` command handlers |
| `src/commands/mod.rs` | Added `Revert`, `Checkpoint`, `Trace` commands to enum and dispatch |
| `src/context.rs` | Replaced `PackSession` with `StateGraph`, added state management methods |
| `src/prompt.rs` | Updated to show `#<state_id>` and `[checkpoint]` in prompt |
| `src/commands/authoring.rs` | Modified `input`, `query`, `remove` to commit state before modification |

### User Tests

```
TEST 3.1: State ID in prompt
────────────────────────────
> input x = "1"
panopticon #1 > input y = "2"
panopticon #2 >
Verify: State ID increments with each command

TEST 3.2: Basic revert
──────────────────────
panopticon #2 > inputs
  x, y
panopticon #2 > revert 1
Reverted to state #1
panopticon #1 > inputs
  x  (y should be gone)
Verify: State actually restored, not just display

TEST 3.3: Revert without argument
─────────────────────────────────
panopticon #5 > revert
Expected: Reverted to state #4 (previous)

TEST 3.4: Checkpoint save/restore
─────────────────────────────────
panopticon #3 > checkpoint save before-analysis
✓ Checkpoint saved: before-analysis (state #3)
panopticon #3 > query step1 = "..."
panopticon #4 > query step2 = "..."
panopticon #5 > checkpoint restore before-analysis
Restored checkpoint: before-analysis
panopticon #3 > steps
  (step1 and step2 should not exist)

TEST 3.5: Checkpoint in prompt
──────────────────────────────
panopticon #3 > checkpoint save validated
panopticon #3 [validated] >
Verify: Checkpoint name shown in prompt

TEST 3.6: Trace command
───────────────────────
panopticon #5 > trace
Expected:
  #0 (initial)
  #1 input x = "1"
  #2 input y = "2"
  #3 query step1 = "..."  [checkpoint: validated]
  │
  ├─#4 query step2 = "..."
  │
  └─#5 (current)
Verify: Tree structure for branches, checkpoints marked

TEST 3.7: Invalid revert
────────────────────────
panopticon #3 > revert 99
Expected: Error: State #99 does not exist
```

### Automated Tests

```rust
// state_graph::tests (10 tests)
test_initial_state              // State #0 with empty session
test_commit_advances_state      // Commit creates new state ID
test_revert_to_previous         // Revert restores session state
test_revert_without_arg         // Revert to parent state
test_checkpoints                // Save/restore named checkpoints
test_list_checkpoints           // List checkpoints sorted by state ID
test_ancestry_chain             // Build path from root to state
test_trace                      // Generate trace entries
test_branching                  // Multiple children from same parent
test_reset                      // Reset clears all history

// context::tests (2 new tests)
test_state_graph_integration    // Context delegates to StateGraph
test_checkpoints                // Context checkpoint methods work
```

### Exit Criteria
- [x] All 7 user tests pass
- [x] Prompt clearly shows state ID
- [x] Trace output readable and shows branches (`trace --tree`)
- [x] No state leakage after revert
- [x] 12 automated tests pass

---

## Phase 4: Validation Integration ✅

**Goal:** Validate queries on definition (syntax and schema).

**Status:** Core complete (December 2024). Schema-aware validation wired.

### Implementation Tasks

- [x] **4.1** Integrate KQL validator for syntax checking
- [ ] **4.2** Validate `{{reference}}` targets exist
- [ ] **4.3** Infer and display output schema when possible
- [x] **4.4** Add `validate [step]` command for explicit validation
- [x] **4.5** Show validation status in `steps` output (checkmarks)
- [ ] **4.6** Block invalid step definition with clear error
- [x] **4.7** Schema-aware validation when workspace schema captured

### User Tests

```
TEST 4.1: Syntax validation on define
─────────────────────────────────────
> query bad = "SecurityEvent | where"
Expected: ✗ Invalid syntax
         Line 1, Col 25: Expected expression after 'where'
Verify: Step NOT added, clear error location

TEST 4.2: Unknown reference validation
──────────────────────────────────────
> query orphan = "{{nonexistent}} | take 10"
Expected: ✗ Unknown reference: nonexistent
         Hint: Define with 'input nonexistent' or 'query nonexistent'
Verify: Helpful hint provided

TEST 4.3: Valid query with schema inference
───────────────────────────────────────────
> query events = "SecurityEvent | project TimeGenerated, Account"
Expected: ✓ Step defined: events
         schema: TimeGenerated:datetime, Account:string
Verify: Schema inferred and displayed

TEST 4.4: Explicit validate command
───────────────────────────────────
> validate
Expected:
  Validating all steps...
  ✓ events       valid
  ✓ summary      valid
  All 2 steps valid

TEST 4.5: Steps shows validation status
───────────────────────────────────────
> steps
Expected:
  Steps (2):
    1. ✓ events   → schema: TimeGenerated, Account
    2. ✓ summary  → depends: [events]
Verify: Checkmarks for valid steps

TEST 4.6: Validate with workspace schema
────────────────────────────────────────
> workspace select prod-sentinel
> query bad_column = "SecurityEvent | where FakeColumn == 1"
Expected: ✗ Schema error: Column 'FakeColumn' not found in SecurityEvent
         Available: TimeGenerated, EventID, Account, ...
Verify: Schema-aware validation when workspace selected
```

### Exit Criteria
- [ ] All 6 user tests pass
- [ ] Syntax errors show line/column
- [ ] Schema errors suggest alternatives
- [ ] Validation fast enough for interactive use

---

## Phase 5: TUI Editor Framework

**Goal:** Build a reusable TUI editor widget with syntax highlighting and completion support, then wire it up for KQL and YAML editing modes.

**Status:** Planning (December 2024)

**Design Reference:** [Previous TUI implementation](https://github.com/dolly-parseton/kql-panopticon/tree/main/src/tui)

### Architecture Overview

The editor is designed as a **generic widget** with pluggable language services:

```
┌─────────────────────────────────────────────────────────────────┐
│                         TuiEditor                               │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  Generic editor widget (tui-textarea based)              │   │
│  │  - Text buffer, cursor, selection, scrolling             │   │
│  │  - Line numbers, status bar                              │   │
│  │  - Completion popup rendering                            │   │
│  │  - Keybindings (Ctrl+D save, Esc cancel)                 │   │
│  └──────────────────────────────────────────────────────────┘   │
│         │                         │                             │
│         ▼                         ▼                             │
│  ┌─────────────┐           ┌─────────────┐                     │
│  │  KQL Mode   │           │  YAML Mode  │                     │
│  │  highlight  │           │  highlight  │                     │
│  │  complete   │           │  (simple)   │                     │
│  └──────┬──────┘           └──────┬──────┘                     │
└─────────┼─────────────────────────┼─────────────────────────────┘
          │                         │
          ▼                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    kql-panopticon-core                          │
│  KqlValidator::get_classifications() → ClassificationResult     │
│  KqlValidator::get_completions()     → CompletionResult         │
│  (FFI to Microsoft.Kusto.Language)                              │
└─────────────────────────────────────────────────────────────────┘
```

### Implementation Tasks

#### 5A: Core Editor Widget

- [ ] **5A.1** Add `tui-textarea` dependency to Cargo.toml
- [ ] **5A.2** Create `src/editor/mod.rs` - module structure
- [ ] **5A.3** Create `src/editor/widget.rs` - `TuiEditor` struct and configuration
- [ ] **5A.4** Implement editor layout (title bar, line numbers, content area, status bar)
- [ ] **5A.5** Implement keybindings: `Ctrl+D` save, `Esc` cancel, standard text editing
- [ ] **5A.6** Implement `run_editor()` function that takes over terminal and returns `EditorResult`
- [ ] **5A.7** Handle terminal resize events
- [ ] **5A.8** Implement cursor position tracking (line, column) in status bar

#### 5B: Syntax Highlighting

- [ ] **5B.1** Re-export `ClassificationKind`, `ClassifiedSpan` from core
- [ ] **5B.2** Create `src/editor/highlight.rs` - `Highlighter` trait
- [ ] **5B.3** Implement `KqlHighlighter` using `KqlValidator::get_classifications()`
- [ ] **5B.4** Define color scheme mapping `ClassificationKind` → ratatui `Style`:
  ```
  QueryOperator     → Cyan        (where, project, summarize)
  Keyword           → Magenta     (by, on, and, or)
  Table             → Yellow      (SecurityEvent, SigninLogs)
  Column            → Green       (TimeGenerated, Account)
  ScalarFunction    → Blue        (ago, now, datetime_diff)
  AggregateFunction → Blue+Bold   (count, sum, avg)
  StringLiteral     → Red         ("string values")
  Literal           → Cyan        (numbers, true/false)
  Comment           → DarkGray    (// comments)
  Operator          → White       (==, >, <, +, -)
  ```
- [ ] **5B.5** Implement `YamlHighlighter` (simple keyword-based for keys, strings, booleans)
- [ ] **5B.6** Apply highlighting spans to `tui-textarea` content
- [ ] **5B.7** Handle highlighting errors gracefully (fall back to plain text)

#### 5C: Completion Support

- [ ] **5C.1** Create `src/editor/completion.rs` - completion popup widget
- [ ] **5C.2** Implement `CompletionPopup` rendering (bordered list with selection)
- [ ] **5C.3** Integrate `KqlValidator::get_completions()` for KQL completion
- [ ] **5C.4** Add session-aware completion for `{{references}}`:
  - Detect `{{` trigger
  - Offer `inputs.<name>` for defined inputs
  - Offer `<step>` and `<step>.first.<column>` for defined steps
- [ ] **5C.5** Implement completion keybindings: `Tab` trigger/accept, `Esc` dismiss, arrows navigate
- [ ] **5C.6** Position popup relative to cursor (above or below depending on space)
- [ ] **5C.7** Handle completion insertion (replace trigger text with selected item)

#### 5D: KQL Editor Mode

- [ ] **5D.1** Create `src/editor/kql_mode.rs` - KQL-specific editor configuration
- [ ] **5D.2** Wire `KqlHighlighter` + completion into editor config
- [ ] **5D.3** Update `query <name>` command (no value) to launch KQL editor
- [ ] **5D.4** Pass session context for `{{reference}}` completion
- [ ] **5D.5** Pass workspace schema for table/column completion
- [ ] **5D.6** Validate on save, show errors in status bar, allow save anyway with warning

#### 5E: YAML Editor Mode

- [ ] **5E.1** Create `src/editor/yaml_mode.rs` - YAML-specific editor configuration
- [ ] **5E.2** Implement `YamlHighlighter` (keys, strings, numbers, booleans)
- [ ] **5E.3** Create YAML template for input definition:
  ```yaml
  name: {name}
  type: string        # string | int | bool | datetime | timespan
  description: ""
  required: true
  default: null
  ```
- [ ] **5E.4** Update `input <name>` command (no value) to launch YAML editor
- [ ] **5E.5** Parse YAML on save, validate structure, convert to `InputDef`

#### 5F: Edit Command & Integration

- [ ] **5F.1** Add `edit <name>` command to `commands/mod.rs`
- [ ] **5F.2** Implement `edit` handler - detect if input or step, launch appropriate editor
- [ ] **5F.3** Pre-populate editor with existing content
- [ ] **5F.4** Handle "not found" error gracefully
- [ ] **5F.5** Update state graph on successful edit (new state created)

### Files Created/Modified

| File | Changes |
|------|---------|
| `Cargo.toml` | Add `tui-textarea` dependency |
| `src/editor/mod.rs` | New - module exports |
| `src/editor/widget.rs` | New - `TuiEditor`, `EditorConfig`, `EditorResult` |
| `src/editor/highlight.rs` | New - `Highlighter` trait, `KqlHighlighter`, `YamlHighlighter` |
| `src/editor/completion.rs` | New - `CompletionPopup` widget, completion logic |
| `src/editor/kql_mode.rs` | New - KQL editor configuration |
| `src/editor/yaml_mode.rs` | New - YAML editor configuration |
| `src/commands/mod.rs` | Add `Edit` command variant |
| `src/commands/authoring.rs` | Update `input`/`query` to launch editors, add `edit` handler |
| `src/main.rs` | Add `editor` module |
| `kql-panopticon-core/src/validation/mod.rs` | Re-export classification/completion types |

### User Tests

```
TEST 5.1: KQL editor opens with highlighting
────────────────────────────────────────────
> query analysis
Expected: Full-screen editor opens:
┌─ query: analysis ──────────────────────────────────────────────┐
│   1 │                                                          │
│   ~ │                                                          │
├────────────────────────────────────────────────────────────────┤
│ Ln 1, Col 1 │ KQL │ [Ctrl+D save] [Esc cancel]                │
└────────────────────────────────────────────────────────────────┘
Verify: Clean editor, keybinding hints visible

TEST 5.2: KQL syntax highlighting
─────────────────────────────────
# Type in editor:
  SecurityEvent
  | where TimeGenerated > ago(1h)
  | project Account, IpAddress
Expected:
  - "SecurityEvent" in yellow (Table)
  - "where", "project" in cyan (QueryOperator)
  - "TimeGenerated", "Account", "IpAddress" in green (Column)
  - "ago" in blue (ScalarFunction)
  - "1h" in cyan (Literal)
Verify: Colors match scheme, updates as you type

TEST 5.3: KQL completion - operators
────────────────────────────────────
# In editor, type: SecurityEvent |<TAB>
Expected: Completion popup appears:
  ┌──────────────────────┐
  │ where                │  ← selected
  │ project              │
  │ summarize            │
  │ extend               │
  │ ...                  │
  └──────────────────────┘
Verify: Tab opens popup, arrows navigate, Tab/Enter accepts

TEST 5.4: KQL completion - columns (schema-aware)
─────────────────────────────────────────────────
# With workspace selected and schema captured:
# Type: SecurityEvent | project <TAB>
Expected: Column completion popup:
  TimeGenerated (datetime)
  EventID (int)
  Account (string)
  ...
Verify: Columns from captured schema appear

TEST 5.5: Reference completion
──────────────────────────────
# With inputs/steps defined:
# Type: | where IP == '{{<TAB>
Expected: Completion popup:
  inputs.threat_ip
  inputs.lookback
  events
  events.first.IpAddress
Verify: Session inputs and steps available

TEST 5.6: Editor save (Ctrl+D)
──────────────────────────────
# After typing query, press Ctrl+D
Expected:
  - Editor closes
  - Returns to REPL
  - "✓ Step defined: analysis" message
  - Query saved correctly
Verify: State updated, step appears in `steps` list

TEST 5.7: Editor cancel (Esc)
─────────────────────────────
# After typing query, press Esc
Expected:
  - Editor closes
  - Returns to REPL
  - No message (or "Cancelled")
  - No step created
Verify: No state change, clean return

TEST 5.8: Input editor with YAML template
─────────────────────────────────────────
> input threat_ip
Expected: YAML editor opens with template:
  name: threat_ip
  type: string        # string | int | bool | datetime | timespan
  description: ""
  required: true
  default: null
Verify: Template pre-populated, YAML highlighting

TEST 5.9: Input editor save and parse
─────────────────────────────────────
# Edit YAML to:
  name: threat_ip
  type: string
  description: "IP address to investigate"
  required: true
  default: null
# Press Ctrl+D
Expected: ✓ Input defined: threat_ip
         type: string, required
         description: IP address to investigate
Verify: YAML parsed correctly, InputDef created

TEST 5.10: Edit existing step
─────────────────────────────
> query events = "SecurityEvent | take 10"
> edit events
Expected: KQL editor opens with existing query pre-loaded
# Modify and save
Expected: Step updated, new state created

TEST 5.11: Edit existing input
──────────────────────────────
> input x = "value"
> edit x
Expected: YAML editor opens with current InputDef as YAML

TEST 5.12: Edit non-existent
────────────────────────────
> edit nonexistent
Expected: Error: No input or step named 'nonexistent'
```

### Automated Tests

```rust
// editor::highlight::tests
test_kql_highlighter_keywords        // QueryOperator spans detected
test_kql_highlighter_table           // Table names classified
test_kql_highlighter_functions       // Scalar/aggregate functions
test_kql_highlighter_strings         // String literals highlighted
test_kql_highlighter_empty           // Empty string handled
test_kql_highlighter_invalid         // Graceful fallback on error
test_yaml_highlighter_keys           // YAML keys highlighted
test_yaml_highlighter_values         // Strings, numbers, booleans

// editor::completion::tests
test_kql_completion_after_pipe       // Operators suggested
test_kql_completion_columns          // Column names with schema
test_reference_completion_inputs     // {{inputs.* suggestions
test_reference_completion_steps      // {{step.* suggestions
test_completion_popup_positioning    // Above/below cursor

// editor::widget::tests
test_editor_result_save              // Ctrl+D returns Saved
test_editor_result_cancel            // Esc returns Cancelled
test_cursor_position_tracking        // Line/col accurate
```

### Exit Criteria

- [ ] All 12 user tests pass
- [ ] Syntax highlighting matches color scheme for all token types
- [ ] Completion popup responsive (<100ms)
- [ ] Tab completion works for operators, columns, and references
- [ ] Editor cancellation leaves no partial state
- [ ] Edit command works for both inputs and steps
- [ ] YAML template parsing handles all input types
- [ ] Works correctly on terminal resize

---

## Phase 6: Execution (run, sample, results)

**Goal:** Execute steps with input prompting and result display.

### Implementation Tasks

- [ ] **6.1** Implement input prompting for `run` command
- [ ] **6.2** Execute steps in dependency order
- [ ] **6.3** Implement `sample <step>` for quick testing
- [ ] **6.4** Store results in session state
- [ ] **6.5** Implement `results [step]` with table display
- [ ] **6.6** Show execution progress with timing
- [ ] **6.7** Handle execution errors gracefully

### User Tests

```
TEST 6.1: Run prompts for inputs
────────────────────────────────
> run
Expected:
  Inputs required:
    target_account [string, required]: _
Verify: Required inputs prompted, optional show defaults

TEST 6.2: Run with default values
─────────────────────────────────
> input lookback = "P7D"  # has default
> run
Expected:
  lookback [timespan, default: P7D]: <enter>
Verify: Pressing enter uses default

TEST 6.3: Run execution progress
────────────────────────────────
> run
  target_account: admin@contoso.com
Expected:
  Running on prod-sentinel...
    ✓ failed_logins     234 rows   (1.2s)
    ✓ source_analysis   5 rows     (0.3s)
  Complete. 2 steps, 1.5s total.
Verify: Progress shows in real-time, timing accurate

TEST 6.4: Sample command
────────────────────────
> sample source_analysis --limit 5
Expected: Prompts for required inputs, then:
  Sampling source_analysis (limit: 5)...
  ┌────────────────┬──────────┐
  │ IpAddress      │ attempts │
  ├────────────────┼──────────┤
  │ 192.168.1.105  │ 23       │
  │ 10.0.0.44      │ 12       │
  └────────────────┴──────────┘
  2 rows (limited to 5)

TEST 6.5: Results command
─────────────────────────
> results source_analysis
Expected: Full results table with pagination if large

> results
Expected: Shows results from all steps or prompts to select

TEST 6.6: Execution error handling
──────────────────────────────────
> run  # with a step that will fail
Expected:
  Running on prod-sentinel...
    ✓ step1           100 rows   (0.5s)
    ✗ step2           Error: Query timeout
    ○ step3           Skipped (depends on step2)
  Completed with errors. 1/3 steps succeeded.
Verify: Dependent steps skipped, clear error shown

TEST 6.7: No workspace selected
───────────────────────────────
> run  # without workspace
Expected:
  No workspace selected.
  Available workspaces:
    1. prod-sentinel
    2. dev-sentinel
  Select [1-2]: _
Verify: Interactive selection, or error with hint
```

### Exit Criteria
- [ ] All 7 user tests pass
- [ ] Progress updates in real-time
- [ ] Tables render with proper alignment
- [ ] Large results paginated or scrollable

---

## Phase 7: Pack Persistence (save, load, new)

**Goal:** Save sessions as pack files and reload them.

### Implementation Tasks

- [ ] **7.1** Implement `save <path.yml>` serialization
- [ ] **7.2** Implement `load <path.yml>` deserialization
- [ ] **7.3** Implement `new [--name]` to start fresh session
- [ ] **7.4** Validate pack file on load
- [ ] **7.5** Handle pack file conflicts (overwrite prompt)
- [ ] **7.6** Support pack metadata (name, description, version)

### User Tests

```
TEST 7.1: Save pack
───────────────────
> save investigation.yml
Expected: ✓ Saved: investigation.yml
         Inputs: 2
         Steps: 3
Verify: File created, valid YAML

TEST 7.2: Save pack contents
────────────────────────────
# Check file contents
Expected YAML structure:
  name: "Brute Force Investigation"
  version: "1.0"
  inputs:
    - name: target_account
      type: string
      required: true
  steps:
    - name: failed_logins
      query: |
        SecurityEvent
        | where EventID == 4625
        ...
Verify: Multi-line queries preserved, proper formatting

TEST 7.3: Load pack
───────────────────
panopticon #0 > load investigation.yml
Expected: ✓ Loaded: Brute Force Investigation
         Inputs: 2
         Steps: 3
panopticon #1 > inputs
  target_account, lookback
panopticon #1 > steps
  1. failed_logins
  2. source_analysis
  3. lateral_check
Verify: Full state restored

TEST 7.4: New session
─────────────────────
panopticon #5 > new
Start new session? Current session has unsaved changes. [y/N]: y
panopticon #0 > inputs
  (none defined)
Verify: Clean slate, warning if unsaved changes

TEST 7.5: New with name
───────────────────────
> new --name "Incident 2024-001"
✓ New session: Incident 2024-001

TEST 7.6: Load invalid pack
───────────────────────────
> load broken.yml
Expected: ✗ Failed to load: broken.yml
         Error: Missing required field 'steps'
         Line 5: Invalid input type 'badtype'
Verify: Clear error, line numbers if possible

TEST 7.7: Overwrite protection
──────────────────────────────
> save existing.yml
File exists. Overwrite? [y/N]: n
Cancelled.
Verify: No data loss without confirmation
```

### Exit Criteria
- [ ] All 7 user tests pass
- [ ] Pack files match documented schema
- [ ] Round-trip (save→load) preserves all data
- [ ] Unsaved changes warning works

---

## Phase 8: Polish & Documentation

**Goal:** Final UX polish, help system, and documentation.

### Implementation Tasks

- [ ] **8.1** Implement comprehensive `help` command
- [ ] **8.2** Add `help <command>` for detailed help
- [ ] **8.3** Add command examples in help
- [ ] **8.4** Implement tab completion for all commands
- [ ] **8.5** Add `--help` flags to all commands
- [ ] **8.6** Create user guide documentation
- [ ] **8.7** Add error recovery suggestions
- [ ] **8.8** Performance optimization pass

### User Tests

```
TEST 8.1: Help command
──────────────────────
> help
Expected: Organized command listing by category:
  Pack Authoring:
    input    Define user input parameter
    query    Define query step
    ...
  Execution:
    run      Execute investigation
    ...
Verify: Logical grouping, brief descriptions

TEST 8.2: Command-specific help
───────────────────────────────
> help query
Expected:
  query <name> [= "<kql>"]

  Define a query step for the investigation.

  Arguments:
    name    Step name (must be unique)
    kql     KQL query (opens editor if omitted)

  Examples:
    query events = "SecurityEvent | take 10"
    query analysis   # opens KQL editor

  See also: input, steps, edit
Verify: Examples included, related commands listed

TEST 8.3: Tab completion
────────────────────────
> que<TAB>
Expected: Completes to 'query'

> query ev<TAB>
Expected: Completes to existing step names if editing

> workspace sel<TAB>
Expected: Completes to 'select'

> workspace select pro<TAB>
Expected: Completes workspace names

TEST 8.4: Error suggestions
───────────────────────────
> qeury step = "..."
Expected: Unknown command: qeury
         Did you mean: query?

> input 123invalid = "x"
Expected: Invalid name: 123invalid
         Names must start with a letter

TEST 8.5: Startup banner
────────────────────────
$ kql-repl
Expected:
  ╦╔═╔═╗ ╦    ╔═╗╔═╗╔╗╔╔═╗╔═╗╔╦╗╦╔═╗╔═╗╔╗╔
  ╠╩╗║═╬╗║    ╠═╝╠═╣║║║║ ║╠═╝ ║ ║║  ║ ║║║║
  ╩ ╩╚═╝╚╩═╝  ╩  ╩ ╩╝╚╝╚═╝╩   ╩ ╩╚═╝╚═╝╝╚╝

  KQL Panopticon REPL v0.1.0
  Type 'help' for commands, 'exit' to quit.

Verify: Clean startup, version shown
```

### Exit Criteria
- [ ] All 5 user tests pass
- [ ] Help covers all commands
- [ ] Tab completion works throughout
- [ ] No obvious UX rough edges

---

## Summary

| Phase | Focus | Key Deliverable | Status |
|-------|-------|-----------------|--------|
| 1 | Core State | `input`, `query`, `steps`, `inputs` commands | ✅ Complete |
| 2 | Multi-line | Line continuation with `\` | ✅ Complete |
| 3 | Exploration | State graph, `revert`, `checkpoint`, `trace` | ✅ Complete |
| 4 | Validation | Syntax/schema validation on define | ✅ Core complete |
| 5 | TUI Editor | KQL editor with highlighting and completion | ✅ Complete (KQL) |
| 6 | Execution | `run`, `sample`, `results` with prompting | 🔲 Not started |
| 7 | Persistence | `save`, `load`, `new` pack management + YAML pack editor | 🔲 Not started |
| 8 | Polish | Help, completion, documentation | 🔲 Not started |

### Phase 5 Sub-phases

| Sub-phase | Focus | Deliverable | Status |
|-----------|-------|-------------|--------|
| 5A | Core Widget | `TuiEditor`, layout, keybindings, terminal handling | ✅ Complete |
| 5B | Highlighting | `Highlighter` trait, `KqlHighlighter` via FFI | ✅ Complete |
| 5C | Completion | Completion popup, KQL completions via FFI, reference completions | ✅ Complete |
| 5D | KQL Mode | Wire highlighting + completion, `query <name>` integration | ✅ Complete |
| 5E | YAML Mode | Pack YAML editor for inputs/steps | 🔄 Deferred to Phase 7 |
| 5F | Edit Command | `edit <name>` for steps | ✅ Complete (steps only) |

**Note (December 2024):** YAML editing deferred to Phase 7. The YAML editor is better suited for editing entire packs rather than individual inputs. The `input <name> = "value"` inline syntax remains the primary method for defining inputs. Pack YAML editing will be implemented alongside `save`/`load` functionality.

## Dependencies

```
Phase 1 ─────┬────► Phase 2 ────► Phase 3
             │
             └────► Phase 4 ────► Phase 5 ────────┐
                         │          │              │
                         │        5A ► 5B ► 5C     │
                         │               │         │
                         │             5D ─┼─► 5E  │
                         │               │         │
                         │             5F ◄────────┤
                         │                         │
                         └─────────────────────────┴────► Phase 6 ────► Phase 7 ────► Phase 8
                                      │
                    (requires workspace/client from current impl)
```

**Phase 5 internal dependencies:**
- 5A (Core Widget) is prerequisite for all other sub-phases
- 5B (Highlighting) and 5C (Completion) can proceed in parallel after 5A
- 5D (KQL Mode) requires 5B + 5C
- 5E (YAML Mode) requires 5B (highlighting only, no KQL completion)
- 5F (Edit Command) requires 5D + 5E

**Cross-phase dependencies:**
- Phases 1-3 complete: Core authoring, line continuation, state graph
- Phase 4 complete: Validation via FFI (`get_classifications`, `get_completions`)
- Phase 5 uses FFI services from Phase 4 for highlighting and completion
- Phase 6 requires Phase 5 for full authoring capability

---

## Future Enhancements

Features identified for future implementation, not currently prioritized.

### Schema Viewer

**Goal:** Interactive exploration of captured workspace schemas.

**Potential Commands:**

```
# List all tables in selected workspace(s)
schema tables [--filter <pattern>]

# Show columns for a specific table
schema show <table_name>

# Search across all tables for a column name
schema search <column_pattern>

# Compare schemas between workspaces
schema diff <workspace1> <workspace2>
```

**Example User Flows:**

```
# Discover available tables
panopticon> schema tables
Tables in prod-sentinel (127 captured):
  SecurityEvent         45 columns   canonical
  SigninLogs            38 columns   canonical
  AuditLogs             22 columns   canonical
  CustomAlerts_CL       12 columns   custom
  ...

# Explore table columns
panopticon> schema show SecurityEvent
SecurityEvent (45 columns):
  TimeGenerated      datetime     Event timestamp
  EventID            int          Windows event ID
  Account            string       Account name
  AccountType        string       User or Machine
  Computer           string       Source computer
  IpAddress          string       Source IP
  ...

# Find tables with a specific column
panopticon> schema search IpAddress
Column 'IpAddress' found in 8 tables:
  SecurityEvent.IpAddress      string
  SigninLogs.IPAddress         string
  AzureActivity.CallerIpAddress string
  ...
```

**Dependencies:** Requires schema capture (ADR-001) to be complete and working.
