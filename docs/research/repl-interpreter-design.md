# Research: Principled REPL Interpreter Design for Security Investigations

*Research Date: December 2024*
*Status: Active Research*

## Abstract

This document captures research into principled REPL (Read-Eval-Print-Loop) interpreter design and its application to security investigation workflows. Current log analytics interfaces (Azure Log Analytics, Microsoft Sentinel, Advanced Hunting) provide query execution but lack the exploratory capabilities essential for iterative threat hunting and incident investigation. By treating the REPL as a proper interpreter with formal semantics, we can enable investigation workflows that support backtracking, state exploration, and reproducible analysis sessions.

## Motivation

### The Gap in Current Tooling

Existing KQL execution environments treat each query as an isolated operation:

| Tool | Limitation |
|------|------------|
| Azure Log Analytics | No session state; each query starts fresh |
| Microsoft Sentinel | Workbooks provide some continuity but no backtracking |
| Advanced Hunting (Defender) | Query history but no state composition |
| Kusto Explorer | Better session support but limited investigation flow |

**What investigators need but don't have:**
- Backtrack to a previous point when a query path proves unproductive
- Branch investigations to explore multiple hypotheses
- Compose query results into subsequent queries seamlessly
- Reproduce an entire investigation session
- Checkpoint progress during long-running investigations

### REPL as Interpreter, Not Just Interface

The insight from academic research is that a REPL is not merely a command-line interface—it is an **interpreter** with its own semantics. When designed properly, a REPL provides:

1. **Compositional semantics** - Commands compose predictably
2. **State management** - Session state evolves through transitions
3. **Exploration support** - Navigate through execution history
4. **Reproducibility** - Sessions can be replayed

## Theoretical Foundation

### Source: "A Principled Approach to REPL Interpreters"

The primary theoretical foundation comes from van Binsbergen et al.'s work on formalizing REPL semantics:

> "Read-eval-print-loops (REPLs) allow programmers to test out snippets of code, explore APIs, or even incrementally construct code, and get immediate feedback on their actions. However, even though many languages provide a REPL, the relation between the language as is and what is accepted at the REPL prompt is not always well-defined."

**Citation:**
```
L. Thomas van Binsbergen, Mauricio Verano Merino, Pierre Jeanjean,
Tijs van der Storm, Benoit Combemale, and Olivier Barais. 2020.
A principled approach to REPL interpreters.
In Proceedings of the 2020 ACM SIGPLAN International Symposium on
New Ideas, New Paradigms, and Reflections on Programming and Software
(Onward! 2020). Association for Computing Machinery, New York, NY, USA, 84–100.
https://doi.org/10.1145/3426428.3426917
```

**Artifact:**
```
L. Thomas van Binsbergen, Mauricio Verano Merino, Pierre Jeanjean,
Tijs van der Storm, Benoit Combemale, and Olivier Barais. 2020.
A principled approach to REPL interpreters (Artifact).
https://dl.acm.org/do/10.1145/3410257/full/
```

### Key Concepts

#### 1. Sequential Languages

A language L = ⟨P, Γ, γ₀, I⟩ is **sequential** if there exists a composition operator `;` such that:

```
For all p₁, p₂ ∈ P and γ ∈ Γ:
  p₁;p₂ ∈ P  and  I(p₁;p₂)(γ) = (I(p₂) ∘ I(p₁))(γ)
```

Where:
- **P** = Set of valid programs/commands
- **Γ** = Set of configurations (states)
- **γ₀** = Initial configuration
- **I** = Interpreter function: Program × State → State

**Implication for KQL REPL:** REPL commands must compose predictably. Executing `workspace select X` followed by `pack load Y` must yield the same result as a hypothetical combined command.

#### 2. Configuration

A configuration captures the complete state needed to evaluate subsequent commands:

```
Configuration = (Environment, SessionState, History)

Where:
  Environment  = Stable context (credentials, connections)
  SessionState = Mutable state (selections, loaded artifacts)
  History      = Accumulated outputs and transitions
```

#### 3. Exploring Interpreter

An extension to the basic REPL that maintains an **execution graph**:

> "An exploring interpreter for a language ⟨P, Γ, γ₀, I⟩ is an algorithm maintaining a current configuration (initially γ₀) and an execution graph (initially containing just the node γ₀) and iteratively executing one of the following actions."

