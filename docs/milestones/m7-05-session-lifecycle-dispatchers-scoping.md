# m7-05 — `session_start` + `session_stop` + `capabilities` dispatcher scoping

**Branch:** `feat/m7-05-session-lifecycle-dispatchers-scoping`
**Cycle:** M7 (v2-spec sub-cycle), fifth deliverable (m7-04 was foundation-only; see `docs/milestones/m7-04-session-start-stop-capabilities-merge.md` § "Revision note")
**Precedence:** `docs/milestones/m7-04-session-start-stop-capabilities-scoping.md` (design rationale); `docs/milestones/m7-04-session-start-stop-capabilities-merge.md` § "Revision note" (scope split); v2 spec `AGENT_API_V2.md` lines 11–13
**Status:** SCOPING — 2026-09-11

## Why this cycle

m7-04 shipped the **foundation layer** (DTOs + `SessionMetadata` extension + docs) but explicitly deferred the dispatcher + MCP wrappers + v1 shims because those proved significantly more complex than the m7-01..m7-03 dispatchers. This cycle (m7-05) picks them up.

The four architectural reasons captured in `m7-04-session-start-stop-capabilities-merge.md` § "Revision note":

1. **Async path.** `session_start{action=spawn}` calls `ProbeService::start(ctx, input)` which is `async fn`. All m7-01..m7-03 dispatchers are synchronous. This is the first async dispatcher in `chronos-services`.
2. **Cross-service plumbing.** `session_stop{drain_subscriptions=true}` chains to `observe{verb=list}`, which needs an `ObserveContext { tripwire_manager, probe, uprobe_counter }`. `ObserveContext` lives on the MCP server (`server.rs:127,131,3137`), not on the session_lifecycle service. The dispatcher must accept an `&ObserveContext` as a borrowed argument.
3. **Store update path.** `session_stop{seal_tail=true}` updates `SessionMetadata.tail_sealed` + `sealed_at`. The store API exposes `save_session(meta, events)` (overwrite) and `load_session(id) -> (meta, events)`. Sealing requires load + mutate + save — a full CAS round-trip. Adequate for v2; a dedicated `update_session_metadata(meta)` is m7+ optimisation.
4. **`ProbeContext` testability.** `ProbeContext<'a>` carries `live_probes: &Mutex<...>` (std), `engines: &TokioMutex<...>`, `session_languages: &Arc<TokioMutex<...>>`, `tripwire_manager: &Arc<TripwireManager>`, `active_session: &TokioMutex<...>`. Constructing all five in a unit test requires real `TripwireManager::new()` and `Arc::new(TokioMutex::new(HashMap::new()))` constructions. Workable — no need for `Box::leak` statics or unsafe `mem::zeroed` (those were the antipatterns from the m7-04 aborted attempt).

## Scope (this cycle)

- Add `chronos-services::session_lifecycle` module (~350 LoC, 20th service module).
- Add `ChronosSessionLifecycleService` with three entry points:
  - `start(ctx: &SessionLifecycleContext<'_>, input: SessionStartInput) -> impl Future<Output = Result<SessionStartOutput, ServiceError>>` (async; spawn path awaits `ProbeService::start`).
  - `stop(ctx: &SessionLifecycleContext<'_>, input: SessionStopInput) -> impl Future<Output = Result<SessionStopOutput, ServiceError>>` (async; stop awaits `ProbeService::stop` + drain chain).
  - `capabilities(ctx: &SessionLifecycleContext<'_>, input: CapabilitiesInput) -> Result<CapabilitiesOutput, ServiceError>` (sync; pure store read).
