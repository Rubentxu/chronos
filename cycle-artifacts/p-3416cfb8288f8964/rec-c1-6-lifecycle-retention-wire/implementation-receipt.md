# Implementation Receipt — REC-C1.6

**Head SHA**: d1891ed3 (feat/rec-c1.6-lifecycle-retention-wire, base 5bbf7748)
**Branch**: feat/rec-c1.6-lifecycle-retention-wire
**Cycle**: rec-c1-6-lifecycle-retention-wire
**Route**: A-lite

## Changes

Close the two gaps left at REC-C1.5 closure: lifecycle-safe `delete_session`
and retention/tail facts on the events_read wire. No public MCP tool
schema changes; additive fields only.

| File | Diff |
|---|---|
| `crates/chronos-services/src/error.rs` | +9. Adds `ServiceError::SessionStillActive { session_id: String, hint: &'static str }` with a Display impl that names session_stop as recovery. |
| `crates/chronos-services/src/sessions.rs` | +60/-2. `SessionsService::delete_session` checks `SessionsContext::connected_sessions` BEFORE any side effect; returns `Err(SessionStillActive)` if the id is live. Two new unit tests (`delete_session_refuses_live_session_unit`, `delete_session_after_unconnected_succeeds`) regression-verified by toggling the precondition. |
| `crates/chronos-services/src/events_log_read.rs` | +278/-1. Adds `RetentionFacts { retained_from_seq, history_truncated }`, `TailStateWire` (snake_case enum: open/sealed/unclean/unknown), and `TailFacts { state, tail_seq }`. `read_page_with` signature gains `retained_from: EventSeq` + `tail_state: &TailState`. `TailFacts::from_log_tail` refuses to invent a tail_seq when state == Unknown. Seven unit tests in `rec_c1_6_wire_facts_tests` pin the wire form. |
| `crates/chronos-services/src/events_read.rs` | +2/-0. `EventsReadOutput::Query` passes `page.retention` and `page.tail` through to the wire. |
| `crates/chronos-services/src/output.rs` | +7. `EventsReadOutput::Query` gains `retention: RetentionFacts` and `tail: TailFacts` (additive, required). |
| `crates/chronos-mcp/src/server.rs` | +77/-0. (a) `session_start{action=spawn\|attach}` Ok arm: `connected_sessions.insert(session_id)`; Load action is defensive-removed. (b) `session_stop` Stopped + AlreadyStopped arms: `connected_sessions.remove(session_id)`. (c) `delete_session` handler: typed `Err(e @ SessionStillActive {..})` arm with text content carrying Display hint. (d) `list_threads` giant exhaustive match gains the new variant for compile. (e) `events_read` CursorStale arm: `CallToolResult::error` with TWO content items (text + json) so the agent gets structured numbers AND restart_uat R2 keeps parsing the text. |
| `chronos-sandbox/tests/lifecycle_delete.rs` | new file, 319 lines. Three UATs: `del_live_1_live_probe_delete_is_refused`, `del_live_2_refused_then_stop_then_delete_then_restart_absent`, `del_live_4_sibling_delete_does_not_perturb_live_a`. Uses raw `call_tool` so `isError:true` surfaces as `McpSandboxError::RpcError`. |
| `chronos-sandbox/tests/wire_retention_facts.rs` | new file, 269 lines. Three UATs: `wire_1_success_envelope_flattens_retention_and_tail`, `wire_2_success_envelope_reports_advanced_retained_from`, `wire_cursor_1_stale_envelope_carries_structured_numbers`. Uses `call_with_timeout` directly to preserve multi-content envelopes. |

## Commit timeline (since base)

