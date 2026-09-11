# M7 — Agent API v2 sub-cycle (Scoping)

**Cycle:** `p-3416cfb8288f8964/m7-events-read-scoping` (B-direct, scoping)
**Path:** B-direct (documentation cycle — no production code changes)
**Author:** orchestrator
**Date:** 2026-09-11
**Status:** scoping proposal — **OPEN**, awaits m7-01 kickoff

> **Naming.** "M7" here refers to the v2-spec sub-cycle that follows the
> M6 sub-cycle (closed 2026-09-11). It is **not** the same as the
> reconstruction-roadmap M7 (*Differential execution v2*) in
> `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` line 163.

---

## Why this cycle

The M6 close report (`docs/milestones/m6-close-report.md` §6) listed
seven v2 spec tools still pending. The m6-06 close cycle closed M6 as a
*structural* milestone (5/8–12 v2 spec tools live, 15 v1 tools preserved
as deprecated shims). The remaining 7 are sequenced for M7+:

1. `events_read` — cursor-based, non-destructive evidence read (spec line 15)
2. `observe` — unified subscription/tripwire/property model (spec lines 28–42)
3. `session_compare` + `session_explain` — split out of overloaded `diff.rs`
4. `session_start` + `session_stop` + `capabilities` — session-lifecycle v2 surface
5. Deprecation sunset sweep — after 2027-09-11 (or extend the deadline)

This cycle does **not** refactor any code. It produces a scoped
proposal that downstream M7 cycles can execute against, and ships the
**m7-01 cycle spec** for the first deliverable (`events_read` merge).

---

## Current event-read tool surface (M7 entry)

Two v1 MCP tools expose event reads:

* `query_events` — paginated read with filters (event_types, thread_id,
  time range, function_pattern, limit, offset). At
  `crates/chronos-mcp/src/server.rs:1230` (248 LoC of MCP body +
  exhaustive `ServiceError` arms + JSON envelope).
* `get_event` — single-event lookup by id. At
  `crates/chronos-mcp/src/server.rs:1478` (28 LoC of MCP body + error
  mapping + JSON envelope).

Both call into `chronos_services::debug_trace` (6 v1 methods, 596 LoC
total), which already owns the algorithm. The v1 tools are pure
transport shims over the existing service code.

A related tool, `probe_drain`, already exposes a cursor-based read
(`ProbeDrainInput.cursor: Option<chronos_domain::EventCursor>`) for
the live ring buffer (m3-04 extraction). The cursor infra
(`chronos_domain::bus::EventCursor` + `CursorStatus` + `InvalidCursorPayload` /
`CursorStale` `ServiceError` variants) is already in place — `events_read`
will reuse it rather than introduce a parallel cursor type.

---

## v2 spec target

