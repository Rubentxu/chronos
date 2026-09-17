# REC-C1.6 — design

## Goal

Two changes, both gated on the plan:

1. **Lifecycle-safe `delete_session`.** A destructive operation against a
   still-live probe becomes a typed refusal. No silent probe_stop. No
   partial state.
2. **Retention/tail facts on the wire.** `events_read` success and
   `CursorStale` envelopes surface the facts the agent needs to
   distinguish "no event", "retired", "lost", "tail uncertain" without
   inferring them from error text.

## Data flow today

```text
delete_session(session_id)
  -> SessionsService::delete_session(session_id, ctx)
       store_ref.delete_session(session_id)                   // durable gone
       delete_durable_execution_log(...)                      // dir gone
       Ok(DeleteResult { session_id, paths_removed })
  -> MCP tool handler converts DeleteResult into success envelope
```

The precondition "session is not live" is **absent** today. We need
to add it without auto-stopping the probe.

```text
events_read(session_id, cursor)
  -> read_page_with(...) returns LogReadPage
       { records, next, position_after, gaps, completeness }
  -> MCP tool handler flattens to:
       { session_id, total_matching, returned_count, events, next_cursor,
         position_after, gaps, completeness }
```

The success envelope lacks `retention` and `tail` facts. The
`CursorStale` error is a flat string.

## What we change

### 1. `SessionStillActive` typed error

In `crates/chronos-services/src/error.rs`:

```rust
#[error(
    "session '{session_id}' is still active ({hint}); \
     stop the probe (session_stop) before deletion"
)]
SessionStillActive {
    session_id: String,
    hint: &'static str,
},
```

In `SessionsService::delete_session` the precondition flows through
`SessionsContext`. Two clean designs:

- **Option A** (preferred): add `liveness: SessionLiveness` to
  `SessionsContext`. The MCP-side caller fills it from
  `ChronosServer::live_probes` (the actual map). Tests fill it
  manually. The service runs `if liveness.is_active(target) { return
  Err(SessionStillActive{..}) }` BEFORE either `store.delete_session`
  or `delete_durable_execution_log`. Symmetry: the liveness check
  owns the precondition; the service owns the side effects.

- **Option B**: inject `live_probes` directly into `SessionsContext`.
  Rejected because it pulls an MCP-only concept into services; tests
  would have to construct a fake `live_probes` map.

**Decision: Option A.** A new trait `SessionLiveness { fn
is_active(&self, id: &str) -> bool; }` with two impls:

- `McpLiveProbes::new(live_probes: &Arc<Mutex<HashMap<…>>>)` — used
  by the MCP tool handler.
- `StaticLiveness::new(HashSet<String>)` — used by tests.

This keeps `chronos-services` free of MCP types; the liveness check
is a port.

### 2. `delete_session` tool envelope

The MCP `delete_session` tool handler already maps `ServiceError` →
`CallToolResult::error(text_content(...))`. We add the new variant:

```rust
Err(ServiceError::SessionStillActive { .. }) =>
    Ok(CallToolResult::error(text_content(format!("{}", e)))),
```

The error text is identical to the existing pattern. The
`isError: true` envelope is unchanged.

### 3. `retention` and `tail` on `LogReadPage`

In `crates/chronos-services/src/events_log_read.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RetentionFacts {
    pub retained_from_seq: u64,
    pub history_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TailFacts {
    pub state: TailStateWire,  // open | sealed | unclean | unknown
    pub tail_seq: Option<u64>, // None only when state == Unknown
}

pub enum TailStateWire { Open, Sealed, Unclean, Unknown }
```

Add to `LogReadPage`:

```rust
pub struct LogReadPage {
    // ... existing fields ...
    pub retention: RetentionFacts,
    pub tail: TailFacts,
}
```

The MCP tool handler flattens both into the success envelope. JSON
serialization uses `#[serde(rename_all = "snake_case")]` on the
enum so `"open" / "sealed" / "unclean" / "unknown"` are the wire
strings. `policy` is intentionally absent.

The constructor of `RetentionFacts` derives `history_truncated` from
`retained_from_seq > 0`.

The constructor of `TailFacts` does NOT infer — if `tail_state` is
`Unknown` AND `tail_seq` is unavailable, `tail_seq = None`. The
agent learns "tail is unknown, no count" rather than "tail is at 0".

### 4. `CursorStale` structured envelope

The current wire shape:

```json
{ "isError": true,
  "content": [ { "type": "text", "text": "cursor at seq 0 is before the retention boundary 5" } ] }
```

C1.6 keeps the `isError: true` envelope (callers depend on it) and
adds a sibling JSON-object content item:

```json
{ "isError": true,
  "content": [
    { "type": "text", "text": "cursor at seq 0 is before the retention boundary 5" },
    { "type": "json", "json": {
        "error": "cursor_stale",
        "requested_next_seq": 0,
        "retained_from_seq": 5
    } }
  ] }
```

The MCP handler builds the JSON content sibling from the
`ServiceError::CursorStale` payload. The existing
`chronos-sandbox/tests/restart_uat.rs` parses both forms (text vs
JSON) so the change is backward-compatible.

### 5. UAT planning (concrete)

Per the proposal, two suites in
`chronos-sandbox/tests/`:

- `lifecycle_delete.rs`: DEL-LIVE-1..4.
- `wire_retention_facts.rs`: WIRE-RET-1..2, WIRE-TAIL-1..3, WIRE-CURSOR-1.

All tests use the `McpTestClient` factory pattern (and the new
`start_with_db_and_exec_log_root` from C1.5).

DEL-LIVE-3 (unclean-recovered, no live writer) is a unit test in
`chronos-services` because it does not need a real two-process
restart — it can simulate the "no live writer" state by leaving
`live_probes` empty, which the Liveness port already models.

## Risk / non-goals

- **Auto-stop on delete is forbidden.** The user's spec calls out
  silent probe_stop as exactly the bug to avoid. We do not add any
  fallback that hides a lifecycle transition.
- **`background_sessions` is not in the liveness check.** It is a
  separate concept (events-vs-control-plane linkage, not a writer).
  Real UATs may surface a future demand for it, but C1.6 stays
  narrow.
- **No `policy` field on the wire yet.** No authoritative policy
  source in the code. We will not invent the name. C1.8 may add it.
- **No churn on `LogReadPage`'s existing fields.** Only additive:
  `retention`, `tail`.

## Sequencing within the cycle

```text
C1.6.0  done (this commit)
   |
   +--> 1. SessionStillActive error + SessionLiveness port
   |        (chronos-services)
   |
   +--> 2. delete_session precondition + tests DEL-LIVE-3 (unit)
   |
   +--> 3. MCP delete_session tool handler maps the new error
   |
   +--> 4. restart_uat: lifecycle_delete.rs DEL-LIVE-1 / 2 / 4
   |
   +--> 5. RetentionFacts + TailFacts on LogReadPage + serialization
   |        (chronos-services, no wire change yet)
   |
   +--> 6. wire_retention_facts.rs: WIRE-RET-1/2, WIRE-TAIL-1/2/3
   |
   +--> 7. MCP events_read tool flattens retention + tail
   |
   +--> 8. CursorStale structured envelope + WIRE-CURSOR-1
   |
   +--> 9. T0 fmt+clippy + T1/T3/T4-smoke battery
   |
   +--> 10. apply-checkpoint + verify-findings + receipt + merge + tag
```

Each step is reviewable; together they close both halves of C1.6.