| SHA | Title | Phase |
|---|---|---|
| `0af8c3c9` | docs(rec-c1.6): reconcile C1.5 closure + C1.6 active in roadmap | C1.6.0 |
| `75492dd6` | docs(rec-c1.6): refine design — connected_sessions is the liveness source | design-refine |
| `dccabee5` | chore(rec-c1.6): proposal + design + tasks + apply-checkpoint | planning |
| `7197c40e` | feat(rec-c1.6): refuse delete_session on a live session (typed error + 2 unit tests + MCP handler arm) | A1+A2+A3 |
| `7afb47ad` | feat(rec-c1.6): wire liveness marker for typed delete_session refusal | A4+A5 |
| `1da26f5c` | feat(rec-c1.6): RetentionFacts + TailFacts on LogReadPage (types + read_page population + unit tests) | B1+B2+B3 |
| `580a8f45` | feat(rec-c1.6): wire retention/tail into events_read success + CursorStale (events_read output struct + MCP envelope + sandbox UATs) | B4+B5+B6 |
| `d1891ed3` | style(rec-c1.6): fmt + clippy -D warnings clean | C1 |

## Surface impact

- **MCP tool signatures**: no changes. `delete_session`, `events_read`,
  `session_start`, `session_stop` retain their input/output DTOs.
- **events_read success envelope**: gains `retention: {retained_from_seq,
  history_truncated}` and `tail: {state, tail_seq}` as top-level keys. Both
  are required (not `Option`); the agent sees them on every page.
- **events_read CursorStale envelope**: now carries TWO content items
  (text + json). The existing text is unchanged so chronos-sandbox
  restart_uat R2 still parses it. The new json content item lets the
  agent pull `requested_next_seq` and `retained_from_seq` without
  scraping text.
- **ServiceError**: gains one variant. Any exhaustive match in the
  workspace would fail to compile without the new arm; the only one in
  the codebase (list_threads) was updated.

## Evidence chain (key regressions)

- `del_live_1_live_probe_delete_is_refused` (chronos-sandbox/tests/lifecycle_delete.rs):
  probe_start(test_add) → delete_session returns Err with text
  `session '<id>' is still active (stop the probe (session_stop) and try again); stop the probe (session_stop) before deletion`.
  Without the liveness-marker fix (7afb47ad) this test fails with `SessionNotFound`
  instead of `SessionStillActive` — the precondition would be reading an
  always-empty set.
- `wire_cursor_1_stale_envelope_carries_structured_numbers`
  (chronos-sandbox/tests/wire_retention_facts.rs): cursor at seq#0
  against retained_from=7 returns BOTH text AND a second json content
  item with `{"error":"cursor_stale","requested_next_seq":0,"retained_from_seq":7}`.
- `rec_c1_6_wire_facts_tests` (chronos-services/src/events_log_read.rs):
  7 unit tests pinning WIRE-RET-1/2 + WIRE-TAIL-1/2/3 + serde shape.
- `sessions::tests::delete_session_refuses_live_session_unit`
  (chronos-services/src/sessions.rs): regression-verified by toggling
  the precondition (commented out → test panics with `expected
  SessionStillActive refusal, got Ok(DeleteResult{..})`).

## Out-of-scope respected

- No auto-stop on delete (still forbidden per design constraint).
- No retention policy wire field invented (facts ≠ policy).
- REC-C2 (legacy/event-path deletion) is still BLOCKED until REC-C1.8
  handoff.
- Background sessions still not part of the liveness check (kept narrow
  to live_probes, now correctly populated).

## Risks and follow-ups

- The `read_page_with` signature gained two parameters (8/7, triggering
  `clippy::too_many_arguments`). The lint is allow-ed at the function
  level rather than refactored into a ReadPageParams struct, because
  the function is `pub(crate)` and acts as the injected-reader test
  contract; the parameter list IS the contract.
- `SessionsContext::connected_sessions` now has a single producer (MCP
  server) and a single consumer (`SessionsService::delete_session` via
  the precondition). No race was introduced because the set is behind
  `Mutex` and the producer clears on every stop; but a future
  background-session source would need its own producer arm.