The exploring interpreter enables:
- **Forward execution**: Normal command evaluation
- **Backward navigation**: Return to previous states
- **Branching**: Fork from any historical state
- **Trace inspection**: View the path through the execution graph

## Application to Security Investigations

### Investigation as State Exploration

A security investigation naturally follows an exploratory pattern:

```
Initial State (Alert triggered)
    │
    ├─► Query: Find related events
    │       │
    │       ├─► Hypothesis A: Malware execution
    │       │       │
    │       │       └─► Dead end, revert
    │       │
    │       └─► Hypothesis B: Lateral movement
    │               │
    │               ├─► Checkpoint: "confirmed-lateral"
    │               │
    │               └─► Continue investigation...
    │
    └─► Branch: Check for persistence mechanisms
```

Current tools force investigators to mentally track this tree. A principled REPL externalizes it.

### Mapping REPL Concepts to KQL Investigation

| REPL Concept | Investigation Application |
|--------------|--------------------------|
| Configuration | Current investigation state (workspace, timeframe, variables) |
| Transition | Query execution that refines understanding |
| Backtracking | Abandon unproductive hypothesis, return to earlier state |
| Branching | Explore parallel hypotheses |
| Checkpoint | Save progress at significant findings |
| Trace | Audit trail of investigation steps |

### Example Investigation Session

```
panopticon #0 > workspace select prod-sentinel
Selected: prod-sentinel
panopticon #1 (prod-sentinel) >

panopticon #1 > let alert_time = 2024-01-15T14:30:00Z
Variable bound: alert_time

panopticon #2 > run --query "SecurityEvent | where TimeGenerated between (alert_time .. alert_time+1h)"
Executing... 847 rows returned
panopticon #3 >

panopticon #3 > checkpoint save initial-triage
Checkpoint saved: initial-triage (state #3)

panopticon #3 > run --query "{{prev}} | where EventID == 4688"
# Investigating process creation
Executing... 23 rows returned
panopticon #4 >

panopticon #4 > run --query "{{prev}} | where NewProcessName contains 'powershell'"
Executing... 0 rows returned
# Dead end - no PowerShell

panopticon #5 > revert 3
Reverted to state #3 (initial-triage)

panopticon #3 > run --query "{{prev}} | where EventID == 4624"
# Different hypothesis: authentication events
Executing... 156 rows returned
panopticon #6 >

panopticon #6 > trace
Investigation trace:
  #0 (initial)
  #1 workspace select prod-sentinel
  #2 let alert_time = 2024-01-15T14:30:00Z
  #3 run --query "SecurityEvent | where TimeGenerated..."  [checkpoint: initial-triage]
  │
  ├─#4 run --query "{{prev}} | where EventID == 4688"
  │  └─#5 run --query "{{prev}} | where NewProcessName contains 'powershell'" [dead end]
  │
  └─#6 run --query "{{prev}} | where EventID == 4624"  ← current
```

## Implementation Architecture

### Core Data Structures

Based on the artifact's implementation patterns:

```rust
/// Exploring interpreter state graph
pub struct StateGraph {
    /// State ID → Snapshot mapping
    snapshots: HashMap<StateId, Snapshot>,

    /// Ancestry: child → parent
    ancestry: HashMap<StateId, StateId>,

    /// Named checkpoints
    checkpoints: HashMap<String, StateId>,

    /// Current state ID
    current: StateId,

    /// Next state ID to assign
    next_id: StateId,
}

/// Immutable snapshot of session state
pub struct Snapshot {
    /// Session state at this point
    pub session: SessionState,

    /// When this state was created
    pub timestamp: DateTime<Utc>,

    /// Command that created this state
    pub command: Option<String>,

    /// Optional annotation
    pub note: Option<String>,
}

/// Session state that evolves through commands
pub struct SessionState {
    /// Selected workspaces
    pub workspaces: Vec<Workspace>,

    /// Loaded pack
    pub pack: Option<LoadedPack>,

    /// Bound variables (from `let` commands)
    pub variables: HashMap<String, Value>,

    /// Last query result (for `{{prev}}` reference)
    pub last_result: Option<QueryResult>,

    /// Investigation timeframe
    pub timeframe: Option<TimeRange>,
}
```

### Command Semantics

