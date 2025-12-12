# kql-panopticon Architecture

This document describes the architectural design and refactoring plan for kql-panopticon.

## Overview

kql-panopticon is being restructured from a monolithic application into a modular workspace with a reusable core library. This enables:

- **Library reuse** - Core functionality can be used by other tools
- **Better separation of concerns** - UI, CLI, and business logic are decoupled
- **Easier testing** - Core logic can be tested without UI dependencies
- **Future extensibility** - Web APIs, SDKs, and integrations become possible

## Workspace Structure

```
kql-panopticon/
├── Cargo.toml                    # Workspace root
├── crates/
│   └── kql-panopticon-core/      # Core library (new)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs            # Library entry point
│           ├── client.rs         # Azure Log Analytics client
│           ├── workspace.rs      # Workspace model
│           ├── error.rs          # Unified error types
│           ├── execution/        # Execution engines
│           │   ├── mod.rs
│           │   ├── engine.rs     # ExecutionEngine trait
│           │   ├── query.rs      # QueryExecutor
│           │   ├── investigation.rs # InvestigationExecutor
│           │   ├── progress.rs   # Progress reporting
│           │   └── trace.rs      # Execution tracing
│           ├── variable/         # Variable substitution
│           │   ├── mod.rs
│           │   ├── parser.rs     # VarRef parsing
│           │   └── substitution.rs
│           ├── result/           # Result storage
│           │   ├── mod.rs
│           │   └── store.rs
│           ├── pack.rs           # Unified pack definitions
│           ├── schema/           # Schema registry
│           │   ├── mod.rs
│           │   ├── types.rs      # Schema types
│           │   └── registry.rs   # SchemaRegistry
│           └── validation/       # KQL validation (optional)
│               └── mod.rs
│
└── src/                          # Main application (existing)
    ├── main.rs                   # CLI entry point
    ├── cli/                      # CLI commands
    ├── tui/                      # Terminal UI
    └── ...                       # (to be migrated to core)
```

## Architecture Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                     User Interfaces                             │
│  ┌──────────┐  ┌──────────┐  ┌─────────────────────────────────┐│
│  │   CLI    │  │   TUI    │  │      Future: Web API / SDK      ││
│  └────┬─────┘  └────┬─────┘  └─────────────────────────────────┘│
│       │             │                                           │
│       └──────┬──────┘                                           │
│              ▼                                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │              Execution Facade                             │  │
│  │  - Unified job tracking                                   │  │
│  │  - Progress channel management                            │  │
│  │  - Result aggregation                                     │  │
│  └─────────────────────────┬─────────────────────────────────┘  │
└────────────────────────────┼────────────────────────────────────┘
                             │
┌────────────────────────────┼────────────────────────────────────┐
│         kql-panopticon-core (library crate)                     │
│                            │                                    │
│  ┌─────────────────────────┴─────────────────────────────────┐  │
│  │                  ExecutionEngine trait                    │  │
│  │  async fn execute(config, progress_tx) → Result           │  │
│  └─────────────────────────┬─────────────────────────────────┘  │
│            ┌───────────────┴───────────────┐                    │
│            ▼                               ▼                    │
│  ┌──────────────────┐           ┌────────────────────┐          │
│  │  QueryExecutor   │           │InvestigationExecutor│         │
│  │  (parallel)      │           │  (ordered/chained) │          │
│  └────────┬─────────┘           └─────────┬──────────┘          │
│           │                               │                     │
│  ┌────────┴─────────────────────────────┴──────────────────┐   │
│  │                    Shared Services                       │   │
│  │  ┌────────────┐ ┌─────────────┐ ┌─────────────────────┐  │   │
│  │  │   Client   │ │  VarSubst   │ │   ResultStore       │  │   │
│  │  │  (Azure)   │ │  (unified)  │ │   (unified format)  │  │   │
│  │  └────────────┘ └─────────────┘ └─────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              Optional: KQL Validator                     │   │
│  │  (via .NET AOT FFI - requires kql-validation feature)    │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Core Library Modules

### `client` - Azure Log Analytics Client

Handles Azure authentication and API communication:

- DefaultAzureCredential authentication
- Token caching with automatic refresh
- Workspace discovery across subscriptions
- Query execution with pagination

### `execution` - Execution Engines

#### `ExecutionEngine` Trait

The core abstraction for all execution types:

```rust
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    type Config: Send + Sync;
    type Result: Send + Sync + Debug;

    async fn execute(
        &self,
        config: Self::Config,
        workspaces: Vec<Workspace>,
        progress: Option<ProgressSender>,
    ) -> Result<Self::Result>;

    fn validate(&self, config: &Self::Config) -> Result<()>;
    fn executor_name(&self) -> &'static str;
}
```

#### `QueryExecutor`

Parallel query execution:
- Runs queries concurrently across workspaces
- Handles pagination automatically
- Streams results to files
- Reports progress per query/workspace

#### `InvestigationExecutor`

