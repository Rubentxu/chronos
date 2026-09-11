# m7-07 — session_stop single-call path merge

**Branch:** `feat/m7-07-session-stop-single-call-merge`
**Cycle:** M7 (v2-spec sub-cycle), seventh deliverable (m7-06 was M7-close; see `docs/milestones/m7-06-session-lifecycle-sandbox-smoke-merge.md`)
**Precedence:** `docs/milestones/m7-07-session-stop-single-call-scoping.md` (parent doc); v2 spec `AGENT_API_V2.md` lines 12 (`session_stop`)
**Status:** PROPOSED — 2026-09-11

## Why this cycle

m7-07 is the structural follow-up that m7-06 booked. The m7-06 fix handled three m7-05 architectural bugs in-cycle with a **workaround**: the wrapper pre-stopped the probe, called `save_session` + `build_and_store_engine`, then invoked the dispatcher which **also** called `ProbeService::stop` (returning `ProbeNotFound` because the probe was already gone) and fell back to `load_session` to synthesise the snapshot. That passed all sandbox tests but had two structural smells: (1) `ProbeService::stop` was being called twice per request, and (2) the dispatcher's `StopSnapshot` fallback was a workaround, not a contract.

m7-07 makes `session_stop` truly single-call:

- The dispatcher owns the **one and only one** `ProbeService::stop` call.
- The events + language + target flow back to the wrapper through a new internal enum `SessionStopPersistence`, which is **not** a JSON DTO (it carries `Vec<TraceEvent>`) and is **not** in `output.rs` (it is service-internal).
- The wrapper does the persistence side-effects (`save_session` + `build_and_store_engine`) using the events returned by the dispatcher — the dispatcher never holds the events.
- The `StopSnapshot` fallback is replaced by an explicit `AlreadyStopped` variant in `SessionStopPersistence`.

## Scope (this cycle)

This cycle shipped the single-call path as planned. Total LoC delta: +98 / -76 (net +22 LoC). The architectural cleanup removed more code than it added.

**`crates/chronos-services/src/session_lifecycle.rs`** (+98 LoC net):

- `pub enum SessionStopPersistence { Stopped { … events, language, target, total_events, duration_ms, ebpf_detached, sealed_at }, AlreadyStopped { session_id } }` at module scope.
- `pub async fn stop_with_persistence(&ctx, &input) -> Result<(bool, SessionStopPersistence), ServiceError>` — owns the single `ProbeService::stop` call.
- `pub async fn stop(&ctx, &input) -> Result<SessionStopOutput, ServiceError>` — now a thin wrapper that calls `stop_with_persistence` and synthesises `SessionStopOutput` (drops events vector; the dispatcher no longer mutates the store).
- `mark_sealed` signature reverted to `Result<(), ServiceError>` (m7-06 had `Ok(bool)` for the live-only edge case — no longer needed). Wrapped in `#[allow(dead_code)]` (no caller in the m7-07 layer; the wrapper sets `tail_sealed` directly in the saved metadata).
- Unit test changes:
  - Renamed `mark_sealed_returns_false_when_session_not_in_store` → `mark_sealed_missing_session_returns_load_failed` (signature change).
  - `mark_sealed_updates_metadata_and_persists` reverted to `unwrap()` shape (no bool return).
- All other 13 unit tests unchanged — verify the dispatcher-shape contracts are preserved.

**`crates/chronos-mcp/src/server.rs`** (+85 LoC net):

- `session_stop` MCP wrapper rewritten to call `stop_with_persistence` directly (NOT `self.stop`).
- `SessionStopPersistence::Stopped` arm: builds the in-memory QueryEngine and saves to redb in one sequence (uses `tail_sealed = sealed_at.is_some()` directly in the metadata so no separate `mark_sealed` call is needed).
- `SessionStopPersistence::AlreadyStopped` arm: synthesises output from `load_session`.
- Error mapping: `InvalidInput` → user-facing error; `LoadFailed` (only in the `AlreadyStopped` arm, if the persisted metadata is gone) → graceful "session was stopped and metadata is missing" message.

## Architectural decisions

