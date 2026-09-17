# REC-C1.6 — tasks

**Cycle**: `p-3416cfb8288f8964/rec-c1-6-lifecycle-retention-wire`
**Base**: `5bbf7748` (REC-C1.5 CLOSED on `main`)
**WIP**: 1, research slot empty.

## Out of scope (explicit, copied from the proposal)

- REC-C2 (legacy/event-path deletion) — BLOCKED until REC-C1.8 handoff.
- Auto-stop on delete — forbidden; the user must call `session_stop`.
- Background sessions as part of the liveness check (kept narrow to
  `live_probes`).
- "Retention policy" wire field — no authoritative source yet; not
  invented.
- Touching `LogReadPage`'s existing fields beyond additive
  `retention`/`tail`.

## Tasks

### Group A — lifecycle-safe delete

- [ ] A1 `feat(rec-c1.6): SessionStillActive error variant + SessionLiveness port`
      - Add `ServiceError::SessionStillActive { session_id, hint }` to
        `crates/chronos-services/src/error.rs`.
      - Add `pub trait SessionLiveness { fn is_active(&self, &str) -> bool; }`
        to `chronos-services/src/sessions.rs` (or a new
        `liveness.rs`).
      - Add two impls: `McpLiveProbes` (chrono-mcp wires it) and
        `StaticLiveness` (tests).
      - Plumb `&dyn SessionLiveness` through `SessionsContext` (new
        field) so the delete path can call it.
- [ ] A2 `feat(rec-c1.6): delete_session precondition refuses live probe`
      - In `SessionsService::delete_session`, BEFORE
        `store.delete_session`, check `liveness.is_active(target)`;
        return `Err(SessionStillActive{..})` if true.
      - Mirror any existing precondition checks (e.g. ownership of
        `SessionsContext` lock ordering must not change).
- [ ] A3 `test(rec-c1.6): DEL-LIVE-3 unit test for unclean-recovered deletable`
      - `crates/chronos-services/tests/lifecycle_delete.rs::del_live_3_unclean_recovered_deletable`.
      - StaticLiveness marks A inactive (no writer); ensure B's
        register survives because B's id is in `StaticLiveness`
        (this is the only place we exercise the contrast).
- [ ] A4 `feat(rec-c1.6): MCP delete_session tool handler maps SessionStillActive`
      - In `crates/chronos-mcp/src/server.rs::delete_session` handler,
        add `ServiceError::SessionStillActive { .. } =>` arm.
      - Surface via the existing
        `CallToolResult::error(text_content(...))` envelope.
      - Construct `McpLiveProbes` for `ChronosServer`; wire it
        through `SessionsContext` at the call site.
- [ ] A5 `test(rec-c1.6): lifecycle_delete DEL-LIVE-1/2/4`
      - `chronos-sandbox/tests/lifecycle_delete.rs`.
      - DEL-LIVE-1: probe_start → delete → typed refusal,
        evidence still readable, dir intact.
      - DEL-LIVE-2: probe_start → refused delete → probe_stop →
        delete → restart → absent.
      - DEL-LIVE-4: while A's probe is live, delete B (B cleanly
        stopped) — A's manifest/tail/registry byte-identical.

### Group B — retention / tail on the wire

- [ ] B1 `feat(rec-c1.6): RetentionFacts + TailFacts on LogReadPage`
      - Add `pub struct RetentionFacts { retained_from_seq: u64,
        history_truncated: bool }` and `pub struct TailFacts { state:
        TailStateWire, tail_seq: Option<u64> }` to
        `crates/chronos-services/src/events_log_read.rs`.
      - `TailStateWire` enum serializes as snake_case.
      - Both implement `Serialize` (caller-determined whether to
        include both fields; we always include both).
      - `LogReadPage` gets two additive fields: `retention`,
        `tail`.
