# m7-05 — `session_start` + `session_stop` + `capabilities` dispatcher merge

**Branch:** `feat/m7-05-session-lifecycle-dispatchers-merge`
**Cycle:** M7 (v2-spec sub-cycle), fifth deliverable
**Precedence:** `docs/milestones/m7-05-session-lifecycle-dispatchers-scoping.md` (parent doc); `docs/milestones/m7-04-session-start-stop-capabilities-merge.md` § "Revision note"; v2 spec `AGENT_API_V2.md` lines 11–13
**Status:** MERGED — 2026-09-11

## Why this cycle

This is the **executor** for the m7-05 scope (picking up the dispatcher + wrappers + shims that m7-04 deferred). It closes the loop on the three v2 tools (`session_start`, `session_stop`, `capabilities`) that m7-04's foundation-only cycle couldn't ship.

After m7-05 ships, the M7 backlog reduces to:
- **m7-06** — v1 sunset bookkeeping (2027-09-11) + M7 close.

## Scope (this cycle)

- Add `chronos-services::session_lifecycle` (~350 LoC, 20th service module).
- `ChronosSessionLifecycleService::start` (async fn; routes by `SessionStartAction { Spawn | Load | Attach }`).
- `ChronosSessionLifecycleService::stop` (async fn; awaits `ProbeService::stop` + drain chain).
- `ChronosSessionLifecycleService::capabilities` (sync; store read).
- `SessionLifecycleContext<'a>` carries `&ProbeContext<'a>`, `&ObserveContext<'a>`, `&SessionStore`.
- 9–11 unit tests (see Test plan).
- Three v2 MCP tool wrappers + three DTOs.
- v1 `probe_start` → `session_start{action=spawn}` shim conversion.
- v1 `probe_stop` → `session_stop` shim conversion (with `seal_tail=true, drain_subscriptions=true` defaults).
- `lib.rs` module index (20→21).
- T4 sandbox smoke (`probe_lifecycle.rs` + `session_lifecycle.rs` + `program_scenarios.rs`).

## Architecture

### Dispatcher signature

```rust
pub struct SessionLifecycleContext<'a> {
    pub store: &'a chronos_store::SessionStore,
    pub probe: &'a ProbeContext<'a>,
    pub observe: &'a ObserveContext<'a>,
}

impl ChronosSessionLifecycleService {
    pub async fn start(
        ctx: &SessionLifecycleContext<'_>,
        input: SessionStartInput,
    ) -> Result<SessionStartOutput, ServiceError>;

    pub async fn stop(
        ctx: &SessionLifecycleContext<'_>,
        input: SessionStopInput,
    ) -> Result<SessionStopOutput, ServiceError>;

    pub fn capabilities(
        ctx: &SessionLifecycleContext<'_>,
        input: CapabilitiesInput,
    ) -> Result<CapabilitiesOutput, ServiceError>;
}
```

### `start` dispatch table

| `input.action` | Path | Async? | Notes |
|---|---|---|---|
| `Spawn`  | `ProbeService::start(ctx.probe, ProbeStartInput{ program, args, trace_syscalls, cwd, bus_capacity, track_function_frames })` | yes | returns session_id + capability snapshot |
| `Load`   | `store.load_session(id)` → `(meta, events)` | no | snapshot built from metadata |
| `Attach` | `ServiceError::Unsupported("attach (m7+)")` if `pid.is_some()`, else `InvalidInput` | no | domain-layer attach API is m7+ |

### `stop` dispatch algorithm

```
async fn stop(ctx, input):
    1. if input.drain_subscriptions:
         ChronosObserveService::observe(ctx.observe,
             ObserveInput { verb: ObserveVerb::List,
                            scope: Some(ObserveScope::Session(input.session_id.clone())),
                            cursor: None, ..Default::default() })
         drained_subscriptions = true
    2. let stop_result = ProbeService::stop(ctx.probe, &input.session_id)
    3. let sealed_at = if input.seal_tail:
         let (mut meta, events) = ctx.store.load_session(&input.session_id)?
         meta.tail_sealed = true
         meta.sealed_at = Some(now_ms())
         ctx.store.save_session(meta, &events)?
         Some(now_ms())
       else:
         None
    4. SessionStopOutput { ...capability_snapshot with tail_sealed=sealed_at.is_some() }
```

### `capabilities` dispatch algorithm

```
fn capabilities(ctx, input):
    1. validate: at least one of `target` or `session_id` is set
    2. let static_caps = input.target.map(|t| Self::static_capabilities(t))
    3. let dynamic_caps = if let Some(sid) = input.session_id:
         let (meta, _events) = ctx.store.load_session(sid)?
         Some(Self::dynamic_capabilities(&meta))
       else:
         None
    4. CapabilitiesOutput { static_capabilities, dynamic_capabilities, provenance }
```

### MCP wrappers

```rust
#[derive(Deserialize, JsonSchema)]
pub struct SessionStartParams {
    pub action: String,                  // "spawn" | "load" | "attach"
    pub spawn_fields: Option<SessionStartSpawnFieldsDto>,
    pub session_id: Option<String>,
    pub pid: Option<u32>,
    pub path: Option<String>,
}

// parse_session_start_params(params) -> SessionStartInput  // helper

#[tool(name = "session_start", ...)]
async fn session_start(&self, params: SessionStartParams) -> Result<CallToolResult, McpError> { ... }
```

