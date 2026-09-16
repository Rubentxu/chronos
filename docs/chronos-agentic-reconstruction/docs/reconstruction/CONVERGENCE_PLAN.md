# Chronos Reconstruction Convergence Plan

Status: **ACTIVE**
Owner namespace: `REC-C*`
Precedence: accepted ADRs > current specs > this plan > historical proposals.

## Why this plan exists

The reconstruction delivered substantial foundations (`chronos-log`, invocation identity, runtime properties, service extraction and v2 dispatchers), but the repository still contains parallel legacy paths and several contracts whose public semantics are stronger than their implementation. The immediate objective is therefore **convergence, not feature expansion**.

No reconstruction milestone M6+ may be promoted to active while a mandatory REC-C gate is red.

## North-star invariants

1. **One authoritative evidence stream.** Agent-visible evidence comes from `ExecutionLog`; bounded buses may exist only as internal transport.
2. **No silent lies.** Unknown, unsupported, incomplete, dropped, heuristic and estimated evidence are explicit states, never empty/default success.
3. **Independent consumers.** One consumer can never steal, truncate or advance another consumer's evidence.
4. **Hexagonal dependency direction.** Domain and application policy depend on ports; concrete tracers, stores, HTTP and MCP depend inward.
5. **Capabilities over optional-method interfaces.** Backends implement small capability ports rather than a wide interface full of `UnsupportedOperation` defaults.
6. **Strong identity.** Session, sequence, invocation, symbol, probe, subscription, property and snapshot identities are typed at domain/application boundaries.
7. **Executable specifications.** A requirement is `verified` only when its real path, UAT and reproducible verification command are recorded.
8. **Legacy can only shrink.** CI rejects newly introduced use of legacy evidence paths and architectural dependency violations.

## Legacy inventory to eliminate

### Evidence transport / reads

- `chronos_domain::bus::EventBus` as an agent/query source.
- destructive `snapshot`, `snapshot_raw` and `drain` semantics in agent-visible paths.
- dual truth (`EventBus` + optional `ExecutionLog`).
- QueryEngine copies treated as authoritative session state.
- cursor DTOs that do not map to an authoritative `EventSeq` position.

### Tripwire/event notification

- independent `fired_buffer` queue.
- mutable counters whose state cannot be reconstructed from evidence.
- webhook HTTP delivery inside `chronos-domain`.

### Adapter contracts

- wide `TraceAdapter` capability surface with default unsupported operations.
- application services constructing or depending directly on `chronos-native`, `chronos-ebpf`, `chronos-browser` or concrete stores.
- `chronos-store -> chronos-native` address-normalization dependency.

### API / compatibility

- legacy MCP tools after equivalent v2 contracts are proven.
- manual string-to-enum protocol mappings where typed DTOs can cross the boundary.
- plausible defaults for missing targets or unknown filters.

### Documentation / governance

- milestone names that conflate internal cycle namespaces with reconstruction milestones.
- README claims that exceed verified capability depth.
- close reports that prove structure but not behavioral invariants.

---

# REC-C0 — Restore the truth baseline

## Goal

Make `main` trustworthy before migration work continues.

## Work

- restore CI and coverage to green;
- add all-feature compilation/testing or an explicit feature matrix;
- create the machine-readable `reconstruction-contracts.toml` ledger;
- install architecture/spec fitness checks;
- classify every reconstruction requirement as `verified`, `partial`, `gap`, `planned` or `blocked`;
- add explicit owners/evidence/UAT/verification commands for every `verified` item;
- fix current documentation drift and product positioning.

## Exit gate

- default CI green;
- architecture contract workflow green;
- no `verified` ledger entry lacks evidence + UAT + command;
- no active milestone claims closure solely from file/tool existence.

# REC-C1 — ExecutionLog cutover and truthful reads

## Goal

Make `ExecutionLog` the sole authoritative evidence source for agentic sessions.

## Work

- require an ExecutionLog for every agentic session;
- route native producer, live reads, snapshots and final reads through it;
- redesign `events_read` around `EventSeq` cursor semantics;
- return real completeness/gap/provenance metadata;
- make QueryEngine an incremental/replayable projection, never primary state;
- define timestamp fields so ordinal sequence is never stored as nanoseconds;
- seal/mark incomplete session tails explicitly.

## Mandatory UAT

Two consumers read the same 10,000-record session independently. Consumer A reads 100 records, pauses while producers advance and resumes from its `EventSeq`; consumer B reads independently. No record is stolen. Forced overflow creates an explicit `Gap`. A request cannot return `complete` if a gap is present or evidence coverage is unknown.

# REC-C2 — Remove legacy evidence/event paths

## Goal

Delete the old architecture after C1 proves replacement paths.

## Work

- remove agent/query reads from EventBus;
- reduce EventBus to a private adapter-local transport or delete it;
- delete destructive shared read APIs from public surfaces;
- model `TripwireFired` as ExecutionLog evidence;
- derive/project fire count from evidence instead of maintaining a disconnected queue;
- remove `fired_buffer`;
- remove dual-write compatibility when all consumers have migrated;
- delete stale/deprecated code rather than leaving permanent shims.

## Exit gate

Repository searches for forbidden legacy symbols are zero outside explicitly documented adapter-private compatibility code, and that allowlist is empty by REC-C7.

# REC-C3 — Hexagonal boundary closure

## Goal

Make dependency direction mechanically enforceable.

## Target layers

