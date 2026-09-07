# Implementation backlog — issue-ready decomposition

Use this as the starting issue set. Keep PRs narrow and close them only with the linked UAT evidence.

## Epic M0 — Truth First

### M0-01 Fix live pagination semantics

**Scope:** MCP live probe drain/read path + event buffer contract.

**Deliver:** requesting 100 records never destroys unseen records.

**DoD:** UAT-M0-01.

### M0-02 Make snapshots cumulative

**Scope:** `session_snapshot`, query-engine refresh/build path.

**Deliver:** second refresh adds evidence instead of replacing first batch.

**DoD:** UAT-M0-02.

### M0-03 Session-own eBPF probe lifecycle

**Scope:** `probe_inject`, `EbpfAdapter`, session registry.

**Deliver:** returned probe remains attached until detach/stop; lifecycle observable.

**DoD:** UAT-M0-04.

### M0-04 Wire tripwires to live evidence

**Deliver:** canonical event causes evaluation; labels/fire count persist; stable ID.

**DoD:** UAT-M0-03.

### M0-05 Typed event filters

**Deliver:** eliminate incomplete manual string mapping; invalid value fails schema/validation.

**DoD:** UAT-M0-05.

### M0-06 Fix state-diff evidence path

**Deliver:** register/state observations required by the algorithm survive preprocessing.

**DoD:** UAT-M0-06.

### M0-07 Explicit query absence semantics

**Deliver:** missing event/session/thread never falls back to plausible defaults.

### M0-08 Timestamp contract

**Deliver:** tests and type/docs separate session monotonic time from wall clock.

### M0-09 Race terminology

**Deliver:** current heuristic exposed as suspicious concurrent access until M9.

### M0-10 Sandbox M0 suite

Create/organize executable gates for all M0 fixes.

---

## Epic M1 — ExecutionLog

### M1-01 Domain IDs

Add `EventSeq`, `ConsumerId` and strong session/probe/subscription IDs without string parsing.

### M1-02 In-memory append/cursor reference implementation

Prove API semantics before durable complexity.

### M1-03 Explicit Gap record

Add loss reason/source and tests.

### M1-04 redb segment store spike/implementation

Measure and record decision; do not change DB technology without evidence.

### M1-05 Producer bridge

Bridge one current producer into ExecutionLog while preserving legacy compatibility.

### M1-06 Query bridge

Make one query consume a replayable projection from the log.

### M1-07 Cursor restart/resume test

Persist and resume projection/consumer cursor.

---

## Epic M2 — Invocation/projections

### M2-01 `SymbolId` and `InvocationId`

### M2-02 InvocationProjection

Pair start/end, preserve incomplete calls, parent/links.

### M2-03 CallGraphProjection v1

Build aggregate graph from concrete invocations instead of ad-hoc MCP stack simulation.

### M2-04 Projection checkpoint format/version

### M2-05 Replay equivalence suite

---

## Epic M3 — Properties/evidence

### M3-01 Property domain model

### M3-02 Minimal deterministic property evaluator

Implement only operators required by first UAT.

### M3-03 StateTransition evidence

### M3-04 Mutation Lens query

### M3-05 PropertyViolation evidence bundle

### M3-06 Conservative causal slice v1

### M3-07 RUST-STATE-001 fixture + verification patch

---

## Epic M4A — Go adaptive instrumentation

### M4G-01 Existing OTel capability discovery

Detect/reuse existing trace context without duplicating operations.

### M4G-02 OBI integration spike

Pin version and document privileges/capabilities.

### M4G-03 Auto SDK correlation spike

### M4G-04 `otelc` debug-build spike

Build in isolated directory/worktree; product source remains clean.

### M4G-05 InstrumentationSpec validator for Go

Resolve symbols/types deterministically.

### M4G-06 Chronos semantic state hook proof

Capture one selected state transition.

### M4G-07 GO-DISCOUNT-001 end-to-end agent investigation

---

## Epic M4B — Rust adaptive instrumentation

### M4R-01 Existing `tracing`/OTel discovery/correlation

### M4R-02 OBI Rust baseline

### M4R-03 Targeted eBPF native probe

### M4R-04 XRay nightly spike

Explicit go/no-go ADR after experiment.

### M4R-05 USDT spike

### M4R-06 Temporary typed semantic probe overlay

Compiler-validated, isolated, reproducible manifest.

### M4R-07 Perturbation comparison

### M4R-08 RUST-TIMING-003 and RUST-STATE-001 end-to-end

---

## PR discipline

Each PR should state:

- roadmap task ID;
- ADR/spec affected;
- tests added;
- UAT evidence if closing a gate;
- new gaps/unsupported behaviour;
- rollback/compatibility impact.

Do not combine a contract migration with unrelated cleanup unless the cleanup is required to make the contract testable.
