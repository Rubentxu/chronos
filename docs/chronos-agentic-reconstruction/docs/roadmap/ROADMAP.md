# Reconstruction roadmap

## Roadmap rules

- Milestones are ordered by dependency/value, not calendar promises.
- A milestone closes only when its UAT gate passes.
- Architecture may evolve through spikes and ADR supersession.
- Prefer vertical slices over broad half-implemented infrastructure.
- No new GUI or language breadth may mask a failing core evidence contract.

# M0 — Truth First: stabilize the current system

## Goal

Remove known false-confidence paths before building the new architecture.

## Work

### Event correctness

- fix EventBus length/high-water/eviction accounting;
- stop destructive global reads from being used as pagination;
- define one timestamp meaning per field;
- reject unknown event type filters;
- remove plausible defaults for missing query targets.

### Session lifecycle

- make probe ownership session-scoped;
- retain eBPF adapter/link lifetime until explicit detach;
- stop producers before sealing final tail, or mark session incomplete;
- make snapshots cumulative or explicitly projection-relative.

### Tripwires

- connect evaluation to live event flow;
- persist labels;
- increment fire counts correctly;
- return stable typed IDs.

### State/query correctness

- preserve register/state evidence required by state-diff paths;
- classify current race detection as heuristic.

# M1 — ExecutionLog vertical slice

## Goal

One authoritative append-only evidence stream.

## Work

- `EventSeq`;
- cursor API;
- multi-consumer reads;
- explicit gaps;
- segment persistence;
- session metadata;
- migrate one producer and one query path end-to-end.

Do not migrate every adapter in parallel. Prove one vertical first.

# M2 — Replayable projections and invocation identity

## Goal

Make dynamic execution structure queryable without treating a symbol as a concrete call.

## Work

- `SymbolId` / `InvocationId`;
- start/end/incomplete invocation model;
- parent/links;
- incremental InvocationProjection;
- CallGraphProjection;
- versioned projection checkpoint;
- replay equivalence test.

# M3 — Runtime properties, Mutation Lens and causal slice

## Goal

Move from event browsing to bug evidence.

## Work

- declarative property model;
- selected state transition records;
- PropertyProjection;
- violation evidence bundle;
- conservative backward causal slice;
- historical property evaluation with explicit unsupported result.

UAT: negative order total caused by known discount computation.

# M4 — Adaptive Instrumentation v1: Go + Rust

## M4A — Go reference

- existing OTel discovery;
- OBI/Go zero-code integration;
- Auto SDK correlation validation;
- `otelc` compile-time debug build;
- Chronos `InstrumentationSpec` -> deterministic Go instrumentation;
- exact binary vs instrumented binary comparison.

UAT: Go checkout bug located with coarse evidence, deepened with compile-time probes and verified after patch.

## M4B — Rust reference

- existing `tracing`/OTel discovery;
- OBI baseline;
- targeted native eBPF;
- XRay nightly spike (`-Z instrument-xray`) with measured constraints;
- USDT spike;
- temporary semantic-probe overlay for one typed mutation;
- exact vs debug-build perturbation comparison.

UAT: Rust state-corruption bug plus one timing-sensitive bug that demonstrates fallback from deeper probes to lighter evidence when perturbation is detected.

# M5 — Agent API v2 and application-service extraction

## Goal

Make MCP composable and thin.

## Work

- SessionService;
- ProbeService / ProbePlanner;
- PropertyService;
- QueryService;
- DiffService;
- typed MCP v2 tools;
- compatibility/deprecation layer for legacy tools;
- capabilities endpoint.

Exit: no new application algorithm belongs directly in `chronos-mcp::server`.

# M6 — OpenTelemetry correlation and export

## Goal

Use existing distributed observability as evidence context and avoid rebuilding generic trace UI.

## Work

- local OTLP ingestion adapter;
- external trace/span context projection;
- provenance mapping;
- optional OTLP export of compatible high-level evidence;
- OBI integration hardening/version pinning.

UAT: distributed request crosses two sandbox services and local mutation evidence links to the correct distributed trace/span.

# M7 — Differential execution v2

## Goal

Find the first **meaningful** divergence, not merely first byte/hash mismatch.

## Work

- hierarchical segment hashes for fast equality;
- align by external trace context/invocation topology where possible;
- loop/async tolerant matching;
- state/property divergence;
- BehaviourFingerprint prototype.

UAT: known-good/failing runs differ in noise/timing but Chronos locates the injected semantic divergence.

# M8 — Counterexample shrinking and test intelligence

## Goal

Turn failing property executions into small reproducible bug evidence.

## Work

- proptest/Hypothesis integration contracts;
- input/counterexample artifact;
- rerun loop;
- causal slice minimization;
- `chronos test` experiment for Go/Rust.

# M9 — Concurrency intelligence

## Goal

Replace time-window race heuristics with causal synchronization evidence.

## Work

- LockAcquire/Release;
- AtomicRead/Write where attainable;
- Task/Goroutine spawn/join;
- Message/Channel send/receive;
- happens-before projection;
- suspicious vs confirmed classification.

# M10 — Execution Explorer

## Goal

Visualize Chronos-specific evidence, not build another IDE.

Views: Live, Execution, Causality, Mutation Lens, Properties/Hypotheses, Compare and Evidence provenance.

# M11 — Additional language depth

Prioritize by demonstrated demand and reliable mechanisms:

1. Python `sys.monitoring`;
2. JVM JFR + OTel agent;
3. Node/JavaScript zero-code/runtime;
4. browser/WASM convergence;
5. C/C++ XRay/rr;
6. other languages.

Each language needs capability matrix and real UAT before claiming support depth.

# Deferred / optional

- graph database;
- remote multi-node persistent control plane;
- permanent semantic-probe promotion workflow;
- production continuous debugging;
- automatic patch application;
- arbitrary property DSL extensions.
