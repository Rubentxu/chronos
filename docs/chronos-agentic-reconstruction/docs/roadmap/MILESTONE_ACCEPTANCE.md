# Milestone acceptance gates

A milestone is not complete until every mandatory gate passes in a reproducible environment.

## Acceptance doctrine

- Existence of a type, DTO, endpoint, file or test name is not sufficient acceptance evidence.
- Agent-visible correctness must be tested through a public/use-case boundary with real production components whenever practical.
- Trust-sensitive paths require negative-path acceptance: invalid, unsupported, incomplete, gap and stale-cursor behavior.
- A response may report `complete` only when authoritative evidence coverage proves that claim.
- A historical cycle may remain closed while its requirement status is `partial`/`gap`; the owning REC-C gate must close the residual obligation.
- Current compliance status is machine-readable in `/reconstruction-contracts.toml`.

## M0 historical gates

### UAT-M0-01 — non-destructive pagination
Given 10,000 records and `limit=100`, records 101..10,000 remain available to the same and another consumer.

### UAT-M0-02 — cumulative session evidence
Two snapshot/index refresh operations preserve evidence from both periods.

### UAT-M0-03 — tripwire reality
A real sandbox event fires one tripwire; label persists, fire count increments/is derivable from evidence, evidence points to a real event, delete prevents later fires.

### UAT-M0-04 — eBPF lifecycle
After probe creation returns, the probe remains attached until explicit stop/detach.

### UAT-M0-05 — explicit invalid filter
Unknown event type returns validation error and never broadens the query.

### UAT-M0-06 — state diff evidence
A fixture with two register/state snapshots produces the known difference.

## M1 historical gates

- two independent cursors receive the same ordered 1000 records;
- resume from cursor N starts at N+1 according to documented delivery semantics;
- forced bounded overflow emits `Gap`;
- interrupted segment write preserves committed segments.

## M2 historical gates

- recursive function creates distinct `InvocationId`s;
- process killed mid-function yields `Incomplete`, not synthetic return;
- full replay equals checkpoint+delta projection.

## M3 historical gates

- known `Order.total` transition `59 -> -35` violates `>=0`;
- causal slice contains known producer and excludes unrelated work;
- property requiring uncaptured historical field returns `UnsupportedByRecordedEvidence`.

## M4A Go

- existing manual OTel reused without duplicate Chronos operation span;
- OBI/zero-code sees expected supported coarse operation without product source change;
- Auto SDK correlation is proven against the same request context;
- compile-time `otelc` debug build adds selected deeper evidence while product git tree stays clean;
- `InstrumentationSpec` deterministically produces the intended debug instrumentation;
- coarse run -> hypothesis -> deep run -> proven root cause -> patched run passes property;
- exact/instrumented perturbation comparison is reported.

## M4B Rust

- existing Rust telemetry is reused/correlated before custom probes;
- targeted native/eBPF event observed without source modification;
- XRay spike either proves viable function capture or is explicitly rejected with measured reasons and superseding ADR;
- USDT spike has an accept/reject decision with evidence;
- temporary debug instrumentation captures one typed before/after mutation without product-branch modification;
- timing fixture changing under deep instrumentation is marked as perturbation and triggers fallback.

## M5

An agent-style test solves a fixture using only canonical API v2 tools. Legacy calls either map to v2 with an explicit compatibility waiver or are removed. MCP wrappers contain no application algorithm.

---

# Mandatory convergence acceptance

## REC-C0 — truth baseline

### UAT-REC-C0-01 — CI truth
The default workspace CI is green on the convergence branch before REC-C0 closes.

### UAT-REC-C0-02 — all features compile

```bash
cargo check --workspace --all-targets --all-features
```

passes. Feature-only code cannot be excluded from the trust baseline.

### UAT-REC-C0-03 — verified means evidence
Intentionally remove `uat` or `verify` from a temporary `verified` ledger entry; `scripts/check_architecture_contracts.py` must fail. Restore it and the checker must pass.

### UAT-REC-C0-04 — architecture drift ratchet
Add a temporary forbidden dependency edge or new production use of a protected legacy token; the architecture workflow must fail.

## REC-C1 — authoritative evidence and truthful reads

### UAT-REC-C1-01 — two independent consumers
Create 10,000 ordered records. A reads 100; B reads independently; producers advance; A resumes. A and B each observe their correct sequence and neither consumes the other's position.

### UAT-REC-C1-02 — real cursor semantics
For `events_read`, response cursor encodes/resolves an authoritative `EventSeq`. Resuming starts strictly after the acknowledged sequence. Reusing a cursor is non-destructive.