From `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
line 15:

> `events_read` — cursor-based, non-destructive evidence read.

Combined with the spec's "Agent ergonomics" section (line 60 onward):
responses must include the next cursor where relevant, plus
completeness, gap summary, provenance, and stable IDs for follow-up
calls.

The v2 `events_read` tool therefore:

* exposes a **cursor-based pagination** model (replacing offset/limit
  for the finalised-session path; matching `probe_drain`'s live
  path).
* is **non-destructive** — the same cursor returns the same event
  set on a second read.
* folds `query_events` + `get_event` behind a single entry-point with
  a discriminator parameter (`mode=Query` vs `mode=ById`).
* reuses the existing cursor infra (no new cursor type).
* preserves the v1 names as deprecated shims (per the M6 standing
  policy in `docs/milestones/m6-close-report.md` §4).

---

## Proposed M7 cycle split

| Cycle    | Topic                                                                              | Notes                                                                                  |
| -------- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| m7-01    | `events_read` merge — `query_events` + `get_event` → 1 v2 dispatcher + 2 shims     | This scoping cycle produces the m7-01 spec for execution.                              |
| m7-02    | `observe` merge — 4 `tripwire_*` + `probe_inject` → 1 v2 dispatcher + 5 shims      | Largest semantic change; needs its own scoping pass.                                    |
| m7-03    | `session_compare` + `session_explain` — split out of `chronos_services::diff`      | Smaller; mostly a rename + parameter restructuring.                                     |
| m7-04    | `session_start` + `session_stop` + `capabilities`                                  | Touches session lifecycle; depends on m7-02 (observe) for the capabilities surface.    |
| m7-05    | Deprecation sunset sweep (after 2027-09-11)                                        | Cleanup; only run if the deadline is not extended.                                     |
| m7-06    | M7 close                                                                            | Docs + structural audit (analog to m5-10, m6-06).                                      |

The exact cycle split will be set at M7 kickoff based on empirical
evidence gathered during m7-01..m7-03.

---

## Sequencing recommendation (m7-01 first)

`m7-01` is the simplest of the five candidates because:

1. The algorithm already lives in `chronos_services::debug_trace`.
   m7-01 is a pure dispatcher + shim conversion (pattern proven by
   m6-01..m6-03).
2. The cursor infra is already in `chronos_domain::bus`. No new
   domain types or persistence changes.
3. The v1 tools are small (~280 LoC combined) so the diff is
   reviewable in one cycle.
4. There is no architectural fork — this is an A-min path
   (1 spec + 1 tasks + ~5 commits).

`m7-02` (observe) is the largest and most architecturally risky
because it unifies three current v1 surfaces (tripwires,
properties, probe injection) under a single subscription model. It
deserves its own scoping pass before execution. It is sequenced
*after* m7-01 so the dispatcher pattern's quirks (input validation,
error-mapping, shim ergonomics) are well-understood by then.

---

## Risks and unknowns

* **Cursor encoding.** `chronos_domain::EventCursor` is a structured
  type with `total_pushed` + `snapshot_len` integers. The MCP
  envelope must serialise/deserialise it as a string (the spec line
  66 promises "next cursor" in the response). The encoding choice
  (base64 JSON, plain JSON object, or a tagged opaque string) is
  open and will be set in m7-01's design step. Default: opaque
  base64 of a JSON `{total_pushed, snapshot_len}` payload (matches
  the existing `probe_drain` cursor handling).
* **Offset/limit compatibility.** The current v1 `query_events`
  uses offset/limit. The v2 `events_read` cursor model replaces it.
  For one M7 minor the v1 shim must accept both forms and translate
  them to cursor semantics (offset → cursor at that offset).
* **Completeness / gap summary.** The spec line 60 promises
  "completeness, gap summary, provenance". Today's
  `QueryEventsResult` does not carry these fields. m7-01 will add
  them to the new `EventsReadOutput` envelope; the v1 shim will
  fill them with `completeness="best_effort"` and `gap_summary=null`
  for the offset/limit path so callers see the new shape without
  a behaviour break.
* **`probe_drain` vs `events_read`.** Both are cursor-based reads;
  `probe_drain` reads from the live ring, `events_read` reads from
  the finalised session. m7-01 keeps them separate (the live ring
  cursor and the finalised-session cursor are different types of
  reads even though they share the same `EventCursor` payload).
  Unifying them is m7-02 (`observe`) territory, not m7-01.

---

## Exit criteria for this scoping cycle

1. `docs/milestones/m7-events-read-scoping.md` (this file) lands
   on `main` via FF-merge.
2. `docs/milestones/m7-01-events-read-merge.md` (the cycle spec for
   the first deliverable) lands on `main` via FF-merge.
3. `docs/ROADMAP.md` is unchanged — M7 candidates are already listed
   in the M6 close commit. The scoping cycle is documentation-only.
4. `cargo fmt --all -- --check` exits 0.
5. `cargo clippy --workspace --all-targets -- -D warnings` exits 0.

No source code is touched in this scoping cycle. The m7-01 cycle
that this scoping seeds is a separate A-min execution cycle.

---

## Cross-references

* `docs/milestones/m6-close-report.md` §6 — M7 candidate list.
* `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
  line 15 — the v2 `events_read` spec.
* `docs/milestones/m6-04-hypothesis-test.md` — pattern for a
  net-new v2 cycle spec (no v1 shim, but same structure).
* `docs/milestones/m6-01-trace-slice-merge.md` — pattern for a
  merge cycle spec (v2 dispatcher + v1 shim refactor).
* `docs/milestones/m5-10-close-M5/cycle-artifacts/` — pattern for a
  close-cycle spec.

---

— Submitted 2026-09-11. Awaits m7-01 execution.
