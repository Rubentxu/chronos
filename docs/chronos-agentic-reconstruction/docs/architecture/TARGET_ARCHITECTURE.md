# Target architecture

## Style

Chronos uses **hexagonal architecture** with event-sourced execution evidence and replayable projections.

```text
Driving adapters
MCP / CLI / UI
        |
        v
Application services
Session / Probe / Property / Query / Diff / Investigation
        |
        v
Domain ports + types
        ^
        |
Driven adapters
Execution store / OTel / eBPF / runtime agents / ptrace / rr
```

Infrastructure depends inward. The domain does not depend on webhook/network/runtime-specific infrastructure.

## Core components

### ExecutionLog

Authoritative append-only stream. It assigns/validates `EventSeq`, preserves known ordering, exposes cursors and explicit gaps, persists segments and supports replay. It is **not** a consumer queue.

### Projection engine

Versioned views:

- InvocationProjection
- CallGraphProjection
- StateProjection
- CausalityProjection
- PropertyProjection
- PerformanceProjection
- OTelCorrelationProjection

A projection is disposable and rebuildable.

### Investigation engine

Coordinates evidence -> hypothesis -> missing evidence -> instrumentation plan -> rerun -> comparison -> confirmation/rejection -> patch verification.

### Probe planner

Chooses the cheapest mechanism that can obtain the required evidence within the observation/perturbation budget.

### Evidence engine

Every bundle preserves provenance, completeness, supporting sequence/range and derivation version.

## Ports

Prefer small capability ports over a broad debugger trait:

```rust
trait ProbeSource { ... }
trait ProbeController { ... }
trait ExecutionLog { ... }
trait ProjectionStore { ... }
trait RuntimeMetadataProvider { ... }
trait StackInspector { ... }
trait ValueObserver { ... }
trait StateObserver { ... }
trait ReplayBackend { ... }
trait TelemetryReceiver { ... }
```

Backends advertise capabilities instead of implementing unsupported methods.

## Strong IDs

```text
SessionId EventSeq SymbolId InvocationId ProbeId SubscriptionId
ConsumerId PropertyId AnalysisId ProjectionId CheckpointId
ExternalTraceId ExternalSpanId
```

## Invocation model

```text
SymbolId      = static function identity
InvocationId  = one concrete runtime call
```

An invocation can have parent, links, thread/task/goroutine, start/end evidence, incomplete status, source mapping, args/returns when observed.

## Execution graph

The graph is a projection, not storage truth.

Nodes: Invocation, Thread, Task/Goroutine, State, Resource, Message, Exception, PropertyViolation.

Edges may include:

```text
CALLS RETURNS_TO SPAWNS AWAITS READS WRITES MUTATES
THROWS CATCHES SENDS RECEIVES ACQUIRES RELEASES CAUSES VIOLATES
```

Create an edge only when Chronos can explain its evidence/derivation.

## OpenTelemetry relationship

OTel spans and Chronos invocations are correlated, not equated. A high-level span may contain many Chronos invocations.

Store external context (`trace_id`, `span_id`, `parent_span_id`, links) separately.

## Architecture constraints

- no global destructive read;
- no silent fallback to a default thread/session/event;
- no string parsing as domain identity;
- no infrastructure dependency from domain;
- no MCP handler containing core algorithms;
- no language adapter forced to implement unsupported capabilities.
