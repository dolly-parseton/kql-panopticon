# ADR-002: Exploring Interpreter Pattern for REPL

*Date: 2024-12-12*
*Status: Proposed*
*Research: [REPL Interpreter Design](../research/repl-interpreter-design.md)*

## Context

The KQL-Panopticon REPL was initially conceived as a command-line interface for query execution. However, security investigations are inherently exploratory—analysts pursue hypotheses, hit dead ends, backtrack, and branch into parallel lines of inquiry.

Current log analytics interfaces (Azure Log Analytics, Microsoft Sentinel Advanced Hunting) treat queries as isolated operations with no session state or exploration support. This forces investigators to:
- Mentally track investigation branches
- Manually manage query history
- Lose work when pursuing unproductive hypotheses
- Lack reproducibility for incident reports

Research into principled REPL design [van Binsbergen et al., 2020] reveals that treating a REPL as a proper **interpreter** with formal semantics enables exploration capabilities essential for investigation workflows.

## Decision

Adopt the **exploring interpreter** pattern for KQL-Panopticon's REPL, treating the REPL as a stateful interpreter with:

1. **Numbered state transitions** - Each command creates a new state ID
2. **Execution graph** - Track ancestry of states for navigation
3. **Backtracking support** - Return to any previous state
4. **Named checkpoints** - Save significant investigation points
5. **Session export/import** - Reproducible investigation scripts

## Implementation

### Core Components

```rust
pub struct StateGraph {
    snapshots: HashMap<StateId, Snapshot>,
    ancestry: HashMap<StateId, StateId>,
    checkpoints: HashMap<String, StateId>,
    current: StateId,
}

pub struct Snapshot {
    session: SessionState,
    timestamp: DateTime<Utc>,
    command: Option<String>,
}
```

### New Commands

| Command | Description |
|---------|-------------|
| `revert [n]` | Return to state #n (default: previous) |
| `checkpoint save <name>` | Save current state with name |
| `checkpoint restore <name>` | Return to named checkpoint |
| `trace [--tree]` | Show execution history |
| `fork [note]` | Create explicit branch point |
| `export <path>` | Export session as script |

### Prompt Enhancement

```
panopticon #<state_id> [checkpoint] (workspace) >
```

## Consequences

### Positive

- **Investigation-native UX** - Matches how analysts actually work
- **Non-destructive exploration** - Risk-free hypothesis testing
- **Reproducibility** - Sessions become documentation
- **Audit trail** - Complete investigation history
- **Differentiation** - Capability not available in existing tools

### Negative

- **Memory overhead** - Storing state snapshots
- **Complexity** - More sophisticated state management
- **Learning curve** - New mental model for users

### Mitigations

- Implement snapshot pruning for long sessions
- Provide simple defaults (undo = `revert`, basic history)
- Clear documentation and examples

## Alternatives Considered

### 1. Linear History Only

Simple command history without state snapshots.

**Rejected:** Cannot support backtracking or branching—the core value proposition.

### 2. Git-like Branches

Explicit branch/merge model.

**Rejected:** Overly complex for investigation workflows. The exploring interpreter's implicit branching is more natural.

### 3. Jupyter Notebook Integration

Use Jupyter as the primary interface.

**Deferred:** Potential future enhancement, but CLI-first approach serves more use cases. Research shows notebooks can be built atop exploring interpreters (Bacatá pattern).

## References

- [Research: REPL Interpreter Design](../research/repl-interpreter-design.md)
- van Binsbergen et al. (2020). A principled approach to REPL interpreters. https://doi.org/10.1145/3426428.3426917
