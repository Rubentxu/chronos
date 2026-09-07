# Milestone acceptance gates

A milestone is not complete until every mandatory gate passes in a reproducible environment.

## M0

### UAT-M0-01 — non-destructive pagination
Given 10,000 records and `limit=100`, records 101..10,000 remain available to the same and another consumer.

### UAT-M0-02 — cumulative session evidence
Two snapshot/index refresh operations preserve evidence from both periods.

### UAT-M0-03 — tripwire reality
A real sandbox event fires one tripwire; label persists, fire count increments, evidence points to a real event, delete prevents later fires.

### UAT-M0-04 — eBPF lifecycle
After probe creation returns, the probe remains attached until explicit stop/detach.

### UAT-M0-05 — explicit invalid filter
Unknown event type returns validation error and never broadens the query.

### UAT-M0-06 — state diff evidence
A fixture with two register/state snapshots produces the known difference.

## M1

- two independent cursors receive the same ordered 1000 records;
- resume from cursor N starts at N+1 according to documented delivery semantics;
- forced bounded overflow emits `Gap`;
- interrupted segment write preserves committed segments.

## M2

- recursive function creates distinct `InvocationId`s;
- process killed mid-function yields `Incomplete`, not synthetic return;
- full replay equals checkpoint+delta projection.

## M3

- known `Order.total` transition `59 -> -35` violates `>=0`;
- causal slice contains known producer and excludes unrelated work;
- property requiring uncaptured historical field returns `UnsupportedByRecordedEvidence`.

## M4A Go

- existing manual OTel reused without duplicate Chronos operation span;
- OBI/zero-code sees expected supported coarse operation without product source change;
- compile-time debug build adds selected deeper evidence while product git tree stays clean;
- coarse run -> hypothesis -> deep run -> proven root cause -> patched run passes property;
- perturbation comparison is reported.

## M4B Rust

- existing Rust telemetry is reused/correlated before custom probes;
- targeted native event observed without source modification;
- XRay spike either proves viable function capture or is explicitly rejected with measured reasons and superseding ADR;
- temporary debug instrumentation captures one typed before/after mutation without product-branch modification;
- timing fixture changing under deep instrumentation is marked as perturbation and triggers fallback.

## M5

An agent-style test solves a fixture using only API v2 tools. Legacy calls either map to v2 or return a documented deprecation path.

## M6

A two-service request links:

```text
external trace/span -> Chronos invocation -> state mutation -> property violation
```

without equating span to invocation.

## M7

Comparison ignores harmless timing/order noise and finds the known semantic divergence.

## M8

A generated failing input shrinks while preserving the same property violation.

## M9

A lock-protected same-address scenario is not a confirmed race; an unsynchronized fixture is supported by happens-before evidence.

## M10

UI renders a large trace through aggregation/virtualization without materializing every event at once.
