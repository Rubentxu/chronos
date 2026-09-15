# Reconstruction convergence backlog

This file decomposes REC-C0..REC-C7 into mergeable development cycles. Each cycle must be small enough to verify independently and must either reduce known debt or add a prerequisite seam that is immediately exercised.

## Delivery rules

- one product/convergence cycle in progress at a time;
- no giant rewrite branch;
- every cycle names the invariant it improves and the legacy/dependency count it should reduce;
- every cycle adds/updates UAT before removing the old path;
- compatibility code is deleted in the same cycle that proves its replacement unless an explicit next-cycle dependency is recorded;
- a cycle cannot close with a stronger capability claim than its evidence proves;
- architectural mechanism may evolve, but the ADR-0011 north-star invariants may not weaken.

---

## REC-C0 — Restore the truth baseline

### REC-C0.1 — Restore default CI

Deliverables:
- identify current unit/doc-test failure on `main`;
- fix root cause rather than quarantine unless proven platform-flaky;
- coverage workflow green;
- record failure class in close evidence.

Gate: default CI green.

### REC-C0.2 — Feature-matrix truth

Deliverables:
- make `cargo check --workspace --all-targets --all-features` green;
- fix/remove domain webhook feature leakage;
- ensure optional features declare every dependency they use;
- add targeted feature tests when runtime behavior differs.

Gate: Architecture Contracts workflow green through all-feature step.

### REC-C0.3 — Compliance-ledger hardening

Deliverables:
- reconcile every major reconstruction requirement with current source;
- add missing requirement rows;
- add evidence/UAT/command to all verified rows;
- add CI test that intentionally malformed verified records are rejected.

Gate: ledger checker green and review finds no untracked P0/P1 audit finding.

### REC-C0.4 — Product/document truth

Deliverables:
- capability matrix by backend/language;
- README claims limited to verified depth;
- remove stale 32+/uniform-support language;
- namespace rules (`REC-C*`, `M*`, governance prefixes) adopted by new cycles.

Gate: docs/API claims are a subset of ledger `verified` capability claims.

---

## REC-C1 — ExecutionLog cutover and truthful reads

### REC-C1.1 — Authoritative cursor model

Deliverables:
- define agent cursor as opaque wire encoding of authoritative `EventSeq` + session identity/version;
- explicit malformed/stale/wrong-session errors;
- no cursor field derived from page length or total matches;
- tests for replay and idempotent reread.

### REC-C1.2 — Session-owned mandatory ExecutionLog

Deliverables:
- every agentic session gets an ExecutionLog at creation;
- session lifecycle owns log, producer handles and seal/incomplete state;
- EventBus-only mode unavailable to canonical agent path;
- memory-only ExecutionLog backend allowed for tests/ephemeral sessions, preserving same semantics.

### REC-C1.3 — `events_read` cutover

Deliverables:
- query from authoritative sequence/log/projection cursor;
- remove hard-coded offset zero behavior;
- real `next_cursor`;
- real provenance;
- explicit query coverage/completeness enum rather than free-form string.

### REC-C1.4 — Gap/completeness contract

Deliverables:
- projection/query exposes intersecting gaps;
- `complete` impossible when gap/unknown coverage intersects requested range;
- add `KnownComplete | Incomplete{...} | UnknownCoverage` or equivalent typed domain result;
- UAT forced overflow and stale retention.

### REC-C1.5 — Incremental projection ownership

Deliverables:
- QueryEngine/index state advances from ExecutionLog seq checkpoints;
- restart replay/checkpoint equivalence;
- remove cumulative merge workaround from canonical path.

Gate REC-C1: UAT-REC-C1-01..05 all green.

---

## REC-C2 — Legacy evidence/event deletion

### REC-C2.1 — Live view projection

Replace semantic EventBus reads with an ExecutionLog-backed live projection/subscription. Measure latency/overhead; keep bounded adapter transport only if needed.

### REC-C2.2 — Tripwire evidence convergence

Deliverables:
- `TripwireFired` appended as evidence/projection event;
- replayable fire history/count;
- remove independent destructive fired queue;
- label/condition/action metadata preserved by stable ID.

### REC-C2.3 — Remove destructive public reads

Delete/privatize `snapshot`, `snapshot_raw`, agent-visible `drain` and equivalent shared destructive evidence APIs.

### REC-C2.4 — Remove dual-write truth

After all canonical consumers read the log, remove `EventBus + ExecutionLog` dual authoritative write. If an adapter-local channel remains, it is transport-only and cannot be queried as historical evidence.

### REC-C2.5 — EventBus deletion or quarantine

Target: delete EventBus. If retained for bounded local transport, move it to adapter infrastructure, rename to communicate transport semantics, and keep it out of domain/API/query dependencies.

Gate REC-C2: LEGACY-001/002 verified and legacy ratchet baseline reduced accordingly.

---

## REC-C3 — Hexagonal boundary closure

### REC-C3.1 — Define application ports

Introduce only ports required by real use cases:
- `ProbeFactory` / `ProbeRegistry`;
- `ExecutionLogProvider`;
- `SessionRepository`;
- `NotificationSink`;
- `TelemetryReceiver`;
- `ArtifactStore` / address-symbolization port if still needed.

Avoid generic repository/service abstractions with no second implementation/test double.

### REC-C3.2 — Extract webhook infrastructure

Move HTTP delivery/retry/spawn from `chronos-domain` to a driven adapter. Domain keeps pure event/command data. Remove `reqwest`, async runtime/logging infrastructure from domain dependencies.

### REC-C3.3 — Invert services -> concrete adapter dependencies

Move native/eBPF/browser/store construction to composition root. `chronos-services` receives ports/capabilities.