- Extend `SessionLifecycleContext<'a>` to carry `&ObserveContext<'a>` (in addition to `&ProbeContext<'a>` and `&SessionStore`) so the drain step can call `ChronosObserveService::observe`.
- Add 9–11 unit tests (see Test plan below).
- Add three v2 MCP tool wrappers: `session_start`, `session_stop`, `capabilities` in `crates/chronos-mcp/src/server.rs`.
- Add `SessionStartParams` / `SessionStopParams` / `CapabilitiesParams` DTOs (mirroring the m7-02/m7-03 precedent).
- Convert `probe_start` v1 to a deprecated shim that routes to `session_start{action=spawn}` (preserve shape 1:1).
- Convert `probe_stop` v1 to a deprecated shim that routes to `session_stop` with `seal_tail=true, drain_subscriptions=true` (defaults).
- Wire `lib.rs` module index (20→21 after this cycle).
- T4 sandbox smoke: `probe_lifecycle.rs` + `session_lifecycle.rs` + `program_scenarios.rs`.

## Concrete API surface (no inventions)

`SessionLifecycleContext<'a>`:

```rust
pub struct SessionLifecycleContext<'a> {
    pub store: &'a chronos_store::SessionStore,
    pub probe: &'a ProbeContext<'a>,
    pub observe: &'a ObserveContext<'a>,
}
```

`start` dispatcher (async; first async dispatcher in the services crate):

```rust
impl ChronosSessionLifecycleService {
    pub async fn start(
        ctx: &SessionLifecycleContext<'_>,
        input: SessionStartInput,
    ) -> Result<SessionStartOutput, ServiceError> { ... }
}
```

`stop` dispatcher (async; drain step calls observe):

```rust
pub async fn stop(
    ctx: &SessionLifecycleContext<'_>,
    input: SessionStopInput,
) -> Result<SessionStopOutput, ServiceError> {
    // 1. if drain_subscriptions: ChronosObserveService::observe(
    //      ctx.observe,
    //      ObserveInput { verb: ObserveVerb::List, scope: Some(ObserveScope::Session(input.session_id.clone())), ... },
    //    )?;
    // 2. ProbeService::stop(ctx.probe, &input.session_id)?;
    // 3. if seal_tail: store.load_session → mutate meta → store.save_session
}
```

`capabilities` dispatcher (sync; store read only):

```rust
pub fn capabilities(
    ctx: &SessionLifecycleContext<'_>,
    input: CapabilitiesInput,
) -> Result<CapabilitiesOutput, ServiceError> { ... }
```

`session_start{action=attach}` remains a `ServiceError::Unsupported("attach (m7+)")` stub — the domain-layer attach API does not exist yet.

## Test plan (planned)

`crates/chronos-services/src/session_lifecycle.rs`:

1. `start_spawn_requires_spawn_fields` — validation guard, no real probe.
2. `start_load_returns_metadata_snapshot` — store-only round-trip.
3. `start_load_session_not_found` — store-only error path.
4. `start_attach_without_pid_returns_invalid_input` — validation guard.
5. `start_attach_with_pid_returns_unsupported` — semantic stub.
6. `capabilities_target_only_returns_static_only` — no session touch.
7. `capabilities_session_only_returns_dynamic_only` — store-only.
8. `capabilities_target_and_session_returns_both` — combined.
9. `capabilities_neither_set_returns_invalid_input` — validation guard.
10. `capabilities_session_not_found` — store-only error path.
11. `provenance_engine_version_is_baked` — cheap invariant.

The stop path and the spawn-runtime path are **not unit-tested** in this cycle — they need a real `ProbeContext` (TripwireManager + Arc<TokioMutex<...>> + TokioMutex<...>) with a live EventBus. Those are exercised by the sandbox smoke (T4 below). This matches the m7-02 precedent where `observe{verb=create}` is also tested only at the sandbox layer.

`crates/chronos-mcp/tests/session_lifecycle_tools.rs` (NEW):

