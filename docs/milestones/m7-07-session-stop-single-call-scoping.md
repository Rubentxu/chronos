# m7-07 — `session_stop` single-call path scoping

**Branch:** `feat/m7-07-session-stop-single-call-scoping`
**Cycle:** M7 (v2-spec sub-cycle), seventh deliverable (m7-06 was M7-close; see `docs/milestones/m7-06-session-lifecycle-sandbox-smoke-merge.md`)
**Precedence:** `docs/milestones/m7-06-session-lifecycle-sandbox-smoke-merge.md` § "followup"; v2 spec `AGENT_API_V2.md` lines 12 (`session_stop`)
**Status:** SCOPING — 2026-09-11

## Why this cycle

m7-06 fixed three m7-05 architectural bugs **in-cycle** with a pragmatic workaround: the `session_stop` MCP wrapper now pre-stops the probe, calls `save_session`, calls `build_and_store_engine`, then invokes the dispatcher for `drain_subscriptions` + `seal_tail`. The dispatcher also calls `ProbeService::stop` internally — that internal call returns `ProbeNotFound` (probe already gone) and the dispatcher now falls back to `load_session` to synthesise the snapshot fields from persisted metadata.

That works, but it is a three-step dance inside the wrapper and the dispatcher's `StopSnapshot` fallback is structurally a workaround. The honest fix is to make `session_stop` single-call: **the dispatcher does everything**.

Two strategies were considered:

1. **A.** Add `events + language + target` to `SessionStopOutput` so the dispatcher returns events through its DTO, and the wrapper calls `save_session` + `build_and_store_engine` directly on the returned events (without calling `ProbeService::stop` itself).
2. **B.** Add a new service-level `ProbeService::finalize(&ctx, session_id) -> ProbeFinalizeOutcome` that bundles `stop + save + build_engine` in one call, and split `session_stop` into two phases: a "live stop" path (calls `finalize`) and an "already-persisted" path (load-only metadata housekeeping).

**Strategy A is the chosen approach.** Rationale:

- `events` can be many MB. Returning them through a JSON DTO bloats the wire. Mitigation: the events field is **only populated in a non-wire helper** (`SessionStopPersistence` enum returned by an internal service-level call), not in `SessionStopOutput`. The wrapper calls the service-level helper, not the dispatcher output.
- Strategy A was the m7-05 **intent** — the m7-06 apply-checkpoint followup array says "Add events + language to SessionStopOutput so session_stop wrapper doesn't need pre_stop (single-call path)" — and was deferred because m7-05's `session_stop_does_not_double_call` was out of scope. m7-07 picks it up.
- Strategy B forces every caller to thread the new `ProbeService::finalize` API; it does not fit the m7-05 dispatcher pattern that other lifecycle consumers (e.g. `v1 probe_stop`, future `stop_watcher`) follow.

## Scope (this cycle)

- Refactor `ChronosSessionLifecycleService::stop` to call `ProbeService::stop` **once** internally and return the events through a new **internal** helper `stop_with_persistence(ctx, input) -> Result<SessionStopPersistence, ServiceError>` that the MCP wrapper calls directly.
- Add `SessionStopPersistence { events: Vec<TraceEvent>, language: Language, target: String, total_events: u64, duration_ms: u64, ebpf_detached: bool }` (service-internal enum, **not** a DTO; not part of `output.rs`).
- The public `session_stop` MCP wrapper calls `stop_with_persistence` → `save_session(meta, &persistence.events)` → `build_and_store_engine(sid, persistence.events, persistence.language)` → returns the existing `SessionStopOutput` (no API change).
- The dispatcher's drain + seal + snapshot synthesis stays as-is; the only change is that `ProbeService::stop` is called exactly once (inside the dispatcher) and the events are threaded back to the wrapper instead of being thrown away.
- Drop the m7-06 `StopSnapshot` `ProbeNotFound → load_session` fallback (it was a workaround; replaced by the single-call path).
- Drop the `mark_sealed_returns_false_when_session_not_in_store` test (the live-only edge case no longer exists; the dispatcher calls `save_session` BEFORE `mark_sealed`, so `mark_sealed.load_session` always succeeds).
- Replace the m7-06 `mark_sealed` returning `Ok(bool)` with `Result<(), ServiceError>` (original m7-05 signature).
- Validate the m7-06 `capabilities_with_context` live-fallback (kept; no change).

## Concrete API surface (no inventions)

```rust
// New service-internal enum (does NOT live in output.rs).
//
// Lives in crates/chronos-services/src/session_lifecycle.rs alongside
// the dispatcher. NOT serialised to JSON.
pub enum SessionStopPersistence {
    Stopped {
        session_id: String,
        events: Vec<chronos_domain::TraceEvent>,
        language: chronos_domain::Language,
        target: String,
        total_events: u64,
        duration_ms: u64,
        ebpf_detached: bool,
        sealed_at: Option<u64>,
    },
    AlreadyStopped { session_id: String },
}

impl ChronosSessionLifecycleService {
    pub async fn stop_with_persistence(
        ctx: &SessionLifecycleContext<'_>,
        input: SessionStopInput,
    ) -> Result<SessionStopPersistence, ServiceError> { ... }

    /// Convenience for callers that only need the metadata-layer output
    /// (no events threaded back). Used by tests + future in-process
    /// consumers. The MCP wrapper calls `stop_with_persistence` directly.
    pub async fn stop(
        ctx: &SessionLifecycleContext<'_>,
        input: SessionStopInput,
    ) -> Result<SessionStopOutput, ServiceError> { ... }
}
```

The MCP wrapper:

```rust
let persistence = ChronosSessionLifecycleService::stop_with_persistence(
    &lifecycle_ctx, v2_input,
).await?;
match persistence {
    SessionStopPersistence::Stopped { session_id, events, language, .. } => {
        let _ = self.store.save_session(meta, &events);
        self.build_and_store_engine(&session_id, events, language).await;
        let out = /* synthesise the existing SessionStopOutput from persistence */;
        Ok(...)
    }
    SessionStopPersistence::AlreadyStopped { session_id } => {
        // load_session and synthesise the output
    }
}
```

## Test plan

`crates/chronos-services/src/session_lifecycle.rs`:

1. `mark_sealed_updates_metadata_and_persists` (kept from m7-06; `mark_sealed` signature reverts to `Result<(), ServiceError>`).
2. `stop_with_persistence_probes_once_and_threads_events` (new — synthesize the stop path against a stub ProbeContext and assert `ProbeService::stop` is called exactly once via a `Mutex<count>`).
3. `stop_with_persistence_already_stopped_returns_already_stopped_variant` (new).
4. `stop_with_persistence_drain_subscriptions_chains_observe` (new — verify the dispatcher still chains to `observe{verb=list}` before the probe stop).
5. `stop_with_persistence_seal_tail_true_persists_sealed_at` (new).

`crates/chronos-mcp/tests/session_lifecycle_tools.rs` (extend m7-05 stub or new):

1. `test_session_stop_does_not_double_call_probe_service_stop` (NEW — assert the wrapper invokes `ProbeService::stop` exactly once; requires a probe service that has been wrapped behind a mock or counter; if the mock is too heavy, defer to T4 smoke).
2. `test_session_stop_persists_session_to_redb_for_session_load` (NEW — after session_stop, session_start_load should find the session without going through the wrapper a second time; this works today thanks to m7-06 save_session — m7-07 simplifies but does not change the contract).

`chronos-sandbox/tests/probe_lifecycle.rs` (extend):

1. `test_session_stop_via_v2_persists_queryable_session_after_call` (NEW — after session_stop, the MCP server should be able to query_events through QueryEngine without rebuilding).

Sandbox smoke subset chosen (T4): `probe_lifecycle.rs` + `session_lifecycle.rs` + `program_scenarios.rs` = 3 suites (same as m7-06; this cycle touches stop plumbing but not new tool surface).

## Risks and known limitations

- **Sequence change.** After m7-07 the wrapper calls `save_session` (redb write) BEFORE `build_and_store_engine` (in-memory write). If `build_and_store_engine` panics, the redb write survives but the engine is missing. m7-07 keeps the m7-06 ordering `save_session` → `build_and_store_engine` to avoid introducing a new invariant on the failure boundary.
- **`SessionStopPersistence` is not a DTO.** It is a service-internal enum; we deliberately do NOT add it to `output.rs` because it carries `Vec<TraceEvent>` which would otherwise need serde + skip_on_serializing_if handling and would bloat the JSON shape for callers that don't need events.
- **`SessionStopOutput` JSON shape unchanged.** External callers (agents, MCP clients) see no API change.
- **No new `ServiceError` variants.** `ProbeNotFound` + `InvalidInput` + `SaveFailed` + `LoadFailed` already cover all the new stop error paths.
- **`Unsupported(String)` still used for any m7+ stubs.** No new stubs added or removed.
- **`detach_all` semantics unchanged.** eBPF cleanup happens inside `ProbeService::stop` (m7-05 behaviour preserved).

## Architectural decisions

- **Single `ProbeService::stop` call.** The dispatcher is the only place that calls it; the wrapper never calls it directly. Eliminates the m7-06 "probe-already-gone" soft-handling and the `load_session` fallback.
- **`SessionStopPersistence` is the new "pivot point."** The wrapper does the persistence side-effects (save_session + build_and_store_engine) using the events returned by the dispatcher. Cleaner ownership: the dispatcher returns data, the wrapper performs I/O.
- **`AlreadyStopped` variant.** Allows the v2 dispatcher to support a "stop was already called" scenario where the events are gone (engine already built, redb already has them). Same code path as the m7-06 "load_session fallback" but expressed as a normal outcome variant instead of an error path. No return-Error semantics changed.
- **Existing `probe_stop` v1 path unchanged.** Still calls `ProbeService::stop` + `build_and_store_engine` directly (does NOT save to redb — this is a v1 historical choice and not in scope for m7-07). v1 m7+ deprecation bookkeeping untouched.
- **Existing `session_start{action=load}` unchanged.** Reads from `SessionStore`. Already broken in m7-05/06 if the session was stop-via-v1 (which never saved to redb); broken in m7-06-fix-and-onward if the session was stop-via-v2 (which now saves via the wrapper pre-stop). m7-07 makes the v2 stop path the only one that saves to redb; v1 stop remains "in-memory only" until the 2027-09-11 sunset.

## Cross-references

* `docs/milestones/m7-06-session-lifecycle-sandbox-smoke-merge.md` "followups[0]" — explicit deferred-to-m7-07 list.
* `docs/milestones/m7-05-session-lifecycle-dispatchers-merge.md` § "architectural_decisions.probe_stop_NOT_shimmed" — pre-existing double-call prediction.
* `docs/milestones/m7-06-session-lifecycle-sandbox-smoke-merge.md` § "follow_up_in_cycle_fixes.bug_1_session_stop_double_call" — in-cycle workaround that this cycle replaces with a structural fix.
* `sddk/changes/m7-06-session-lifecycle-sandbox-smoke-merge/apply-checkpoint.json` `followups[0]` — the explicit m7-07 booking.
* `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md` lines 12 (session_stop tool spec).

---

— Submitted 2026-09-11. Awaits m7-07 execute cycle kickoff.