### REC-C3.4 — Remove store -> native

Extract address normalization/symbolization behind a port owned by domain/application semantics; implement it in native adapter where appropriate.

### REC-C3.5 — Dependency graph gate to zero

Remove corresponding entries from `known_dependency_violations` as code lands. A cycle is incomplete if the code edge disappears but the waiver remains.

Gate REC-C3: no unwaived forbidden edge and no convergence-owned dependency waiver.

---

## REC-C4 — SOLID + connascence reduction

### REC-C4.1 — Capability interface split

Replace wide `TraceAdapter` use with narrow capabilities:
- `ProbeSource`;
- `AttachCapability`;
- `StackInspector`;
- `ValueInspector`;
- `ExpressionEvaluator`;
- `RuntimeMetadataProvider`.

Adapters implement only supported ports.

### REC-C4.2 — Capability-first planner

`capabilities()` returns mechanism, evidence depth, perturbation/risk, supported queries and requirements. Planner selects a path before invocation; `UnsupportedOperation` is exceptional rather than normal discovery.

### REC-C4.3 — Strong IDs

Replace identity-bearing inner strings with newtypes where they cross use-case boundaries (`SessionId`, `ProbeId`, `SubscriptionId`, `ConsumerId`, `SnapshotId`). Keep serialization at adapters.

### REC-C4.4 — Time semantics

Separate `EventSeq`, monotonic elapsed time and wall-clock timestamp. Add conversion/correlation only where mathematically defined.

### REC-C4.5 — Responsibility hotspots

Review largest service/server modules. Extract only cohesive use cases. Target symptoms rather than line-count quotas: backend construction, query algorithms, persistence policy or graph traversal do not belong in MCP wrappers.

Gate REC-C4: no new broad optional adapter interface, no high-severity connascence-of-meaning/execution finding on canonical path.

---

## REC-C5 — Agent API convergence

### REC-C5.1 — Canonical v2 contract freeze

Confirm canonical tool set and typed request/response schemas. Each response carries evidence/provenance/completeness semantics appropriate to its operation.

### REC-C5.2 — v2-only end-to-end fixture

Real agent-style fixture uses no v1 tools. Cover success + invalid/unsupported/incomplete paths.

### REC-C5.3 — Compatibility module isolation

Move legacy wrappers to one compatibility module/crate boundary. Maintain machine-readable inventory with replacement and deletion gate.

### REC-C5.4 — Shim deletion waves

Delete shims in small waves by use-case family after compatibility/UAT proof. The count must decrease every deletion cycle.

Gate REC-C5: API-001/API-002 verified; remaining shim count zero or explicitly accepted by new ADR.

---

## REC-C6 — Finish promised reconstruction foundations

### REC-C6.1 — M2/M3 residual closure

- real killed-mid-invocation incomplete evidence;
- projection checkpoint/delta replay equivalence;
- authoritative property historical evidence;
- subscription model convergence for observe/tripwire/property.

### REC-C6.2 — Go adaptive instrumentation discovery

- detect/reuse existing OTel;
- OBI feasibility and supported signal matrix;
- Auto SDK correlation;
- measured overhead/provenance.

### REC-C6.3 — Go compile-time deepening

- `otelc` experiment;
- deterministic `InstrumentationSpec`;
- clean product tree guarantee;
- exact vs instrumented behavioral/latency comparison.

### REC-C6.4 — Rust coarse/adaptive baseline

- existing `tracing`/OTel reuse;
- targeted eBPF;
- capability/provenance reporting.

### REC-C6.5 — Rust XRay + USDT decision spikes

Each spike ends in accepted/rejected/superseded ADR with measured constraints; no permanent half-integrated experiment.

### REC-C6.6 — Rust typed semantic mutation probe

One temporary debug overlay captures typed before/after state, feeds property evidence and is removed without product-source residue.

### REC-C6.7 — Perturbation fallback UAT

Timing-sensitive fixture demonstrates that deeper instrumentation changing behavior is detected and causes a fallback to lighter evidence.

Gate REC-C6: all M1–M4 ledger rows previously claimed as delivered are verified or explicitly moved to a later official milestone with a truthful capability downgrade.

---

## REC-C7 — Convergence close

### REC-C7.1 — Strict machine gate

Run `scripts/check_architecture_contracts.py --strict-no-gaps`; all convergence-owned requirements verified.

### REC-C7.2 — Full verification bundle

- fmt/clippy/build;
- all features;
- workspace unit/integration/doc tests;
- sandbox UATs;
- replay tests;
- negative No-Silent-Lies tests.

### REC-C7.3 — Independent architecture re-audit

Review hexagonal direction, SOLID, connascence, smells and false-confidence risks from source. P0/P1 = close blocker. P2 must have owner and target milestone.

### REC-C7.4 — Delete temporary governance debt

- remove obsolete waivers;
- remove convergence-only compatibility notes;
- reconcile README/capability matrix;
- archive close evidence.

### REC-C7.5 — Promote M6

Only after 7.1–7.4 pass, mark official M6 OpenTelemetry correlation/export `in_progress`.

---

## Metrics that matter

Track trends, not vanity scores:

- count of forbidden dependency waivers (must decrease to zero for convergence-owned edges);
- count of production legacy-token references (must decrease, never increase);
- canonical vs compatibility MCP tool count;
- number of verified/partial/gap contracts;
- UAT coverage of negative trust states;
- percentage of agent-visible reads sourced from authoritative ExecutionLog (target 100%);
- replay-equivalence failures (target zero);
- false `complete`/unknown-as-empty incidents (target zero).

Do not use raw LoC reduction, crate count or test count as success criteria without a behavioral invariant.