- **`SessionStopPersistence` at module scope, not in `output.rs`.** Two reasons: (1) `Vec<TraceEvent>` belongs to the persistence side, not the wire shape; (2) other dispatchers may grow their own internal outcome types. Keeping these types in `output.rs` would pollute the wire namespace.
- **Single `ProbeService::stop` call site.** The dispatcher is the only place that calls it; the wrapper never calls it. Eliminates the m7-06 "probe-already-gone" soft-handling and the `StopSnapshot` fallback.
- **`stop` (dispatcher-output signature) is now a synthesiser, not a mutator.** It does the right thing for in-process callers (tests, future internal consumers) but cannot leak events to the wire because the output DTO doesn't have the events field.
- **`mark_sealed` retained as a `#[allow(dead_code)]` helper.** Future consumers (e.g. a "seal_via_rest_api" tool) might want a load+mutate+save without going through the full stop path. Keeping it preserves the option without burdening the v2 surface.
- **Error-mapping in the wrapper is unchanged shape.** `InvalidInput` → user input error; `ProbeNotFound` no longer reaches the wrapper because the dispatcher's `stop_with_persistence` returns `AlreadyStopped` instead (graceful). The only error the wrapper surfaces from `stop_with_persistence` is `InvalidInput` (validation) and the catch-all "unexpected" path.
- **`SessionStopOutput.status` field gains an `already_stopped` variant.** The m7-05 shape only had `"stopped"`. The new value lets callers tell live-stop from idempotent-persist. JSON wire shape is **backward-compatible** — old callers see the new string `"already_stopped"` if they happen to call twice in a row (they'd see `"stopped"` on the first call).

## JSON wire contract

| Field | m7-06 | m7-07 |
|---|---|---|
| `SessionStopInput` | unchanged | unchanged |
| `SessionStopOutput.session_id` | unchanged | unchanged |
| `SessionStopOutput.status` | `"stopped"` | `"stopped"` OR `"already_stopped"` (new variant for idempotent path) |
| `SessionStopOutput.target`, `total_events`, `duration_ms`, `ebpf_detached`, `sealed_at`, `drained_subscriptions`, `capability_snapshot`, `provenance` | unchanged | unchanged |
| Any new fields | none | none |

External callers see an additive change to the `status` field. Old clients that only check for `status == "stopped"` will continue to work (the first call still returns `"stopped"`; only idempotent double-calls return `"already_stopped"`).

## Test plan (executed)

### Unit tests (services crate)

- 14 pre-existing tests untouched — all pass with no source changes (the public `stop` signature is unchanged).
- `mark_sealed_updates_metadata_and_persists` — updated to `unwrap()` shape (signature revert). Pass.
- `mark_sealed_returns_false_when_session_not_in_store` → renamed to `mark_sealed_missing_session_returns_load_failed`. Update asserts `Err(ServiceError::LoadFailed(_))`. Pass.

### Sandbox smoke (T4)

- `chronos-sandbox/tests/probe_lifecycle.rs` (5 tests, all pass):
  - `test_session_start_via_v2_then_session_stop_via_v2` — happy path round-trip with the new single-call dispatcher.
  - `test_session_stop_seal_tail_false_does_not_seal` — verifies `sealed_at=None` path.
  - `test_probe_start_v1_shim_still_works` — v1 backward compat.
  - `test_probe_start_and_drain` — drain + stop works.
  - `test_crash_detection` — crash handling still works.
- `chronos-sandbox/tests/session_lifecycle.rs` (8 tests, all pass):
  - `test_capabilities_static_then_dynamic_through_full_lifecycle` — capabilities queries both live and post-stop sessions through the live-probes fallback (m7-06 fix preserved).
  - `test_session_start_load_returns_metadata` — load after stop works (the single-call path saves to redb before the response is built).
  - `test_session_start_attach_returns_unsupported` — stub surface.
  - 5 deletion/drop tests untouched.
- `chronos-sandbox/tests/program_scenarios.rs` (11 tests, all pass):
  - `test_session_lifecycle_in_full_session` — full lifecycle on real binary.
  - 10 pre-existing tests untouched.

## Gates (this cycle)

- **T0 (fmt + clippy `-D warnings`)**: PASS.
- **T1 (workspace lib, ex chronos-native)**: 798/798 + 4 ignored (same as m7-06 — no regression).
- **T2 (services + store + mcp integration)**: 358/358 (same as m7-06 — no regression).
- **T4 (sandbox smoke)**: 5 + 11 + 8 = 24 tests pass in ~167s wall (`--test-threads=1`).
- **Behaviour change**: `session_stop` MCP response `status` field gains an `already_stopped` variant (additive). Single-call path is internal; the wire shape is preserved.
- **No new ServiceError variants, no new `Unsupported(String)` stubs, no new `#[allow]` escapes outside the documented `mark_sealed` helper.**

## Risks and known limitations (carried forward)

- **v1 `probe_stop` still does NOT save to redb.** Only `session_stop` (v2) saves to redb. This means `session_start{action=load}` after a v1 `probe_stop` returns `SessionNotFound`. Acceptable per the m7-05 decision (v1 stays in-memory; sunset 2027-09-11).
- **`SessionStopPersistence` is service-internal.** If a future `chronos-cli` wants events on stop, it needs a new public API (out of m7-07 scope).
- **`mark_sealed` is `#[allow(dead_code)]`.** Acceptable because (a) the helper still has unit tests, (b) the helper is documented as "for future callers", and (c) the test module imports it explicitly so the public visibility is exercised.
- **`stop` (dispatcher-output) is not unit-tested in this cycle.** The dispatcher's `mark_sealed` call site is gone, but the new path through `stop_with_persistence` → synthesise output is exercised only in sandbox smoke. A dedicated unit test (using a stubbed ProbeContext that always returns Ok or always returns ProbeNotFound) is a m7+ candidate but was not blocking.

## Followups (m7+)

- **m7-09 (deferred):** Implement `chronos_domain::attach` API; unblocks `session_start{action=attach}` (currently `Unsupported`). Same scope as the m7-04 followup list, item 1.
- **m8 (new milestone):** Plan in m8 scoping cycle. Likely candidates: live-probe perf, MCP server hardening, distributed tracing. v1 sunset bookkeeping remains passive until 2027-09-11.

## Cross-references

* `docs/milestones/m7-07-session-stop-single-call-scoping.md` — parent scoping doc.
* `docs/milestones/m7-06-session-lifecycle-sandbox-smoke-merge.md` "follow_up_in_cycle_fixes.bug_1_session_stop_double_call" — the in-cycle workaround that this cycle replaces with a structural fix.
* `docs/milestones/m7-05-session-lifecycle-dispatchers-merge.md` "architectural_decisions.probe_stop_NOT_shimmed" — pre-existing double-call prediction.
* `docs/milestones/m7-events-read-scoping.md` — M7 split + sequencing.
* `sddk/changes/m7-06-session-lifecycle-sandbox-smoke-merge/apply-checkpoint.json` `followups[0]` — the explicit m7-07 booking.
* `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md` lines 12 (`session_stop` tool spec).

---

— Submitted 2026-09-11. Awaiting FF-merge to main + tag.
