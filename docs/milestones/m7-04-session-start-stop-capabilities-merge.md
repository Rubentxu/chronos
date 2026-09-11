# m7-04 — `session_start` + `session_stop` + `capabilities` merge

**Branch:** `feat/m7-04-session-start-stop-capabilities-merge`
**Cycle:** M7 (v2-spec sub-cycle), fourth deliverable
**Precedence:** `docs/milestones/m7-04-session-start-stop-capabilities-scoping.md`;
`docs/milestones/m7-events-read-scoping.md`; v2 spec
`AGENT_API_V2.md` lines 11–13
**Status:** PROPOSED, 2026-09-11

## Why this cycle

The v2 spec calls for three lifecycle / introspection tools:

* `session_start` — start/attach and return `SessionId` + capability snapshot
* `session_stop` — gracefully end producers and seal tail
* `capabilities` — available evidence mechanisms for target/session

Today the session lifecycle is split across five v1 MCP tools:

* `probe_start` — spawn a process under a probe (v1 shape).
* `probe_stop` — stop a probe, drain, build query engine (v1 shape).
* `probe_drain` — drain current events without stopping (read-only path).
* `session_save` / `session_load` / `drop_session` / `session_snapshot`
  — persistence path, kept as-is for m7-04.

The v1 tools are the closest matches to `session_start` / `session_stop`
but have **non-trivially different shapes** from the v2 spec target
(especially the `+capability snapshot` requirement and the `+seal tail`
semantic). `capabilities` is **net-new**: no v1 tool backs it.

m7-04:

1. **Adds a net-new `session_start` v2 tool** with
   `action: "spawn" | "load" | "attach"` discriminator.
   `spawn` preserves the v1 `probe_start` parameter set 1:1 (existing
   v1 `probe_start` becomes a deprecated shim).
   `load` takes a `session_id` from the store.
   `attach` (new path) is `ServiceError::Unsupported` for m7-04
   until the domain-layer attach API exists.
2. **Adds a net-new `session_stop` v2 tool** that wraps `probe_stop`
   semantics plus two new flags:
   `seal_tail: bool = true` (default-on, marks the session
   metadata `tail_sealed=true` + `sealed_at=<now>`),
   `drain_subscriptions: bool = true` (default-on, drains
   `observe{verb=list}` before stop). v1 `probe_stop` becomes a
   deprecated shim mapping to `session_stop` with both defaults.
3. **Adds a net-new `capabilities` v2 tool** with two scopes:
   `target` (static enumeration of what evidence mechanisms a target
   can support before any session is started) and `session_id`
   (dynamic enumeration of what the producer has emitted so far).

This is the fourth M7 deliverable (after m7-01 events_read, m7-02
observe, m7-03 session_compare + session_explain). After m7-04 ships,
the M7 backlog has m7-05 (deprecation sunset, calendared for
2027-09-11) and m7-06 (M7 close) remaining.

---

## Scope (this cycle)

- Add `chronos-services::session_lifecycle` module (~400 LoC, 20th
  service module).
- Extend `chronos-store::SessionMetadata` with two additive fields:
  `tail_sealed: bool` (default `false`) and
  `sealed_at: Option<u64>` (default `None`). Bump schema version in
  the metadata header to `v3`.
- Add `SessionStartAction` enum
  (`Spawn | Load | Attach`).
- Add `SessionStartInput` / `SessionStartOutput` DTOs (per-action
  fields, plus `provenance`).
- Add `SessionStopInput` / `SessionStopOutput` DTOs
  (`session_id`, `seal_tail`, `drain_subscriptions`, `sealed_at`,
  `total_events_drained`, `provenance`).
- Add `CapabilitiesInput` / `CapabilitiesOutput` DTOs
  (target + session_id + `StaticCapabilities` + `DynamicCapabilities`
  + `provenance`).
- Add `ChronosSessionLifecycleService` dispatcher with three
  entry points: `start`, `stop`, `capabilities`. Unit tests for each.
- Add three v2 MCP tool wrappers: `session_start`, `session_stop`,
  `capabilities`.
- Convert v1 `probe_start` to a deprecated shim mapping to
  `session_start{action=spawn}`.
- Convert v1 `probe_stop` to a deprecated shim mapping to
  `session_stop` with both defaults on.
- Update `chronos_services::lib.rs` module index (19 → 20 algorithm
  modules after m7-03; 20 → 21 after m7-04).

## Out of scope

- `observe` already shipped (m7-02). m7-04 only consumes
  `observe{verb=list}` from `session_stop{drain_subscriptions=true}`.
  No m7-04 changes to `observe`.