### UAT-REC-C1-03 — gap truth
Force bounded retention loss. The read crossing the lost interval returns an explicit gap/incomplete state and MUST NOT report `complete`.

### UAT-REC-C1-04 — cursor invalid/stale
Malformed, unknown-session or retention-stale cursors fail explicitly; none silently restart from offset zero.

### UAT-REC-C1-05 — time semantics
A sequence-only fixture cannot be serialized as monotonic nanoseconds. Wall-clock/Unix timestamps are independently validated.

## REC-C2 — legacy evidence deletion

### UAT-REC-C2-01 — EventBus cannot steal agent evidence
No canonical agent/query API reads authoritative evidence from destructive EventBus snapshots/drains.

### UAT-REC-C2-02 — tripwire evidence is replayable
Fire a tripwire, restart/replay the session, and derive the same fired evidence/count without relying on an in-memory fired queue.

### UAT-REC-C2-03 — no dual-truth divergence
A producer writes once to the authoritative path; live view and later query projection derive from that same record identity.

## REC-C3 — hexagonal boundary closure

### UAT-REC-C3-01 — dependency fitness
`python3 scripts/check_architecture_contracts.py` reports no unwaived forbidden dependency edge.

### UAT-REC-C3-02 — replace adapter without application change
Use a fake/in-memory implementation of each major port (probe factory/log/session store/notification) in application-service tests without importing concrete native/eBPF/browser/store crates.

### UAT-REC-C3-03 — domain is infrastructure-free
Domain tests compile without HTTP clients, async runtime delivery, filesystem/database or concrete tracer dependencies.

## REC-C4 — SOLID and connascence

### UAT-REC-C4-01 — capability honesty
For each tested backend, advertised capability operations succeed; absent capabilities are not selected by the planner and are not discovered by normal `UnsupportedOperation` failure.

### UAT-REC-C4-02 — typed identity isolation
Two sessions/probes/subscriptions with similar textual labels cannot be accidentally interchanged across typed application boundaries.

### UAT-REC-C4-03 — projection order independence
Eliminate any required `drain -> merge -> rebuild` sequence from the canonical evidence path; incremental projection replay yields the same result after restart.

## REC-C5 — canonical Agent API

### UAT-REC-C5-01 — v2-only diagnosis
An agent fixture performs start -> observe/read/query -> slice/property/diff -> explain/export using only canonical v2 operations.

### UAT-REC-C5-02 — invalid input never broadens
Unknown enum/filter/mode/target fails validation rather than becoming `None`/all-events/default scope.

### UAT-REC-C5-03 — legacy surface shrinks
The compatibility inventory count cannot increase. Each remaining shim has owner, reason and removal gate/date. REC-C7 target is zero unless a specific ADR preserves one.

## REC-C6 — unfinished reconstruction contracts

### UAT-REC-C6-GO
Run the complete M4A Go acceptance flow using real Go sandbox code and record mechanism/overhead/provenance.

### UAT-REC-C6-RUST
Run the complete M4B Rust acceptance flow including perturbation fallback and record accept/reject decisions for XRay/USDT.

### UAT-REC-C6-REPLAY
Full replay equals checkpoint+delta for every promoted projection type.

### UAT-REC-C6-SUBSCRIPTIONS
Observe/tripwire/property conditions that share the same evidence source do not maintain contradictory independent delivery state.

## REC-C7 — convergence close

Mandatory commands:

```bash
python3 scripts/check_architecture_contracts.py --strict-no-gaps
cargo check --workspace --all-targets --all-features
cargo test --workspace -- --test-threads=1
```

plus the dedicated sandbox UAT suite.

REC-C7 may close only when:

- no convergence-owned ledger item is `gap`, `partial` or `blocked`;
- default CI and architecture-contract CI are green;
- no P0/P1 false-confidence path remains;
- convergence-owned legacy/dependency waivers are empty or explicitly superseded by a new accepted ADR;
- README/API capability claims are a subset of verified ledger capabilities;
- an independent source review can trace every `verified` claim to executable evidence.

---

## M6

A two-service request links:

```text
external trace/span -> Chronos invocation -> state mutation -> property violation
```

without equating span to invocation and without using session-relative monotonic time as Unix wall clock.

## M7

Comparison ignores harmless timing/order noise and finds the known semantic divergence.

## M8

A generated failing input shrinks while preserving the same property violation.

## M9

A lock-protected same-address scenario is not a confirmed race; an unsynchronized fixture is supported by happens-before evidence.

## M10

UI renders a large trace through aggregation/virtualization without materializing every event at once.
