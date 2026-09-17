# REC-C1.7 — Projection Authority + final REC-C1 acceptance (design)

**Cycle**: `p-3416cfb8288f8964/rec-c1-7-projection-authority-acceptance`
**Branch**: `feat/rec-c1.7-projection-authority-acceptance`
**Companion**: `proposal.md`

## Architecture (after C1.7)

```
                       ExecutionLog  ← single authority
                            │
              ┌─────────────┼─────────────┐
              │             │             │
        events_read     projection    restart/recovery
              │             │
              │             ▼
              │       build_engine(         ← single canonical builder
              │           &SessionExecutionLog)
              │             │
              │             ▼
              │       QueryEngine  ← reconstructible projection
              │             │
              │     ┌───────┼───────┐
              │     │       │       │
              │     ▼       ▼       ▼
              │  execution state  trace_slice
              │    _query   _query
              │
              └──→ MCP wire (events_read response)

EventBus / drain_raw_events / fired_buffer / TripwireFired
     ↓
   LEGACY (owned by REC-C2, still present but not consulted by
   the canonical query operations)
```

The crucial invariant: **the only way `execution_query`, `state_query`,
or `trace_slice` learn about a session is via `build_engine(&log)`**.

## Module layout

New module `chronos_services::projection` (file
`crates/chronos-services/src/projection.rs`):

```rust
//! The single canonical builder that turns a `SessionExecutionLog`
//! into a `QueryEngine`. Every canonical operation that needs an
//! engine goes through this builder — `probe_stop`,
//! `session_snapshot`, and any future caller.

use chronos_domain::TraceEvent;
use chronos_log::{EventSeq, SessionId};

use crate::error::ServiceError;
use crate::events_log_read::{decode, payload_tag};
use crate::session_log::SessionExecutionLog;

/// What the projection knows about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionMeta {
    pub session_id: SessionId,
    /// First seq examined during projection. Always equal to
    /// `log.handle().retained_from()` — we never silently skip
    /// pre-retention records.
    pub projected_from: EventSeq,
    /// Last seq examined. `None` for an empty session.
    pub projected_through: Option<EventSeq>,
    pub completeness: ProjectionCompleteness,
    pub source: ProjectionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionSource {
    ExecutionLog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionCompleteness {
    Full,
    Truncated { retained_from: EventSeq },
    Empty,
}

/// Result of one projection. The engine is the *only* data the
/// canonical operations consume; the meta is exposed so the wire
/// can be honest.
pub struct ProjectionResult {
    pub engine: QueryEngine,
    pub meta: ProjectionMeta,
}

/// Build a `QueryEngine` for `log` by reading every record from
/// `retained_from` to `tail_seq`, decoding each via the shared
/// `decode()` helper, filtering noisy events, and feeding them into
/// the existing `IndexBuilder` + `QueryEngine::with_indices`
/// pipeline.
///
/// If `retained_from > 0`, returns
/// `ServiceError::EvidenceUnavailableDueToRetention { retained_from }`
/// when the projection is asked to claim "complete history" by
/// upstream callers (see `require_full_history`).
pub fn build_engine(log: &SessionExecutionLog) -> Result<ProjectionResult, ServiceError>;

/// Strict variant: refuses to project unless the log holds its full
/// history. Used by `execution_query`, `state_query`, and
/// `trace_slice` — none of those queries can give an honest answer
/// without the whole tail.
pub fn require_full_history(result: ProjectionResult) -> Result<ProjectionResult, ServiceError> {
    match result.meta.completeness {
        ProjectionCompleteness::Full | ProjectionCompleteness::Empty => Ok(result),
        ProjectionCompleteness::Truncated { retained_from } => {
            Err(ServiceError::EvidenceUnavailableDueToRetention {
                retained_from: retained_from.0,
            })
        }
    }
}
```

Implementation outline (uses existing primitives):

