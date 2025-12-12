# ADR-001: Schema Capture Strategy

**Date**: 2024-01-XX
**Status**: Accepted
**Context**: Schema registry implementation for KQL validation and completion

## Problem

We need to capture table schemas from Azure Log Analytics workspaces to enable:
- KQL query validation (syntax and semantic)
- Future autocompletion/intellisense
- Workspace-aware query authoring (knowing which tables exist where)

The challenge is balancing query costs, latency, and completeness of schema data.

## Options Considered

### Option A: Reactive (On-Demand)

Capture schema only when validation needs it.

```
User writes query → Validator sees unknown table → Run "T | getschema" → Cache result
```

**Pros:**
- Minimal queries - only fetch what's needed
- No upfront cost

**Cons:**
- Latency during validation (user waits for query)
- Repeated queries if same table validated multiple times before cache
- Poor offline experience
- No discovery of available tables

### Option B: Proactive Scan (Startup)

Run `search * | distinct $table` on startup for new workspaces.

**Pros:**
- Know what tables exist upfront
- Single query per workspace

**Cons:**
- Only gets table names, not column schemas
- Still need follow-up `T | getschema` for each table
- Startup delay for new workspaces

### Option C: Heavy Pack (Once-and-Done)

A comprehensive schema discovery pack that captures everything once per workspace.

```yaml
# Conceptual structure
steps:
  - name: discover_tables
    query: search * | where TimeGenerated > ago(30d) | distinct $table

  - name: capture_schemas
    foreach: discover_tables as t
    query: "{{t.$table}} | getschema"

  - name: sample_dynamic_fields  # Future enhancement
    foreach: discover_tables as t
    query: "{{t.$table}} | take 100 | mv-apply ..."
```

**Pros:**
- One-time cost, zero runtime cost
- Richer metadata than reactive (can capture dynamic field structures, value distributions)
- Predictable experience - no surprises during query authoring
- Works offline after initial capture
- Dogfooding - tool uses its own pack system
- Fits MSSP workflow: onboard workspace → run schema pack → ready to investigate

**Cons:**
- Upfront query cost (could be expensive for workspaces with many tables)
- Stale data if new tables are added (requires manual refresh)
- Initial setup step required

## Decision

**Option C: Heavy Pack (Once-and-Done)**

The pack-based approach provides the best experience for the MSSP use case:
1. Clear operational model: onboard workspace → capture schema → work
2. No latency during query authoring
3. Predictable query costs (known upfront, not spread throughout usage)
4. Enables richer metadata capture (dynamic field sampling, value distributions)
5. Self-documenting (the pack itself shows how schema capture works)

## Implementation

### Storage Location

```
~/.kql-panopticon/
├── schemas.json           # Schema registry (all workspaces)
└── schema-discovery.yaml  # Optional: custom schema discovery pack
```

### Pack Location

- **Default**: Baked into the tool as an embedded resource
- **Override**: If `~/.kql-panopticon/schema-discovery.yaml` exists, use that instead
- This allows users to customize schema capture (different timespan, additional sampling, etc.)

### Schema Types

Tables are classified by schema stability:

| Type | Description | Storage |
|------|-------------|---------|
| **Canonical** | Fixed schema (SecurityEvent, SigninLogs) | Shared in registry |
| **Extensible** | Base schema + workspace extensions (AzureDiagnostics, Syslog) | Base shared, extensions per-workspace |
| **Custom** | Entirely workspace-specific (*_CL tables) | Per-workspace only |

### Dynamic Field Sampling (Future)

For extensible tables and dynamic columns, the pack can optionally sample nested structures:

- Depth limit: X levels (configurable, default 3)
- Key limit: Y keys per level (configurable, default 50)
- This addresses a major KQL pain point: discovering nested field paths

### Staleness

- Schemas are considered stale after 30 days
- Tool can prompt for refresh, but doesn't auto-refresh (predictable query costs)
- Manual refresh via command: `kql-panopticon schema refresh <workspace>`

### Fallback Behavior

For unknown tables encountered during validation:
- Warn: "Table 'Foo' not in schema cache for workspace 'bar'"
- Don't automatically fetch (maintains predictable query model)
- User can manually run schema capture or add table to custom override

## Consequences

### Positive

- Zero latency during query authoring after initial capture
- Predictable query costs (all upfront during onboarding)
- Rich metadata enables future features (completion, field suggestions)
- Users can customize capture behavior
- Works offline after initial setup

### Negative

- Requires explicit onboarding step for new workspaces
- Schema can become stale (mitigated by staleness warnings)
- Initial capture may be slow for workspaces with many tables

### Risks

- Query costs for large workspaces could be significant
  - Mitigation: Document expected query volume, allow pack customization
- Schema drift over time
  - Mitigation: Staleness tracking, easy refresh command

## Related Decisions

- ADR-XXX: Pack output format (affects how schema capture results are consumed)
- ADR-XXX: Advanced Hunting support (separate schema world, different capture approach)

## Notes

The schema discovery pack is a form of dogfooding - the tool uses its own pack execution system to bootstrap itself. This validates the pack system design and ensures it's capable enough for real-world workflows.