1. `test_session_start_spawn_via_v2` — happy path against a real `/bin/echo` target.
2. `test_session_start_load_via_v2` — save session → load returns metadata.
3. `test_session_start_attach_returns_unsupported` — stub surface.
4. `test_session_stop_default_seal_tail_true_drain_subscriptions_true` — verify sealed_at populated + drain ok.
5. `test_session_stop_seal_tail_false_does_not_seal` — verify metadata unchanged.
6. `test_capabilities_target_only_returns_static_capabilities`.
7. `test_capabilities_session_only_returns_dynamic_capabilities`.
8. `test_probe_start_v1_shim_routes_to_session_start_spawn` — backward-compat.
9. `test_probe_stop_v1_shim_routes_to_session_stop_with_defaults`.

`chronos-sandbox/tests/probe_lifecycle.rs` (extend existing):

1. `test_session_start_via_v2_then_session_stop_via_v2` — happy-path round-trip on real `/bin/echo`.
2. `test_probe_start_v1_shim_still_works` — backward-compat smoke.
3. `test_probe_stop_v1_shim_still_works` — backward-compat smoke.

`chronos-sandbox/tests/session_lifecycle.rs` (extend existing):

1. `test_capabilities_static_then_dynamic_through_full_lifecycle` — start → drain → capabilities{dynamic} → stop → capabilities{tail_sealed=true}.

`chronos-sandbox/tests/program_scenarios.rs` (cross-cutting):

1. `test_session_lifecycle_in_full_session` — full path exercising the v2 dispatcher on a real binary.

Smoke subset chosen: `probe_lifecycle.rs` + `session_lifecycle.rs` + `program_scenarios.rs` = 3 suites. m7-05 does not touch diff/hypothesis directly so those suites are not required for T4-smoke.

## Risks and known limitations

- **`session_start{action=attach}` is a stub.** Returns `ServiceError::Unsupported("attach (m7+)")`. Wire shape is documented so callers can compose for m7+ without a breaking change.
- **`seal_tail` is a load+mutate+save round-trip.** Adequate for the v2 spec semantic; a dedicated `store::update_session_metadata(meta)` is m7+ optimisation.
- **`drain_subscriptions` chains to `observe{verb=list}` (destructive).** Matches the m7-02 default `retention=Drained` semantics. Agents that want to preserve fired events must call `observe{verb=list}` first.
- **`capabilities` is read-only and best-effort.** No mutations; no caching; each call re-reads metadata + bus state. Cached capabilities are m7+.
- **No `capabilities` MCP-suite precedent.** First read-only introspection tool in the v2 surface. Shape borrows from `observe` (m7-02) and `events_read` (m7-01).
- **No v1 `capabilities` shim.** Net-new; no v1 analogue exists.
- **`SessionMetadata` schema bump.** v2 → v3 additive change; `#[serde(default)]` ensures old files load. Round-trip tested by m7-04.
- **Dispatcher stop path is sandbox-only.** No in-process unit test for the stop path; relies on T4 smoke (3 suites) for signal. Justified: the stop path mutates real EventBus + TripwireManager state, which unit tests cannot mock cleanly without leaking statics.
- **v1 sunset drift.** No new v1 names added; existing `probe_start` + `probe_stop` retain the 2027-09-11 sunset.

## Cross-references

* `docs/milestones/m7-04-session-start-stop-capabilities-scoping.md` — design rationale (parent doc, m7-04).
* `docs/milestones/m7-04-session-start-stop-capabilities-merge.md` § "Revision note" — scope split rationale (4 reasons).
* `docs/milestones/m7-events-read-scoping.md` — M7 split + sequencing.
* `docs/milestones/m7-02-observability-merge.md` — closest dispatcher precedent for `SessionLifecycleService` (typed-input + tagged-enum discriminator + provenance).
* `docs/milestones/m7-03-session-compare-explain-merge.md` — closest dispatcher pattern for v1 shim routing.
* `sddk/changes/m7-04-session-start-stop-capabilities-merge/apply-checkpoint.json` `deferred_to_m7_05` — explicit deferred-scope list.
* `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md` lines 11–13 (the three tools) + lines 28–42 (subscription model that `capabilities` reflects) + lines 64–66 (agent ergonomics).

---

— Submitted 2026-09-11. Awaits m7-05 execute cycle kickoff.
