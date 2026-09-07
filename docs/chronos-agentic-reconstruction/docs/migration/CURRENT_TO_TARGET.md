# Migration: current implementation to target architecture

## Baseline

Prepared against `main` commit:

`c76b1096dfc25de666a32bbb56992d4190e5aee3`

The latest commit already moves browser capture toward an agent-first approach. The reconstruction continues that convergence across the workspace.

## Migration principle

Do not perform a flag-day rewrite.

Use strangler migration:

```text
legacy EventBus/Query paths
        |
        +--> compatibility adapter --> ExecutionLog
                                   --> new projection
                                   --> API v2
```

Move one complete flow at a time.

## Known high-priority corrections

### Shared event buffers

Current domain EventBus uses separate raw/semantic queues and destructive reads.

Action:

- introduce ExecutionLog beside it;
- bridge current producers into the log;
- prohibit new features from depending on destructive drains;
- retire old bus only after consumers migrate.

### CapturePipeline vs EventBus

Two transport approaches coexist.

Action:

- define ExecutionLog append as application boundary;
- adapters may internally use channels/rings;
- internal transport must not leak as session semantics.

### Tripwires

- preserve compatibility initially;
- evaluate against canonical append path;
- persist label/fire state correctly;
- evolve toward typed subscription/condition;
- remove duplicate fired buffers after migration.

### MCP God object

Extract in order:

1. SessionService;
2. ProbeService;
3. QueryService;
4. PropertyService;
5. DiffService.

Handlers become mapping only. Do not mix extraction with unrelated style refactors.

### Probe lifecycle

eBPF links/backend handles are session-owned. A tool-local adapter is not an accepted lifetime model.

### Snapshot/index refresh

Replace "drain batch, replace QueryEngine" with projection cursors/checkpoints.

### Timestamp meaning

Separate monotonic session time from wall clock.

### Event type parsing

Generate/derive typed schema from domain enum. Reject invalid names.

### State diff/register evidence

Do not discard records needed by state projection as generic noise.

### Race terminology

Rename current heuristic output before adding synchronization evidence.

## Crate evolution proposal

Target decomposition; do not create all crates immediately:

```text
chronos-domain
  stable IDs, execution/evidence/property types, ports

chronos-log (new or initially inside store)
  append/cursor/segment contracts

chronos-store
  redb segment persistence, checkpoints, artifacts

chronos-projection (may evolve from index/query)
  invocation/state/causality/property projections

chronos-query
  query facade over projections

chronos-investigation (when responsibility is stable)
  ProbePlanner, hypothesis/evidence orchestration

chronos-capture
  adapter registry/lifecycle glue, not debugger super-trait

chronos-ebpf
  debugging-specific eBPF + OBI integration boundary

chronos-go
  Go capability adapter: OTel/OBI/compile-time/semantic

chronos-native / future rust capability module
  native/Rust capabilities

chronos-mcp
  thin driving adapter

chronos-sandbox
  UAT executable corpus
```

## Compatibility strategy

Legacy MCP tools remain during early milestones if not harmful. Route internally to new services as available, attach deprecation metadata, and remove only after API v2 UAT proves replacement.

## README migration

Replace root README with the reconstruction-aware README in this package. It distinguishes target direction from current implementation so unfinished features are not falsely claimed as complete.

## Migration complete when

- no global destructive evidence consumption;
- MCP contains no core algorithms;
- adapters publish through ExecutionLog;
- graphs/indexes rebuild from replay;
- at least Go and Rust adaptive UAT pass;
- historical proposal docs are clearly non-authoritative.