```text
Driving adapters
  MCP / CLI / HTTP / tests
          |
          v
Application services
  Session / Probe / Observe / Property / Query / Diff
          |
          v
Domain + ports
  evidence, properties, identities, capability contracts
          ^
          |
Driven adapters
  native / eBPF / browser / OTLP / redb / filesystem / webhook
```

## Work

- introduce narrow ports such as `ProbeFactory`, `ProbeRegistry`, `ExecutionLogProvider`, `SessionRepository`, `NotificationSink`, `TelemetryReceiver` and `ArtifactStore`;
- move concrete construction to composition roots;
- remove `reqwest`/HTTP/tokio delivery policy from domain;
- remove `chronos-services -> concrete adapters` dependencies;
- remove `chronos-store -> chronos-native` dependency and express address normalization as a port/service;
- add dependency fitness rules that block future reversals.

## Exit gate

The machine-checker reports no unwaived dependency-direction violations.

# REC-C4 — SOLID + connascence reduction

## Goal

Reduce change coupling and ambiguous runtime contracts.

## Work

- split `TraceAdapter` into capability ports (`ProbeSource`, `AttachCapability`, `StackInspector`, `ValueInspector`, `ExpressionEvaluator`, `RuntimeMetadataProvider`);
- make `capabilities()` authoritative for agent planning;
- replace protocol strings with typed enums/newtypes at inner boundaries;
- introduce typed `SessionId`, `ProbeId`, `SubscriptionId`, `ConsumerId`, `PropertyId`, `SnapshotId` where raw strings still carry identity;
- replace connascence-of-execution (`drain -> merge -> rebuild`) with append + incremental projection;
- separate wall clock, monotonic time and sequence ordering;
- decompose oversized service/server modules where responsibilities still cross use-case boundaries.

## Exit gate

- no new default `UnsupportedOperation` methods in shared adapter traits;
- capability tests prove unsupported operations are absent from capability sets rather than discovered by failure;
- architecture review has no high-severity connascence-of-execution/meaning finding.

# REC-C5 — Agent API convergence and legacy API deletion

## Goal

Finish M5's product-level objective, not only its extraction objective.

## Canonical v2 surface

Target approximately 8–12 composable operations:

- `session_start`
- `session_stop`
- `capabilities`
- `observe`
- `events_read`
- `execution_query`
- `state_query`
- `trace_slice`
- `hypothesis_test`
- `session_compare`
- `session_explain`
- `session_export`

## Work

- prove every canonical v2 tool against real UAT fixtures;
- move legacy aliases behind a compatibility module with an explicit sunset inventory;
- delete aliases as soon as compatibility gates permit;
- prohibit new application algorithms in MCP wrappers;
- fail malformed/unknown filters explicitly.

## Exit gate

An agent solves representative fixtures using only canonical v2 tools; legacy surface count is zero or has an individually justified, dated compatibility waiver.

# REC-C6 — Close unfinished reconstruction foundations

## Goal

Align implementation with the work already promised before advancing roadmap features.

## M4A Go

- existing OTel discovery;
- OBI/zero-code baseline;
- Auto SDK correlation validation;
- `otelc` compile-time debug instrumentation;
- deterministic `InstrumentationSpec -> Go instrumentation`;
- exact vs instrumented perturbation comparison.

## M4B Rust

- existing `tracing`/OTel discovery;
- OBI/native coarse baseline;
- targeted eBPF;
- XRay viability spike with measured accept/reject ADR;
- USDT viability spike;
- typed temporary semantic mutation probe;
- perturbation detection/fallback.

## M1/M2/M3 residuals

- authoritative ExecutionLog cutover;
- incomplete invocation semantics on real terminated processes;
- replay/checkpoint equivalence for projections;
- unified subscription/condition/action model for observe/tripwire/property;
- historical evidence returns explicit unsupported/incomplete states.

## Exit gate

No item marked as delivered in the reconstruction roadmap is `gap` in the machine ledger.

# REC-C7 — Reconstruction convergence close

## Goal

Prove the repository is ready to resume M6+ feature development.

## Mandatory close evidence

- clean CI including feature matrix;
- architecture fitness gate green;
- spec compliance ledger has no unowned gap;
- legacy allowlist empty for EventBus/query paths, broad TraceAdapter and forbidden dependency edges;
- all mandatory UATs run against real executable fixtures;
- README/product docs match verified capabilities;
- fresh architecture review reports no P0/P1 false-confidence path;
- roadmap drift check confirms cycle names and reconstruction milestones are unambiguous.

Only after REC-C7 closes may official reconstruction **M6 OpenTelemetry correlation + export** become active.

---

# Post-convergence roadmap alignment

After REC-C7:

1. **M6 OpenTelemetry correlation/export** — ExternalTraceContext, OTLP ingestion, provenance and correlation.
2. **M7 Differential execution v2** — semantic alignment, hierarchical hashes and BehaviourFingerprint.
3. **M8 Test intelligence** — continue shrinking from the existing foundation into end-to-end rerun/slice workflows.
4. **M9 Concurrency intelligence** — happens-before evidence, not time-window race guesses.
5. **M10 Execution Explorer** — Chronos-specific evidence UI only after evidence contracts are trustworthy.
6. **M11 Language depth** — capability matrix + real UAT per runtime.

## Rule for emergent architecture

A spike may change a technical mechanism, but it may not weaken a north-star invariant. If a mechanism changes, update an ADR/spec/ledger entry in the same cycle. A close report must state both what passed and what remains unsupported.