Similarly for `SessionStopParams` + `session_stop` tool + `CapabilitiesParams` + `capabilities` tool.

### v1 shim conversions

`probe_start(params: ProbeStartParams)`:

```rust
let v2_input = SessionStartInput {
    action: SessionStartAction::Spawn,
    spawn_fields: Some(SessionStartSpawnFields::from(params)),
    session_id: None,
    pid: None,
    path: None,
};
ChronosSessionLifecycleService::start(&ctx, v2_input).await
```

`probe_stop(params: ProbeStopParams)`:

```rust
let v2_input = SessionStopInput {
    session_id: params.session_id,
    seal_tail: true,
    drain_subscriptions: true,
};
ChronosSessionLifecycleService::stop(&ctx, v2_input).await
```

## Test plan (planned)

`crates/chronos-services/src/session_lifecycle.rs`:

1. `start_spawn_requires_spawn_fields`
2. `start_load_returns_metadata_snapshot`
3. `start_load_session_not_found`
4. `start_attach_without_pid_returns_invalid_input`
5. `start_attach_with_pid_returns_unsupported`
6. `capabilities_target_only_returns_static_only`
7. `capabilities_session_only_returns_dynamic_only`
8. `capabilities_target_and_session_returns_both`
9. `capabilities_neither_set_returns_invalid_input`
10. `capabilities_session_not_found`
11. `provenance_engine_version_is_baked`

The async paths (spawn, stop) are exercised only at the sandbox layer (T4).

`crates/chronos-mcp/tests/session_lifecycle_tools.rs` (NEW):

1. `test_session_start_spawn_via_v2`
2. `test_session_start_load_via_v2`
3. `test_session_start_attach_returns_unsupported`
4. `test_session_stop_default_seal_tail_true_drain_subscriptions_true`
5. `test_session_stop_seal_tail_false_does_not_seal`
6. `test_capabilities_target_only_returns_static_capabilities`
7. `test_capabilities_session_only_returns_dynamic_capabilities`
8. `test_probe_start_v1_shim_routes_to_session_start_spawn`
9. `test_probe_stop_v1_shim_routes_to_session_stop_with_defaults`

`chronos-sandbox/tests/probe_lifecycle.rs` (extend existing):

1. `test_session_start_via_v2_then_session_stop_via_v2`
2. `test_probe_start_v1_shim_still_works`
3. `test_probe_stop_v1_shim_still_works`

`chronos-sandbox/tests/session_lifecycle.rs` (extend existing):

1. `test_capabilities_static_then_dynamic_through_full_lifecycle`

`chronos-sandbox/tests/program_scenarios.rs` (cross-cutting):

1. `test_session_lifecycle_in_full_session`

Smoke subset: `probe_lifecycle.rs` + `session_lifecycle.rs` + `program_scenarios.rs` = 3 suites.

## Risks and known limitations

- **`session_start{action=attach}` is a stub.** Returns `ServiceError::Unsupported("attach (m7+)")`. Wire shape is documented so callers can compose for m7+ without a breaking change.
- **`seal_tail` is a load+mutate+save round-trip.** Adequate for the v2 spec semantic; a dedicated `store::update_session_metadata(meta)` is m7+ optimisation.
- **`drain_subscriptions` chains to `observe{verb=list}` (destructive).** Matches the m7-02 default `retention=Drained` semantics. Agents that want to preserve fired events must call `observe{verb=list}` first.
- **`capabilities` is read-only and best-effort.** No mutations; no caching; each call re-reads metadata + bus state. Cached capabilities are m7+.
- **No `capabilities` MCP-suite precedent.** First read-only introspection tool in the v2 surface.
- **No v1 `capabilities` shim.** Net-new; no v1 analogue exists.
- **v1 sunset drift.** No new v1 names added; existing `probe_start` + `probe_stop` retain the 2027-09-11 sunset.
- **Dispatcher stop path is sandbox-tested only.** No in-process unit test for the stop path (real EventBus + TripwireManager needed); T4 smoke is the signal.

## Cross-references

* `docs/milestones/m7-05-session-lifecycle-dispatchers-scoping.md` — parent doc (scope rationale).
* `docs/milestones/m7-04-session-start-stop-capabilities-scoping.md` — design rationale (m7-04).
* `docs/milestones/m7-04-session-start-stop-capabilities-merge.md` § "Revision note" — scope split rationale (4 reasons).
* `docs/milestones/m7-events-read-scoping.md` — M7 split + sequencing.
* `docs/milestones/m7-02-observability-merge.md` — closest dispatcher precedent (typed-input + tagged-enum discriminator + provenance).
* `docs/milestones/m7-03-session-compare-explain-merge.md` — closest dispatcher pattern for v1 shim routing.
* `sddk/changes/m7-04-session-start-stop-capabilities-merge/apply-checkpoint.json` `deferred_to_m7_05` — explicit deferred-scope list.
* `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md` lines 11–13 (the three tools) + lines 28–42 (subscription model that `capabilities` reflects) + lines 64–66 (agent ergonomics).

---

— Submitted 2026-09-11. Awaits m7-05 execute cycle kickoff.