Ordered, chained execution:
- Topological sort by dependencies
- Variable extraction and substitution
- Conditional execution (`when` clauses)
- Foreach iteration with batching
- HTTP enrichment steps
- Report generation

### `variable` - Variable Substitution

Unified variable handling:

| Syntax | Description |
|--------|-------------|
| `{{inputs.name}}` | User-provided input |
| `{{step.*.column}}` | All values from column (array) |
| `{{step.first.column}}` | First row value |
| `{{step[N].column}}` | Nth row value |
| `{{alias.column}}` | Current foreach row |
| `{{secrets.name}}` | Environment variable |

### `result` - Result Storage

Pluggable result storage:

```rust
#[async_trait]
pub trait ResultStore: Send + Sync {
    async fn store_results(...) -> Result<PathBuf>;
    async fn store_manifest(...) -> Result<PathBuf>;
    async fn load_manifest(...) -> Result<ResultManifest>;
    async fn list_executions(&self) -> Result<Vec<String>>;
}
```

Default implementation: `FileResultStore`

### `pack` - Pack Definitions

Unified pack structure for all query execution:

- Single `Pack` type (no separate QueryPack/InvestigationPack)
- Dependency-driven execution model
- Simple packs (no dependencies) execute in parallel
- Complex packs (with dependencies) execute in topological order
- Step types: `kql`, `http`, `file`

### `schema` - Schema Registry

Persistent cache of table schemas for validation and completion:

- **Storage**: `~/.kql-panopticon/schemas.json`
- **Schema types**:
  - `Canonical` - Fixed schema, shared across workspaces (SecurityEvent, SigninLogs)
  - `Extensible` - Base schema + per-workspace extensions (AzureDiagnostics, Syslog)
  - `Custom` - Entirely workspace-specific (*_CL tables)
- **Workspace tracking**: Which tables exist in which workspaces
- **Staleness**: 30-day threshold with refresh prompts
- **Capture**: Via schema discovery pack (see ADR-001)

```rust
pub struct SchemaRegistry {
    tables: HashMap<String, TableInfo>,      // Canonical schemas
    workspaces: HashMap<String, WorkspaceSchema>, // Per-workspace info
}
```

### `validation` - KQL Validation (Optional)

When the `kql-validation` feature is enabled:
- Syntax validation
- Schema-aware semantic validation (uses SchemaRegistry)
- Uses .NET AOT compiled Kusto.Language library

## Execution Models

### Query Pack Execution

```
         ┌──────────────────────────────────────────┐
         │              QueryExecutor               │
         └──────────────────┬───────────────────────┘
                            │
         ┌──────────────────┼──────────────────┐
         ▼                  ▼                  ▼
    ┌─────────┐        ┌─────────┐        ┌─────────┐
    │Query 1  │        │Query 2  │        │Query 3  │
    │WS A,B,C │        │WS A,B,C │        │WS A,B,C │
    └────┬────┘        └────┬────┘        └────┬────┘
         │                  │                  │
         ▼                  ▼                  ▼
    [Parallel execution across all queries and workspaces]
         │                  │                  │
         └──────────────────┼──────────────────┘
                            ▼
                    ┌──────────────┐
                    │   Results    │
                    └──────────────┘
```

### Investigation Pack Execution

```
         ┌──────────────────────────────────────────┐
         │         InvestigationExecutor            │
         └──────────────────┬───────────────────────┘
                            │
                  ┌─────────┴─────────┐
                  ▼                   ▼
             Workspace A         Workspace B
                  │                   │
           ┌──────┴──────┐     ┌──────┴──────┐
           ▼             ▼     ▼             ▼
       Step 1 ───────► Step 2   Step 1 ───────► Step 2
           │      vars     │       │      vars     │
           │               │       │               │
           ▼               ▼       ▼               ▼
       Step 3 ◄─────── depends   Step 3 ◄─────── depends
           │                       │
           ▼                       ▼
      [Results]               [Results]
```

## Progress Reporting

Unified progress system for all execution types:

```rust
pub enum ProgressUpdate {
    // Lifecycle
    Started { job_id, job_type, total_steps, total_workspaces },
    Completed { job_id, duration_ms },
    Failed { job_id, error },

    // Step-level
    StepStarted { job_id, step_name, workspace },
    StepCompleted { job_id, step_name, workspace, rows, duration_ms },
    StepFailed { job_id, step_name, workspace, error },
    StepSkipped { job_id, step_name, workspace, reason },

    // Investigation-specific
    VariablesExtracted { job_id, step_name, variables },
    ConditionEvaluated { job_id, step_name, condition, result },
    ForeachProgress { job_id, step_name, current, total },
    HttpExecuted { job_id, step_name, url, status },
}
```

## Migration Plan

### Phase 1: Core Library Foundation (Current)

- [x] Create workspace structure
- [x] Stub out kql-panopticon-core modules
- [x] Define ExecutionEngine trait
- [x] Define unified progress reporting
- [x] Define result storage abstraction
- [ ] Document architecture

### Phase 2: Move Existing Code