- `events_read` (m7-01), `session_compare` + `session_explain`
  (m7-03) — no m7-04 changes.
- `session_save` / `session_load` / `drop_session` /
  `session_snapshot` — v1 persistence tools, kept as-is for m7-04.
- `probe_drain` — non-destructive read, not lifecycle. Kept as-is.
- Deprecation sunset sweep (m7-05).
- M7 close (m7-06).
- `session_start{action=attach}` runtime implementation — rejected
  with `ServiceError::Unsupported` for m7-04 (domain-layer attach API
  is m7+).
- Per-language capability probing (e.g. "does this Go binary support
  stack-trace introspection?") — m7+.
- Richer provenance (lineage graph, staleness windows) — m7+.
- Tail-log file format — m7-04 seals via metadata fields, not a
  separate file.

## Algorithm

### `session_start{action=spawn}`

Wraps the existing
`chronos_services::probe::ProbeService::start` algorithm:

* Input: `{ program, args?, cwd?, trace_syscalls?, bus_capacity?,
  track_function_frames? }` (same as v1 `ProbeStartInput`).
* Algorithm: `ProbeService::start` (process spawn + eBPF attach +
  bus provisioning) → returns `session_id`.
* Capability snapshot: static enumeration
  `{ probe_type: "ebpf_user", language: <detected>,
  target_event_types: <from EventType enum>, language_adapters:
  <active for language>, project: [function_entry, function_exit,
  syscall_enter, syscall_exit, signal_delivered, variable_write,
  memory_write] }` + dynamic enumeration of the new session
  (`bus_capacity`, `bus_fill: 0`, `query_engine_ready: false`).
* Output: `SessionStartOutput { session_id, action: "spawn",
  target, language, bus_capacity, capability_snapshot,
  provenance }`.

### `session_start{action=load}`

Loads a previously persisted session into memory:

* Input: `{ session_id }`. `path` optional override deferred to m7+.
* Algorithm: `chronos_store::SessionStore::load_session` →
  `ChronosServer::build_and_store_engine` (mirrors `probe_stop`'s
  final step but skips the producer).
* Capability snapshot: dynamic only (the session already exists, so
  static is taken from the existing metadata).
* Output: `SessionStartOutput { session_id, action: "load",
  event_count, duration_ms, language, capability_snapshot,
  provenance }`.

### `session_start{action=attach}`

Stub for m7-04. Returns
`ServiceError::Unsupported("attach (m7+) — no domain-layer attach
API yet")`. Wire shape is documented so callers can compose for
m7+ without a breaking change.

### `session_stop`

Wraps the existing
`chronos_services::probe::ProbeService::stop` algorithm plus two
new steps:

* Input: `{ session_id, seal_tail: bool = true,
  drain_subscriptions: bool = true }`.
* Algorithm:
  1. If `drain_subscriptions` (default-on):
     `ChronosObserveService::observe{verb=list}` — destructive
     drain of fired events.
  2. `ProbeService::stop` (terminate producer + drain ring buffer +
     build query engine) → `result.events`,
     `result.total_events`, `result.duration_ms`,
     `result.ebpf_detached`.
  3. If `seal_tail` (default-on): update `SessionMetadata` →
     `tail_sealed = true`, `sealed_at = Some(now)`. Persist back
     to the store.
  4. Return `SessionStopOutput`.
* Output: `SessionStopOutput { session_id, status: "stopped",
  target, total_events, duration_ms, ebpf_detached,
  sealed_at: Option<u64>, drained_subscriptions: bool,
  capability_snapshot: <final dynamic snapshot>,
  provenance }`.

### `capabilities`

Read-only introspection. Two scopes:

* Input: `target: Option<TargetSpec>`, `session_id: Option<String>`.
  At least one required; both allowed for combined static+dynamic
  view. If only `target` is given, the `static` field is filled
  and the `dynamic` field is `None`; if only `session_id` is
  given, the `static` field is read from the session's metadata
  (the target it was started against) and the `dynamic` field is
  filled.
* Algorithm:
  - **Static**: from `Language` (target detected) +
    `EventType` enum + `LanguageAdapterStatus` registry →
    `{ probe_type, language, target_event_types, language_adapters,
    projections: [events_read, execution_query, state_query,
    trace_slice, session_compare, session_explain] }`.
  - **Dynamic**: from the live session's bus + QueryEngine →
    `{ bus_capacity, bus_fill, event_types_emitted,
    event_type_counts: HashMap<EventType, u64>,
    query_engine_ready, active_subscriptions: Vec<SubscriptionId>,
    tail_sealed, sealed_at }`.
* Output: `CapabilitiesOutput { static: Option<StaticCapabilities>,
  dynamic: Option<DynamicCapabilities>, provenance }`.

## DTOs (new in `output.rs`)

```text
SessionStartAction         { Spawn, Load, Attach }
SessionStartInput          { action, spawn_fields?, session_id?, path? }
SessionStartOutput         { session_id, action, target?, language?,
                             event_count?, duration_ms?,
                             bus_capacity?, capability_snapshot,
                             provenance }
SessionStopInput           { session_id, seal_tail: bool = true,
                             drain_subscriptions: bool = true }
SessionStopOutput          { session_id, status: "stopped", target,
                             total_events, duration_ms,
                             ebpf_detached, sealed_at,
                             drained_subscriptions: bool,
                             capability_snapshot,
                             provenance }
CapabilitiesInput          { target: Option<TargetSpec>,
                             session_id: Option<String> }
CapabilitiesOutput         { static: Option<StaticCapabilities>,
                             dynamic: Option<DynamicCapabilities>,
                             provenance }
StaticCapabilities         { probe_type, language,
                             target_event_types: Vec<EventType>,
                             language_adapters: Vec<LanguageAdapterStatus>,
                             projections: Vec<ProjectionKind> }
DynamicCapabilities        { bus_capacity, bus_fill,
                             event_types_emitted: Vec<EventType>,
                             event_type_counts: HashMap<EventType, u64>,
                             query_engine_ready: bool,
                             active_subscriptions: Vec<String>,
                             tail_sealed: bool,
                             sealed_at: Option<u64> }
TargetSpec                 { program: String, args: Vec<String>,
                             language: Option<Language> }
LanguageAdapterStatus      { language, available: bool, reason: Option<String> }
ProjectionKind             { EventsRead, ExecutionQuery, StateQuery,
                             TraceSlice, SessionCompare,
                             SessionExplain }
SessionLifecycleProvenance { engine_version, source }
```

## `SessionMetadata` extension

```text
// crates/chronos-store/src/storage.rs (existing struct)
pub struct SessionMetadata {
    pub session_id: String,
    pub created_at: u64,
    pub language: String,
    pub target: String,
    pub event_count: usize,
    pub duration_ms: u64,
    // New in m7-04:
    #[serde(default)]
    pub tail_sealed: bool,
    #[serde(default)]
    pub sealed_at: Option<u64>,
}
```

The `#[serde(default)]` decorators ensure backward compatibility:
metadata files written before m7-04 deserialize with
`tail_sealed=false, sealed_at=None` so the new reader is a
superset of the old schema. (Note: `SessionMetadata` does not
currently carry a `schema_version` field — m7-04 does not add one
either; the additive bump is implicit in the `#[serde(default)]`
design and round-trip unit tests.)

## v1 shim JSON shape preservation

| v1 tool | v2 mapping | Preserved shape |
|---|---|---|
| `probe_start` | `session_start{action=spawn}` | `{session_id, status, target, language, bus_capacity, hint}` |
| `probe_stop` | `session_stop` (with defaults) | `{session_id, status: "stopped", target, total_events, duration_ms, ebpf_detached, hint}` |

The shims drop the new `capability_snapshot` field (only the v2
tool exposes it; v1 callers see exactly what they did before m7-04)
and the new `sealed_at` / `drained_subscriptions` (only the v2 tool
exposes them; v1 callers see no difference).

## File changes (planned)

* `crates/chronos-services/src/output.rs` (+250 LoC: 11 new types)
* `crates/chronos-services/src/error.rs` (no new variants — reuse
  `Unsupported(String)` from m7-02)
* `crates/chronos-services/src/session_lifecycle.rs` (new, ~400
  LoC: `SessionLifecycleContext<'_>`, `SessionStartInput`,
  `SessionStopInput`, `CapabilitiesInput`,
  `ChronosSessionLifecycleService::{start, stop, capabilities}`,
  ~9 unit tests)
* `crates/chronos-store/src/session_metadata.rs` (+10 LoC: two
  additive fields, `schema_version` bump)
* `crates/chronos-services/src/lib.rs` (+10 LoC: register
  `session_lifecycle` module, update module index 19 → 20
  algorithm modules after m7-03; 20 → 21 after m7-04)
* `crates/chronos-mcp/src/server.rs` (+260/−70 net:
  `SessionStartParams`, `SessionStopParams`, `CapabilitiesParams`,
  3 tool wrappers, 2 v1 shim conversions, 1 parser helper)

## Test plan (planned)

`crates/chronos-services/src/session_lifecycle.rs`:

1. `start_spawn_returns_session_id_with_capability_snapshot`
2. `start_load_returns_session_id_with_dynamic_capability_snapshot`
3. `start_attach_returns_unsupported`
4. `start_spawn_propagates_probe_start_error`
5. `stop_seals_tail_when_seal_tail_true`
6. `stop_does_not_seal_when_seal_tail_false`
7. `stop_drains_subscriptions_when_drain_subscriptions_true`
8. `capabilities_target_only_returns_static_only`
9. `capabilities_session_only_returns_dynamic_only`
10. `capabilities_target_and_session_returns_both`
11. `provenance_present_on_all_three_tools`

`chronos-sandbox/tests/probe_lifecycle.rs` (extend existing):

1. `test_session_start_spawn_via_v2`
2. `test_session_stop_via_v2`
3. `test_session_stop_seals_tail`
4. `test_probe_start_v1_shim_preserves_shape`
5. `test_probe_stop_v1_shim_preserves_shape`

`chronos-sandbox/tests/session_lifecycle.rs` (extend existing):

1. `test_session_start_load_via_v2`
2. `test_capabilities_static_only`
3. `test_capabilities_dynamic_only`

`chronos-sandbox/tests/program_scenarios.rs` (cross-cutting):

1. `test_capabilities_full_session_lifecycle` — start →
   probe_drain → capabilities{dynamic} → session_stop →
   capabilities{dynamic tail_sealed=true}

`chronos-sandbox/tests/analytics_tools.rs` (cross-cutting):

1. `test_session_start_load_then_events_read`

Smoke subset chosen: `probe_lifecycle.rs` +
`session_lifecycle.rs` + `program_scenarios.rs` = 3 suites. m7-04
does not touch diff/hypothesis/observe directly
(`session_stop{drain_subscriptions=true}` is a thin pass-through to
`observe{verb=list}` already exercised by m7-02 smoke), so those
suites are not required for T4-smoke.

---

## Risks and known limitations

* **`session_start{action=attach}` is a stub.** Returns
  `ServiceError::Unsupported("attach (m7+)")`. Wire shape is
  documented so callers can compose for m7+ without a breaking
  change. See scoping doc §Risks.
* **`seal_tail` is metadata-only.** No separate "tail log" file
  format; `tail_sealed` lives on `SessionMetadata` and `sealed_at`
  carries the wall-clock timestamp. Adequate for the v2 spec
  semantic ("gracefully end producers and seal tail"); richer
  per-event tail markers are m7+.
* **`drain_subscriptions` is destructive.** Matches the m7-02
  default `retention=Drained` semantics. Agents that want to
  preserve fired events must call `observe{verb=list}` first.
* **`capabilities` is read-only and best-effort.** No mutations;
  no caching; each call re-reads metadata + bus state. Adequate
  for an introspection surface; cached capabilities are m7+.
* **No `capabilities` MCP-suite precedent.** First read-only
  introspection tool in the v2 surface. Shape borrows from
  `observe` (m7-02) and `events_read` (m7-01).
* **No v1 `capabilities` shim.** Net-new; no v1 analogue exists.
  No deprecation migration window; justified per scoping §Risks.
* **`SessionMetadata` schema bump.** v2 → v3 additive change;
  `#[serde(default)]` ensures old files load. Unit-tested
  (round-trip v2 metadata → v3 reader → preserve
  `tail_sealed=false, sealed_at=None`).
* **Sandbox smoke coverage.** Three suites chosen:
  `probe_lifecycle.rs`, `session_lifecycle.rs`,
  `program_scenarios.rs`. m7-04 may add a `capabilities_tools.rs`
  suite if any of the above suites is too narrow — defer the
  decision to the execute cycle.
* **v1 sunset drift.** No new v1 names added; existing
  `probe_start` + `probe_stop` retain the 2027-09-11 sunset.

---

## Cross-references

* `docs/milestones/m7-04-session-start-stop-capabilities-scoping.md`
  — scoping rationale (parent doc).
* `docs/milestones/m7-events-read-scoping.md` — M7 split +
  sequencing.
* `docs/milestones/m7-02-observability-merge.md` — closest
  dispatcher precedent for `SessionLifecycleService`
  (typed-input + tagged-enum discriminator + provenance).
* `docs/milestones/m7-03-session-compare-explain-merge.md` —
  closest dispatcher pattern for v1 shim routing
  (`probe_start` → `session_start{action=spawn}`).
* `docs/milestones/m6-close-report.md` §6 — origin of the three
  tools scoped here.
* `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
  lines 11–13 (the three tools) + line 28–42 (subscription model
  that `capabilities` reflects) + line 64–66 (agent ergonomics).

---

— Submitted 2026-09-11. Awaits m7-04 execute cycle kickoff.
