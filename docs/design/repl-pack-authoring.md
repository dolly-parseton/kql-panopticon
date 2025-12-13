# REPL Pack Authoring Design

*Created: December 2024*
*Status: Implementation In Progress*

## Goal

Transform the KQL-Panopticon REPL from a simple command interface into an **interactive pack authoring environment** where investigation packs emerge naturally from exploratory sessions. The REPL becomes an interpreter with formal state semantics, enabling backtracking, validation-as-you-go, and session reproducibility.

## Design Principles

1. **Session-as-Pack** - The REPL session state *is* the pack being built
2. **Validate Early** - Syntax and schema validation on definition, not execution
3. **Explore Freely** - Backtracking and checkpoints enable risk-free exploration
4. **Clear Semantics** - Distinct commands for distinct concepts (`input` vs `query`)
5. **Progressive Disclosure** - Simple inline syntax for quick work, editors for complex cases

---

## Command Reference

### Definition Commands

| Command | Description | Behavior |
|---------|-------------|----------|
| `input <name> [= "<value>"]` | Define or edit user input parameter | Opens editor if exists, creates new otherwise |
| `query <name> [= "<kql>"]` | Define or edit query step | Opens KQL editor if exists, creates new otherwise |
| `remove <name>` | Remove input or step | Removes from session |

### Inspection Commands

| Command | Description |
|---------|-------------|
| `inputs` | List all defined inputs |
| `steps [--show <name>]` | List all defined query steps |
| `info` | Show pack metadata and summary |
| `peek <step>` | View results from a step |

### Validation & Testing

| Command | Description |
|---------|-------------|
| `validate [step] [--query "..."]` | Validate step(s) syntax and schema |
| `sample <step> [--limit N]` | Execute step with sample data |

### Execution

| Command | Description |
|---------|-------------|
| `run [path] [--query "..."] [--all]` | Execute investigation (prompts for inputs) |
| `results [job_id]` | Display results from execution |
| `jobs` | List running/completed jobs |
| `history` | View execution history |

### Pack Management

| Command | Description |
|---------|-------------|
| `new [--name "<name>"]` | Start new pack session |
| `pack load <path>` | Load pack into session |
| `pack save <path>` | Save session as pack file |

### Exploration (Exploring Interpreter)

| Command | Description |
|---------|-------------|
| `revert [N]` | Return to state #N (default: previous) |
| `checkpoint save <name>` | Save named checkpoint |
| `checkpoint restore <name>` | Restore checkpoint |
| `checkpoint list` | List saved checkpoints |
| `checkpoint delete <name>` | Delete checkpoint |
| `trace [--tree]` | Show execution history |

### Environment

| Command | Description |
|---------|-------------|
| `workspace list` | List available workspaces |
| `workspace select [<name>]` | Select workspace for execution |
| `workspace schema [--capture]` | View/capture workspace schema |
| `config [key] [value]` | View/set configuration |
| `status` | Show current session status |
| `clear` | Clear screen |
| `help [command]` | Show help |
| `exit` | Exit REPL |

---

## State Model

```rust
pub struct ReplContext {
    // Exploring interpreter
    pub state_graph: StateGraph,

    // Environment
    pub workspaces: Vec<Workspace>,
    pub selected: HashSet<String>,
    pub client: Option<Client>,

    // Validation
    pub validator: Option<KqlValidator>,
    pub schema_cache: SchemaCache,
}

pub struct PackSession {
    pub name: Option<String>,
    pub description: Option<String>,
    pub inputs: IndexMap<String, InputDef>,
    pub steps: IndexMap<String, StepDef>,
}

pub struct StateGraph {
    pub snapshots: HashMap<StateId, Snapshot>,
    pub ancestry: HashMap<StateId, StateId>,
    pub checkpoints: HashMap<String, StateId>,
    pub current: StateId,
}
```

### State Semantics

Every state-modifying command creates a new state in the graph:
- `input`, `query`, `remove` create new states
- `revert` changes current state pointer
- `checkpoint save/restore` creates named references

This enables:
- **Backtracking**: Return to any previous state
- **Branching**: Explore alternatives from any point
- **Checkpoints**: Mark and return to significant states

---

## Prompt Design

```
panopticon #<state_id> [checkpoint] (workspace) [status] >
```