```rust
pub fn build_engine(log: &SessionExecutionLog) -> Result<ProjectionResult, ServiceError> {
    let handle = log.handle();
    let session_id = log.session_id().clone();
    let retained_from = handle.retained_from();
    let tail_seq = handle.tail_seq();
    let tail_state = handle.tail_state();

    // Decide completeness first — a TailState::Unknown with records
    // present is still `Full` because we can read the bytes; the
    // tail_state wire fact is its own dimension.
    let completeness = match tail_seq {
        None => ProjectionCompleteness::Empty,
        Some(tail) if retained_from.0 > 0 => {
            ProjectionCompleteness::Truncated { retained_from }
        }
        Some(_) => ProjectionCompleteness::Full,
    };

    let mut events: Vec<TraceEvent> = Vec::new();
    let mut position = retained_from;

    // Walk the log via the same primitive events_read uses, minus
    // filters. Bounded by chunk size so we cannot blow the stack on
    // huge sessions.
    const SCAN_CHUNK: usize = 1024;
    loop {
        let page = handle
            .read_from_seq(position, SCAN_CHUNK)
            .map_err(map_log_error)?;
        for record in &page.records {
            let event = decode(record).ok_or_else(|| {
                ServiceError::EvidenceDecodeFailed {
                    session_id: session_id.as_str().to_string(),
                    seq: record.seq.0,
                    payload_tag: payload_tag(record),
                }
            })?;
            events.push(event);
        }
        let next_position = page.position_after;
        let stopped = page.exhausted || events.last().is_none() == false && next_position <= position;
        position = next_position;
        if page.exhausted { break; }
        if position.0 == u64::MAX { break; } // safety
    }

    // Apply the same noisy-event filter the MCP server currently
    // applies (Custom/Registers, Unknown). Move the filter out of
    // server.rs into this module so the rule lives once.
    let events: Vec<TraceEvent> = events
        .into_iter()
        .filter(|e| !matches!(
            (&e.event_type, &e.data),
            (EventType::Custom, EventData::Registers(_)) | (EventType::Unknown, _)
        ))
        .collect();

    // Reuse the existing IndexBuilder + QueryEngine pipeline.
    let mut builder = IndexBuilder::new();
    builder.push_all(&events);
    let indices = builder.finalize();
    let engine = QueryEngine::with_indices(events, indices.shadow, indices.temporal)
        .with_causality(indices.causality)
        .with_performance(indices.performance);

    let meta = ProjectionMeta {
        session_id: session_id.clone(),
        projected_from: retained_from,
        projected_through: tail_seq,
        completeness,
        source: ProjectionSource::ExecutionLog,
    };
    Ok(ProjectionResult { engine, meta })
}
```

`build_and_store_engine` in `server.rs:2140` is rewritten to take a
`&SessionExecutionLog` instead of `Vec<TraceEvent>`:

```rust
async fn build_and_store_engine(
    &self,
    session_id: &str,
    log: &SessionExecutionLog,         // was: events: Vec<TraceEvent>
    language: Language,
) {
    match chronos_services::projection::build_engine(log) {
        Ok(projection) => {
            let ProjectionResult { engine, meta } = projection;
            let mut engines = self.engines.lock().await;
            engines.insert(session_id.to_string(), engine);
            self.projection_meta.lock().await.insert(session_id.to_string(), meta);
            info!(
                "Projected execution log into query engine for session {} \
                 (completeness={:?})",
                session_id, meta.completeness,
            );
        }
        Err(e) => {
            tracing::warn!(
                "Projection failed for session {}: {e} — engine will be \
                 empty until next refresh",
                session_id,
            );
        }
    }
}
```

`self.projection_meta` is a new `Arc<Mutex<HashMap<String, ProjectionMeta>>>`
on `ChronosServer`. The three canonical operations read `meta` alongside
the engine and refuse the call via `require_full_history` when the
projection is `Truncated`.

## Where each current `build_and_store_engine` caller goes

| Caller | Today | After C1.7 |
|---|---|---|
| `probe.rs::ProbeService::stop` returns `Vec<TraceEvent>` from a backend drain. | server.rs:4594 calls `build_and_store_engine(session_id, events, language)` after the wrapper. | server.rs keeps the result envelope (still needs `language` + `total_events` + `duration_ms` for `ProbeStopResult`), but the engine build routes through `projection::build_engine(&session_log)`. The `Vec<TraceEvent>` return is preserved for the MCP `probe_stop` JSON shape; it is *informational*, not authoritative. |
| `probe.rs::session_snapshot` | same drain pattern, line 608. | same routing. |
| Incremental refresh callers (the 7 callers in `server.rs` that pass synthetic events to `build_and_store_engine`) | mostly test fixtures and `save_session`-style code paths. | Each incremental caller either: (a) has access to a `SessionExecutionLog` and routes through `projection::build_engine`, or (b) writes its events into the ExecutionLog first, then re-projects. We will classify every caller in C1.7.2; nothing is left calling `build_and_store_engine` with raw events. |

`server.rs:6736`, `:6781`, `:7076` are test-only fixtures (they
synthesize events into the engine map without a live log). C1.7
replaces them with `SessionExecutionLog` fixtures so the same
test surface remains, but no production code path skips the log.

