# Reconstruction roadmap

## Roadmap rules

- Milestones are ordered by dependency/value, not calendar promises.
- A milestone closes only when its UAT gate passes.
- Architecture may evolve through spikes and ADR supersession.
- Prefer vertical slices over broad half-implemented infrastructure.
- No new GUI or language breadth may mask a failing core evidence contract.
- **A cycle close is not proof of requirement compliance.** Current compliance lives in `/reconstruction-contracts.toml`.
- **REC-C0..REC-C7 are mandatory convergence gates. Official M6+ is blocked until REC-C7 closes.**

## Current status

**ACTIVE: REC-C0 — Restore the truth baseline.**

The source review performed on 2026-09-15 found substantial delivered reconstruction work together with material residual gaps: parallel EventBus/ExecutionLog paths, non-authoritative cursor/completeness semantics, unresolved dependency-direction violations, wide optional adapter interfaces, documentation drift, unfinished M4 adaptive instrumentation and trust-sensitive UAT gaps.

See:

- `../reconstruction/CONVERGENCE_PLAN.md`
- `../adr/0011-reconstruction-convergence-truth-gate.md`
- `../testing/ARCHITECTURE_FITNESS_FUNCTIONS.md`
- `../testing/SPEC_COMPLIANCE.md`
- `/reconstruction-contracts.toml`

# M0 — Truth First: stabilize the current system

## Goal

Remove known false-confidence paths before building the new architecture.

## Delivery status

Historical implementation work is closed, but residual contract obligations are owned by REC-C0..REC-C2. In particular, destructive legacy paths and truthful completeness/cursor semantics must be revalidated rather than assumed from the historical close.

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
- increment/derive fire counts correctly;
- return stable typed IDs.

### State/query correctness

- preserve register/state evidence required by state-diff paths;
- classify current race detection as heuristic.

# M1 — ExecutionLog vertical slice

## Goal

One authoritative append-only evidence stream.

## Delivery status

The append-only log, EventSeq, durable multi-consumer cursors, explicit gaps, segment persistence and compaction are substantially implemented. **Authoritative cutover is not complete** while EventBus-only/dual-truth and QueryEngine-primary paths remain. REC-C1 and REC-C2 own the cutover/deletion.

## Work

- `EventSeq`;
- cursor API;
- multi-consumer reads;
- explicit gaps;
- segment persistence;
- session metadata;
- migrate producers and query paths end-to-end until the old truth path disappears.

# M2 — Replayable projections and invocation identity

## Goal

Make dynamic execution structure queryable without treating a symbol as a concrete call.

## Delivery status

`SymbolId`, `InvocationId`, parent identity and call-graph/analytics foundations are delivered. REC-C6 revalidates real incomplete-invocation behavior and projection replay/checkpoint equivalence.

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

## Delivery status

Property model, projection, violation evidence, Mutation Lens and conservative slicing foundations are delivered. REC-C6 closes remaining integration around authoritative evidence, subscriptions and unsupported historical evidence.

## Work

- declarative property model;
- selected state transition records;
- PropertyProjection;
- violation evidence bundle;
- conservative backward causal slice;
- historical property evaluation with explicit unsupported result.

UAT: negative order total caused by known discount computation.

# M4 — Adaptive Instrumentation v1: Go + Rust

## Delivery status

**Not complete. REC-C6 must close the missing implementation before M6 starts.** Existing observation/property feed work is a useful foundation but does not satisfy the complete Go/Rust instrumentation contract.

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

## Delivery status

Application-service extraction is materially delivered. Product-level convergence to a small canonical v2 surface and deletion of legacy shims remains and is owned by REC-C4/REC-C5.

## Work

- SessionService;
- ProbeService / ProbePlanner;
- PropertyService;
- QueryService;
- DiffService;
- typed MCP v2 tools;
- compatibility/deprecation layer for legacy tools;
- capabilities endpoint.

Exit target remains: no application algorithm belongs directly in `chronos-mcp::server`, and the public agent surface converges to approximately 8–12 orthogonal tools.

---

# Mandatory reconstruction convergence gate

The detailed plan is `../reconstruction/CONVERGENCE_PLAN.md`.

## REC-C0 — Restore the truth baseline — **ACTIVE**

- restore CI/coverage;
- machine-readable specification ledger;
- architecture/spec CI fitness gates;
- all-feature compilation;
- documentation/capability truth reconciliation.

## REC-C1 — ExecutionLog cutover and truthful reads

- ExecutionLog mandatory for agentic sessions;
- `events_read` uses real `EventSeq` cursors;
- real gap/completeness/provenance;
- QueryEngine becomes projection only;
- explicit tail sealing/incomplete state.

## REC-C2 — Legacy evidence deletion

- remove agent/query EventBus paths;
- remove destructive global evidence reads;
- move TripwireFired into the log/projection model;
- remove fired queue and dual truth;
- delete compatibility code once replacement UAT is green.

## REC-C3 — Hexagonal boundary closure

- ports for probes, stores, logs, notifications and telemetry;
- remove infrastructure from domain;
- remove application -> concrete adapter edges;
- remove store -> native edge;
- composition only at driving adapter/bootstrap boundary.

## REC-C4 — SOLID and connascence reduction

Status: DONE (cycle rec-c4-solid-connascence, tag `3a572db4`, 2026-09-20).

- split broad TraceAdapter by capabilities; ✅ C4.1 CaptureLifecycle + DebugInspect blanket split (SOLID-001 gap→partial)
- capability-first agent planning;
- typed identities; ✅ C4.3 SubscriptionId typed MCP→services (CONN-002 partial; probe_id pending)
- separate sequence/monotonic/wall-clock semantics; ✅ C4.2 MonotonicNs + WallClockMs newtypes (CONN-001 partial; chronos-log boundary pending)
- eliminate drain/merge/rebuild ordering coupling. (not in C4 scope; deferred)
- new pre-existing debt: DEBT-C4-01..04 (see vault maintenance/debt-ledger.md)

## REC-C5 — Agent API convergence

- canonical v2-only UAT path;
- legacy aliases isolated with explicit sunset;
- shrink/delete compatibility surface;
- MCP wrappers remain thin.

## REC-C6 — Close unfinished reconstruction foundations

- finish M4A Go;
- finish M4B Rust;
- close M1/M2/M3 residual contracts;
- unify observe/tripwire/property subscription concepts where semantics overlap.

## REC-C7 — Convergence close

- strict no-gap ledger check;
- all-feature CI and real sandbox UAT green;
- legacy/dependency waiver inventory empty for convergence-owned debt;
- no P0/P1 false-confidence path;
- README/API claims <= verified capability depth.

**Only REC-C7 may unblock official M6.**

---

# M6 — OpenTelemetry correlation and export — BLOCKED by REC-C7

## Goal

Use existing distributed observability as evidence context and avoid rebuilding generic trace UI.

## Work

- local OTLP ingestion adapter;
- external trace/span context projection;
- provenance mapping;
- optional OTLP export of compatible high-level evidence;
- OBI integration hardening/version pinning.

UAT: distributed request crosses two sandbox services and local mutation evidence links to the correct distributed trace/span, without equating span identity to invocation identity.

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

- preserve existing shrinking foundation;
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

Each language needs a capability matrix and real UAT before claiming support depth.

# Deferred / optional

- graph database;
- remote multi-node persistent control plane;
- permanent semantic-probe promotion workflow;
- production continuous debugging;
- automatic patch application;
- arbitrary property DSL extensions.