- [ ] Move `client.rs` to core library
- [ ] Move `workspace.rs` to core library
- [ ] Move `query_job.rs` → `execution/query.rs`
- [ ] Move `investigation/` → core library
- [ ] Update imports in main crate

### Phase 3: Implement Facade

- [ ] Create execution facade in main crate
- [ ] Unify job tracking for TUI
- [ ] Complete TUI investigation execution
- [ ] Update CLI commands to use facade

### Phase 4: KQL Validation

- [ ] Create .NET AOT wrapper for Kusto.Language
- [ ] Implement FFI bindings in Rust
- [ ] Add kql-validation feature flag
- [ ] Integrate with editor/pack validation

### Phase 5: TUI Improvements

- [ ] Unified Jobs view for queries and investigations
- [ ] Investigation progress display
- [ ] Result browsing in TUI
- [ ] Execution trace viewer

## KQL Validation Architecture

The optional KQL validation feature uses .NET AOT compilation:

```
┌────────────────────────────────────────────────────────────┐
│                    .NET AOT Library                        │
│  ┌──────────────────────────────────────────────────────┐  │
│  │           Microsoft.Azure.Kusto.Language             │  │
│  │  - Full KQL parser                                   │  │
│  │  - Semantic analysis                                 │  │
│  │  - Schema-aware validation                           │  │
│  └──────────────────────────────────────────────────────┘  │
│                            │                               │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              C# AOT Wrapper                          │  │
│  │  [UnmanagedCallersOnly]                              │  │
│  │  - validate_syntax(query) → errors                   │  │
│  │  - validate_with_schema(query, schema) → errors      │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────────────┬───────────────────────────────┘
                             │ Native library (.dylib/.so/.dll)
                             ▼
┌────────────────────────────────────────────────────────────┐
│                    Rust FFI Layer                          │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              validation/mod.rs                       │  │
│  │  extern "C" { fn validate_syntax(...) }              │  │
│  │  pub struct KqlValidator                             │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
```

### Building the .NET AOT Library

```bash
# In the dotnet-kql-validator project:
dotnet publish -c Release -r osx-arm64 --self-contained

# Produces: libkql_validator.dylib
```

### Rust Integration

```rust
// Feature-gated
#[cfg(feature = "kql-validation")]
mod ffi {
    #[link(name = "kql_validator")]
    extern "C" {
        fn kql_validator_init() -> i32;
        fn kql_validate_syntax(
            query: *const u8,
            query_len: i32,
            errors: *mut u8,
            errors_len: i32,
        ) -> i32;
    }
}
```

## Error Handling

Unified error type hierarchy:

```rust
pub enum Error {
    Azure { message, source },
    Query { message, workspace, query },
    Investigation { message, step, workspace },
    Variable { message, variable },
    Pack { message, path },
    Http { message, url, status },
    Storage { message, path },
    Config { message },
    Io { message },
    Validation { message, line, column },
    Timeout { message },
    Other(String),
}
```

## Testing Strategy

### Unit Tests

- Variable parsing and substitution
- Pack validation
- Error types
- Result formatting

### Integration Tests

- Query execution (requires Azure credentials)
- Investigation execution
- Result storage

### Mock Testing

- Mock Azure client for offline testing
- Mock HTTP responses for enrichment steps

## Configuration

### Environment Variables

| Variable | Description |
|----------|-------------|
| `AZURE_TENANT_ID` | Azure AD tenant |
| `AZURE_CLIENT_ID` | Service principal ID |
| `AZURE_CLIENT_SECRET` | Service principal secret |
| `KQL_PANOPTICON_OUTPUT` | Default output directory |
| `KQL_PANOPTICON_LOG` | Log level |

### Pack Secrets

Secrets in investigation packs reference environment variables:

```yaml
secrets:
  api_key: ${MY_API_KEY}
  auth_token: ${AUTH_TOKEN}
```

## Future Considerations

### Web API

The core library could power a REST API:

```
POST /api/execute/query
POST /api/execute/investigation
GET  /api/jobs/{id}
GET  /api/jobs/{id}/progress (WebSocket)
GET  /api/results/{id}
```

### SDK

Publish `kql-panopticon-core` to crates.io for use in other tools.

### Distributed Execution

The ExecutionEngine trait could support distributed backends:
- Kubernetes jobs
- Azure Functions
- Distributed query coordinators

## Decision Records

Architectural decisions are documented in `docs/decisions/`:

- [ADR-001: Schema Capture Strategy](decisions/001-schema-capture.md) - How we capture and cache table schemas
- [ADR-002: Exploring Interpreter REPL](decisions/002-exploring-interpreter-repl.md) - Principled REPL design for investigation workflows

## Design Documents

Active design specifications:

- [REPL Pack Authoring](design/repl-pack-authoring.md) - Interactive pack building in the REPL
- [REPL Implementation Plan](design/repl-implementation-plan.md) - Phased implementation with user tests

## Research

Background research informing architectural decisions:

- [REPL Interpreter Design](research/repl-interpreter-design.md) - Formal REPL semantics and exploring interpreter patterns
