# m7-01 — `events_read` merge

**Branch:** `feat/m7-01-events-read-merge`
**Cycle:** M7 (v2-spec sub-cycle), first deliverable
**Precedence:** `docs/milestones/m7-events-read-scoping.md`;
`docs/milestones/m6-close-report.md` §6 item 1; v2 spec
`AGENT_API_V2.md` line 15
**Status:** PROPOSED, 2026-09-11

## Why this cycle

The v2 spec calls for an `events_read` tool (cursor-based,
non-destructive evidence read). The current v1 tools `query_events`
and `get_event` are the closest match — they expose event reads over
finalised sessions — but they:

* use **offset/limit** pagination (the v2 spec wants cursor-based);
* lack the **completeness / gap summary / provenance / next cursor**
  fields the v2 spec requires in responses;
* are split across two separate MCP tools (one for filters, one for
  single-id lookup).

m7-01 folds them behind a single v2 dispatcher + cursor-based
pagination model, and converts the v1 names into deprecated MCP
shims that route through the dispatcher.

## Scope (this cycle)

- Add `chronos-services::events_read` module (~300-380 LoC, 19th
  service module).
- Add `EventsReadKind` enum (`Query` / `ById`).
- Add `EventsReadInput` DTO carrying one of:
  - `Query { event_types, thread_id, timestamp_start, timestamp_end, function_pattern, limit, cursor }`
  - `ById { event_id }`
- Add `EventsReadOutput` envelope carrying the v1 payload plus the
  v2-spec fields: `next_cursor`, `completeness`, `gap_summary`,
  `provenance`.
- Add `ChronosEventsReadService::read` dispatcher with unit tests.
- Add `events_read` v2 MCP tool wrapper.
- Convert v1 `query_events` and `get_event` to deprecated shims that
  route through the dispatcher (pattern established by
  m6-01..m6-03).
- Cursor encoding: opaque base64 of a JSON `{total_pushed,
  snapshot_len}` payload (matches the existing `probe_drain` cursor
  handling).

## Out of scope

- Live ring buffer reads (today: `probe_drain`). The v2 `events_read`
  reads from the finalised session; live reads remain under
  `probe_drain` until m7-02 (`observe` merge) unifies them.
- `observe` merge (m7-02). Tripwires, properties, and probe injection
  stay as-is.
- `session_compare` / `session_explain` split (m7-03).
- `session_start` / `session_stop` / `capabilities` surface (m7-04).
- Deprecation sunset sweep (m7-05).
- M7 close (m7-06).

## Algorithm

### `Query` mode

Wraps the existing `DebugTraceService::query_events` algorithm. The
input carries:

* `event_types: Option<Vec<EventType>>` — typed enum, parsed at the
  MCP boundary (today the v1 tool parses strings inline; the v2
  wrapper does the same and returns `InvalidInput` on unknown types
  for symmetry with `m6-02`'s `state_query` dispatcher).
* `thread_id`, `timestamp_start`, `timestamp_end`, `function_pattern`
  — same semantics as v1.
* `limit: Option<usize>` — caps the page size. The MCP boundary
  defaults to a sensible value (e.g. 200) if `None`.
* `cursor: Option<EventCursorDto>` — opaque string. If `None`, the
  dispatcher issues a fresh cursor at the head of the result set
  (offset = 0). If `Some`, the dispatcher validates the cursor
  against the session's `total_pushed` and either:
  - returns the next page starting at the cursor's position; or
  - returns `ServiceError::CursorStale` if the cursor's `total_pushed`
    is older than the session's current `total_pushed`.

The output carries:

* `events: Vec<TraceEvent>` — the matching events (same shape as v1).
* `next_cursor: Option<EventCursorDto>` — opaque string for the
  next page; `None` if the result set was fully returned.
* `completeness: String` — `"best_effort"` for the offset/limit
  compatibility shim path; `"complete"` for the cursor path.
* `gap_summary: Option<Vec<GapDescriptor>>` — always `None` in m7-01
  (the `chronos_query::QueryEngine` does not yet expose gap info;
  reserved for m7+).
* `provenance: ProvenanceDescriptor` — `{source: "query_engine",
  captured_at: session_start_ts, session_id: ...}` (constant for
  this mode).

### `ById` mode

Wraps the existing `DebugTraceService::get_event` algorithm. The
input is `{event_id: u64}`. The output is the `TraceEvent` (or
`null` if not found — same semantics as v1).

The `provenance` field is filled the same way as `Query`.

## Cursor encoding (decision)

The cursor is encoded as an opaque base64 string carrying a JSON
payload `{total_pushed: u64, snapshot_len: u64}`. This matches the
existing `chronos_domain::EventCursor` type and the encoding pattern
already in `probe_drain`'s `CursorDto`. The MCP boundary:

* serialises: `EventCursor` → JSON → base64 (URL-safe no-pad).
* deserialises: base64 → JSON → `EventCursor` (returning
  `ServiceError::InvalidCursorPayload` on malformed input).

## v1 shim behaviour

`query_events` and `get_event` continue to work after m7-01 lands,
with the following changes:

* `query_events` body becomes a thin wrapper that builds an
  `EventsReadInput::Query`, calls the dispatcher, and re-serialises
  the output into the v1 JSON shape (with `not_found`, `reason`,
  `total_matching`, `returned_count`, `next_offset` fields preserved
  for backward compatibility). The shim does not honour the
  v2 `next_cursor` field (v1 callers don't read it).
* `get_event` body becomes a thin wrapper that builds an
  `EventsReadInput::ById`, calls the dispatcher, and unwraps the
  `EventsReadOutput::Event` variant to extract the `TraceEvent` or
  `None`.
* Both shims' `#[tool]` descriptions open with the literal string
  *"Deprecated. Use `events_read` with `mode=query` (or `mode=by_id`)
  instead."*

## Honest disclosure

* The v1 `query_events` shim accepts both offset/limit (legacy) and
  cursor (new). It translates offset → cursor at the dispatcher
  boundary. If a caller passes both, the cursor wins (with an
  `InvalidInput` warning if they conflict — the M0 lesson about
  explicit query absence semantics).
* The `gap_summary` field is reserved (always `None`) in m7-01
  because `chronos_query::QueryEngine` does not currently expose
  gap info. The DTO is final so m7+ can fill the field without
  breaking consumers.
* The cursor encoding decision (opaque base64 JSON) is final at
  m7-01 close unless a downstream cycle proposes a better one with
  a working migration path.

## Out-of-scope notes

* The `event_types` parsing in the v1 shim is currently inline in
  `server.rs:1241-1256`. m7-01 lifts it into the MCP wrapper for the
  new `events_read` tool but leaves the v1 shim's parsing where it
  is (to keep the shim's diff small). A future cleanup cycle could
  unify the parsing.
* This cycle is a v2 dispatcher with v1 shim conversion — same
  shape as m6-01..m6-03, no architectural fork.