## The three canonical operations

### `execution_query`

`ExecutionQueryService::query` already takes `&TokioMutex<HashMap<String,
QueryEngine>>`. It now also takes a `&HashMap<String, ProjectionMeta>`
(or we store meta alongside the engine in a single
`HashMap<String, Projection>`). When `meta[session_id].completeness`
is `Truncated`, the service returns
`ServiceError::EvidenceUnavailableDueToRetention { retained_from }`
**before** touching the engine. Wire response shape unchanged; only the
error path changes.

### `state_query`

Same change as `execution_query`. Note that `state_query` also calls
`DebugTraceService::state_diff` (memory/register state at two events),
which is address-driven and does not need a full historical
projection — but the engine map is still required for it, so we still
require `Full`. This matches the user's "no fake engine" rule.

### `trace_slice`

Same change. `trace_slice` returns a windowed view of events; a
truncated history would silently omit pre-retention events. Same
`require_full_history` gate.

## Why this is not the hexagonal inversion

C3 will introduce `ExecutionLogProvider` as a port that
`chronos-services` consumes. C1.7 deliberately does NOT do that: we
keep using `SessionExecutionLog` directly. The benefit is that C1.7
has zero new architecture; the cost is that C3 will need to swap
`SessionExecutionLog` for the port. The constraint
*`build_engine` is the only engine-construction path* survives the
swap because it lives at the services boundary, not at the port.

## Tests

### `chronos-services::projection::tests`

- `build_engine_empty_session_returns_empty_completeness`.
- `build_engine_full_history_returns_full_completeness`.
- `build_engine_truncated_history_returns_truncated_completeness_and_refuses_require_full_history`.
- `build_engine_decodes_via_shared_decode_helper` — uses a payload
  with a deliberately corrupted JSON to assert
  `EvidenceDecodeFailed { seq, payload_tag }`.
- `build_engine_filters_registers_and_unknown` — verifies the noisy
  filter still applies after the migration.

### `chronos-services::dual_truth_characterization`

The C1.7.1 RED tests get a dedicated module so the proof is
grep-able. After C1.7.2 they become GREEN.

### `chronos-sandbox::projection_restart_equivalence`

End-to-end: starts an MCP server, runs the three operations,
restarts the server (drops the engine map), runs them again,
asserts JSON equality.

### `chronos-sandbox::time_semantics_uncorrelated`

Fixture builder that injects records with seq/event_id/timestamp_ns
deliberately uncorrelated (40, 90, 130, / 10_000_500, 25_320_700,
25_999_001). Reads via events_read, execution_query, trace_slice.
Asserts that `record.seq != record.event_id`, and that no encoded
value is dropped, swapped, or coerced to a wall-clock interpretation.

### Public C1-01 UAT (real MCP)

A new `chronos-sandbox::c1_01_two_consumers_real_wire` suite:

- Spawn an MCP server.
- Append 10,000 records via the durable producer.
- Consumer A: cursor at seq 0, reads 100, returns.
- Consumer B: cursor at seq 0 (independent consumer id), reads 100,
  returns.
- Producer advances by 1,000.
- Consumer A resumes; reads 200 from its saved cursor; verifies
  it observes 200 records none of which B has read.
- Consumer B resumes; reads 200 from its cursor; verifies A's
  records are absent.
- Force a gap (use the C1.4 fixture): the read crossing the gap
  returns `GapDetected`, never `Complete`.

## Migration safety

`build_and_store_engine`'s caller count is finite (10 callers in
`server.rs` after excluding tests). The migration is mechanical:

1. For each caller, locate the session's `SessionExecutionLog` (it
   is in `self.session_logs` or its registry — already plumbed).
2. Replace `build_and_store_engine(sid, events, lang)` with
   `build_and_store_engine(sid, &log, lang)`.
3. Drop the `events` parameter from the call sites; the events
   are now derived from the log inside `build_engine`.
4. Synthetic test fixtures move to use a
   `SessionExecutionLog::create(test_dir, session_id)` + a small
   `append(NewExecutionRecord{...})` helper.

## Out of scope (explicit)

- Removing `EventBus` / `fired_buffer` / `drain_raw_events` /
  `TripwireFired` from the codebase. C2 owns this.
- Changing `TraceEvent::timestamp_ns` semantics. C1.7 only proves
  the current model is honest.
- Introducing an `ExecutionLogProvider` port. C3 owns this.
- CC#39 governance debt. A separate `gov-*` cycle.
- Cross-process projection cache. C1.7 keeps the projection
  per-process, rebuilt on demand.