Examples:
```
panopticon #0 >                                    # Initial
panopticon #3 (prod-sentinel) >                    # Workspace selected
panopticon #7 [validated] (prod-sentinel) >        # At checkpoint
panopticon #12 (prod-sentinel) [2 steps] >         # With steps defined
```

---

## Implementation Status

### Complete ✅

| Feature | Description |
|---------|-------------|
| Core State Model | `PackSession`, `InputDef`, `StepDef` with serialization |
| Definition Commands | `input`, `query`, `inputs`, `steps`, `remove`, `info` |
| Variable References | `{{inputs.x}}`, `{{step.x}}` parsing and dependency tracking |
| Multi-line Input | Line continuation with `\`, continuation prompt |
| Exploring Interpreter | `StateGraph`, `revert`, `checkpoint`, `trace` |
| Validation | KQL syntax validation via FFI, schema-aware validation |
| TUI Editor | KQL editor with syntax highlighting and completion |
| TUI Shell | Full ratatui TUI with inline widgets |

### In Progress 🔄

| Feature | Description | Notes |
|---------|-------------|-------|
| Execution | `run`, `sample` with input prompting | Basic flow works, needs polish |
| Results Display | `peek`, `results` with table view | Widget exists, integration needed |

### Not Started 🔲

| Feature | Description |
|---------|-------------|
| Pack Persistence | `pack save`, `pack load` |
| New Session | `new` command with unsaved warning |
| Pack Metadata | Name, description, version in pack files |

---

## Next Steps: Pack Management

### Phase 6: Execution Polish

1. **Input prompting flow** - Collect required inputs before execution
2. **Progress display** - Real-time step status in TUI
3. **Results storage** - Store results in session state
4. **Error handling** - Graceful failure with dependent step skipping

### Phase 7: Pack Persistence

1. **Save command** - Serialize session to YAML pack file
2. **Load command** - Deserialize pack file to session
3. **New command** - Reset session with unsaved changes warning
4. **Pack metadata** - Name, description, version fields

### Pack File Format

```yaml
name: "Brute Force Investigation"
version: "1.0"
description: "Investigate failed login patterns"

inputs:
  - name: target_account
    type: string
    description: "Account to investigate"
    required: true
  - name: lookback
    type: timespan
    default: "P7D"

steps:
  - name: failed_logins
    query: |
      SecurityEvent
      | where EventID == 4625
      | where TargetAccount == '{{inputs.target_account}}'
      | where TimeGenerated > ago({{inputs.lookback}})
  - name: source_analysis
    query: |
      {{failed_logins}}
      | summarize attempts=count() by IpAddress
      | top 10 by attempts
```

---

## Example Session

```bash
panopticon #0 > new --name "Brute Force Investigation"
✓ New pack session: Brute Force Investigation

panopticon #1 > workspace select prod-sentinel
✓ Selected: prod-sentinel

panopticon #2 > input target_account
# Opens input definition form
✓ Input defined: target_account (string, required)

panopticon #3 > input lookback = "P7D"
✓ Input defined: lookback (timespan, default: P7D)

panopticon #4 > query failed_logins = "SecurityEvent \
  | where EventID == 4625 \
  | where TargetAccount == '{{inputs.target_account}}' \
  | where TimeGenerated > ago({{inputs.lookback}})"
✓ Step defined: failed_logins
  refs: [inputs.target_account, inputs.lookback]

panopticon #5 > query source_analysis
# Opens KQL editor with completion for {{inputs.*}}, {{failed_logins.*}}
✓ Step defined: source_analysis
  depends: [failed_logins]

panopticon #6 > sample source_analysis
Inputs required:
  target_account: admin@contoso.com
  lookback [P7D]: <enter>

Sampling...
┌────────────────┬──────────┐
│ IpAddress      │ attempts │
├────────────────┼──────────┤
│ 192.168.1.105  │ 47       │
└────────────────┴──────────┘

panopticon #7 > pack save brute-force.yml
✓ Saved: brute-force.yml (2 inputs, 2 steps)
```

---

## References

- [TUI Shell Design](tui-shell-design.md)
- [Research: REPL Interpreter Design](../research/repl-interpreter-design.md)
- [ADR-002: Exploring Interpreter REPL](../decisions/002-exploring-interpreter-repl.md)
- van Binsbergen et al. (2020). A principled approach to REPL interpreters. https://doi.org/10.1145/3426428.3426917
