# REPL Pack Authoring Design

*Created: December 2024*
*Status: Implementation Planning*

## Goal

Transform the KQL-Panopticon REPL from a simple command interface into an **interactive pack authoring environment** where investigation packs emerge naturally from exploratory sessions. The REPL becomes an interpreter with formal state semantics, enabling backtracking, validation-as-you-go, and session reproducibility.

## Design Principles

1. **Session-as-Pack** - The REPL session state *is* the pack being built
2. **Validate Early** - Syntax and schema validation on definition, not execution
3. **Explore Freely** - Backtracking and checkpoints enable risk-free exploration
4. **Clear Semantics** - Distinct commands for distinct concepts (`input` vs `query`)
5. **Progressive Disclosure** - Simple inline syntax for quick work, editors for complex cases

## Command Reference

### Definition Commands

| Command | Description | Editor |
|---------|-------------|--------|
| `input <name> [= "<value>"]` | Define user input parameter | YAML (type, description, default) |
| `query <name> [= "<kql>"]` | Define query step | KQL (syntax highlighting, completion) |
| `edit <name>` | Edit existing input or step | Appropriate editor |
| `remove <name>` | Remove input or step | - |

### Inspection Commands

| Command | Description |
|---------|-------------|
| `inputs` | List all defined inputs |
| `steps` | List all defined query steps |
| `info` | Show pack metadata and summary |

### Validation & Testing

| Command | Description |
|---------|-------------|
| `validate [step]` | Validate step(s) syntax and schema |
| `sample <step> [--limit N]` | Execute step with sample data |

### Execution

| Command | Description |
|---------|-------------|
| `run [--step <name>]` | Execute investigation (prompts for inputs) |
| `results [step]` | Display results from last run |

### Pack Management

| Command | Description |
|---------|-------------|
| `new [--name "<name>"]` | Start new pack session |
| `save <path.yml>` | Save session as pack file |
| `load <path.yml>` | Load pack into session |

### Exploration (Exploring Interpreter)

| Command | Description |
|---------|-------------|
| `revert [N]` | Return to state #N (default: previous) |
| `checkpoint save <name>` | Save named checkpoint |
| `checkpoint restore <name>` | Restore checkpoint |
| `trace` | Show execution history tree |

### Environment

| Command | Description |
|---------|-------------|
| `workspace list` | List available workspaces |
| `workspace select <name>` | Select workspace for execution |
| `clear` | Clear screen |
| `exit` | Exit REPL |

## State Model

```rust
pub struct ReplState {
    // Exploring interpreter
    pub state_graph: StateGraph,

    // Pack session
    pub session: PackSession,

    // Environment
    pub workspace: Option<Workspace>,
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
    pub input_values: HashMap<String, Value>,
    pub results: HashMap<String, QueryResult>,
}

pub struct StateGraph {
    pub snapshots: HashMap<StateId, Snapshot>,
    pub ancestry: HashMap<StateId, StateId>,
    pub checkpoints: HashMap<String, StateId>,
    pub current: StateId,
}
```

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

## Example Session

```bash
panopticon #0 > new --name "Brute Force Investigation"
✓ New pack session: Brute Force Investigation

panopticon #1 > workspace select prod-sentinel
✓ Selected: prod-sentinel

panopticon #2 > input target_account
# Opens YAML editor
✓ Input defined: target_account (string, required)

panopticon #3 > input lookback = "P7D"
✓ Input defined: lookback (timespan, default: P7D)

panopticon #4 > query failed_logins = "SecurityEvent \
  | where EventID == 4625 \
  | where TargetAccount == '{{inputs.target_account}}' \
  | where TimeGenerated > ago({{inputs.lookback}})"
✓ Step defined: failed_logins
  refs: [inputs.target_account, inputs.lookback]
  schema: TimeGenerated, TargetAccount, IpAddress, ...

panopticon #5 > query source_analysis
# Opens KQL editor with completion for:
#   {{inputs.*}}, {{failed_logins.*}}
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

panopticon #7 > save brute-force.yml
✓ Saved: brute-force.yml (2 inputs, 2 steps)
```

## References

- [Research: REPL Interpreter Design](../research/repl-interpreter-design.md)
- [ADR-002: Exploring Interpreter REPL](../decisions/002-exploring-interpreter-repl.md)
- van Binsbergen et al. (2020). A principled approach to REPL interpreters. https://doi.org/10.1145/3426428.3426917