Following the paper's categorization of how state fields compose:

| Field | Composition | Behavior |
|-------|-------------|----------|
| `workspaces` | **Mutable** | New selection replaces old |
| `pack` | **Mutable** | Loading replaces previous |
| `variables` | **Mutable** | New bindings add/override |
| `last_result` | **Replaceable** | Each query replaces |
| `history` | **Appendable** | Always grows |
| `violations` | **Appendable** | Errors accumulate |

### REPL Commands

```rust
pub enum Command {
    // === State-modifying commands ===
    Workspace { action: WorkspaceAction },
    Pack { action: PackAction },
    Run { query: Option<String>, ... },
    Let { name: String, value: Value },

    // === Exploration commands ===
    /// Revert to previous state
    Revert { state_id: Option<i32> },  // None = previous

    /// Save named checkpoint
    Checkpoint { action: CheckpointAction },

    /// Show execution trace
    Trace { count: usize, tree: bool },

    /// Fork current state (explicit branch point)
    Fork { note: Option<String> },

    // === Session commands ===
    /// Export session as reproducible script
    Export { path: PathBuf },

    /// Import and replay session
    Import { path: PathBuf },
}
```

### Prompt Design

The prompt communicates exploration state:

```
panopticon #<state_id> [checkpoint_name] (workspace) [status] >
```

Examples:
```
panopticon #0 >                           # Initial state
panopticon #3 (prod) >                    # State 3, workspace selected
panopticon #7 [lateral-movement] (prod) > # At named checkpoint
panopticon #12 (prod) [3 jobs] >          # With running jobs
```

## Benefits for Security Operations

### 1. Reduced Cognitive Load

Investigators no longer need to mentally track exploration branches. The REPL externalizes the investigation tree.

### 2. Reproducible Investigations

Sessions can be exported and replayed:
- Share investigation methodology with team
- Document steps for incident reports
- Create templates for common investigation patterns

### 3. Non-Destructive Exploration

Backtracking enables risk-free hypothesis testing. Investigators can pursue aggressive queries knowing they can always return to a known-good state.

### 4. Audit Trail

The execution graph provides a complete audit trail of investigation steps—valuable for compliance and post-incident review.

### 5. Training and Knowledge Transfer

Recorded sessions become training materials. Junior analysts can replay expert investigations to learn techniques.

## Future Research Directions

### 1. Collaborative Investigation

Multiple analysts sharing an exploration graph, with branching and merging of investigation threads.

### 2. Automated Hypothesis Generation

AI-assisted suggestion of next exploration steps based on current state and historical patterns.

### 3. Investigation Notebooks

Jupyter-style notebooks backed by the exploring interpreter, combining narrative documentation with executable investigation steps.

### 4. Temporal Branching

First-class support for "what if this happened at a different time" exploration.

## References

1. van Binsbergen, L.T., Verano Merino, M., Jeanjean, P., van der Storm, T., Combemale, B., & Barais, O. (2020). A principled approach to REPL interpreters. *Onward! 2020*, 84-100. https://doi.org/10.1145/3426428.3426917

2. van Binsbergen, L.T., et al. (2020). A principled approach to REPL interpreters (Artifact). https://dl.acm.org/do/10.1145/3410257/full/

3. Verano Merino, M., & van der Storm, T. (2020). Bacatá: Notebooks for DSLs, Almost for Free. *The Art, Science, and Engineering of Programming*, 4(3). https://doi.org/10.22152/programming-journal.org/2020/4/11

4. van Binsbergen, L.T., et al. (2020). eFLINT: a domain-specific language for executable norm specifications. *GPCE 2020*. https://doi.org/10.1145/3425898.3426958

## Appendix: Related Tools

### Time-Travel Debugging

The exploring interpreter concept relates to time-travel debugging systems:
- **rr** (Mozilla) - Record and replay debugging
- **Undo LiveRecorder** - Commercial time-travel debugger
- **Chidori** - Reactive runtime with execution graph visualization

### Notebook Systems

- **Jupyter** - Interactive computing notebooks
- **Bacatá** - Generic Jupyter kernel generator for DSLs
- **Observable** - Reactive notebooks with dataflow

---

*This research informs the design of KQL-Panopticon's REPL interpreter to provide investigation-oriented exploration capabilities not available in current log analytics tooling.*