- [ ] B2 `feat(rec-c1.6): populate RetentionFacts + TailFacts in read_page`
      - In `read_page_with`, after the read loop, build
        `RetentionFacts::new(log.retained_from().0)` and
        `TailFacts::new(log.tail_state(), log.tail_seq())`.
      - If `tail_state` is `Unknown`, set `tail_seq = None` —
        no inference.
- [ ] B3 `test(rec-c1.6): WIRE-RET-1/2 + WIRE-TAIL-1/2/3 (unit-level)`
      - `crates/chronos-services/tests/wire_retention_facts.rs`
        (or a sibling `tests/wire_facts.rs` if the name is too long).
      - Pure-function unit tests against `read_page_with` driving an
        injected reader. No binary subprocess needed.
- [ ] B4 `feat(rec-c1.6): MCP events_read tool flattens retention + tail`
      - In `crates/chronos-mcp/src/server.rs::events_read`, pull the
        new fields off `LogReadPage` and add them under
        `retention` and `tail` keys in the success JSON envelope.
- [ ] B5 `feat(rec-c1.6): CursorStale envelope carries structured numbers`
      - In the events_read `ServiceError::CursorStale {..}` arm,
        attach a second content item (json) with
        `{error: "cursor_stale", requested_next_seq, retained_from_seq}`.
      - Existing text content is unchanged.
      - chronos-sandbox restart_uat R2 already parses both forms,
        so backward-compatible.
- [ ] B6 `test(rec-c1.6): WIRE-CURSOR-1`
      - chronos-sandbox/tests/wire_retention_facts.rs::wire_cursor_1.
      - Pre-seed a session with `retained_from=5`; call events_read
        with cursor seq=0; assert the json content item carries
        `requested_next_seq=0, retained_from_seq=5` (parse via
        rmcp JSON).

### Group C — regression + closure

- [ ] C1 `style(rec-c1.6): fmt + clippy -D warnings clean`
- [ ] C2 `test(rec-c1.6): T0/T1/T3 battery + T4-smoke (lifecycle_delete + wire_retention_facts + restart_uat + e2e_connectivity)`
- [ ] C3 `chore(rec-c1.6): apply-checkpoint.json + verify-findings.json + implementation-receipt.md`
- [ ] C4 `merge(rec-c1.6) --no-ff into main`, tag
      `rec-c1-6-lifecycle-retention-wire`, push.
- [ ] C5 `vault drift sweep PASS` (CC#4 cascade regenerated; CC#56
      still clean; rec-c1-5-closure row already in cycles index).

## Decision needed before apply: No

The path is A-lite (bounded, cross-cutting, no architectural fork).
The user's response to the C1.5 closure summary named these exact
two gaps and the WIP=1 plan; no design ambiguity remains.

## Chained PRs recommended: No

A single reviewable branch across both groups keeps the user's
"scope cohesion" preference. If A and B need to ship
independently, that becomes a follow-up cycle; we do not chain
within C1.6.

## Commit strategy (one reviewable work unit per group step)

| Commit | Group | Subject |
|---|---|---|
| 0af8c3c9 | C1.6.0 reconciliation | (already landed) |
| A1 | A | SessionStillActive + Liveness port |
| A2 | A | delete_session precondition |
| A3 | A | DEL-LIVE-3 unit test |
| A4 | A | MCP delete_session maps the error |
| A5 | A | DEL-LIVE-1/2/4 sandbox UATs |
| B1 | B | RetentionFacts + TailFacts types |
| B2 | B | read_page populates them |
| B3 | B | WIRE-RET-1/2 + WIRE-TAIL-1/2/3 unit |
| B4 | B | MCP events_read flattens retention + tail |
| B5 | B | CursorStale structured envelope |
| B6 | B | WIRE-CURSOR-1 UAT |
| C1 | C | fmt + clippy |
| C2 | C | regression battery |
| C3 | C | apply-checkpoint + verify-findings + receipt |
| C4 | C | merge + tag + push |

Each commit compiles independently and passes T0 + clippy.
