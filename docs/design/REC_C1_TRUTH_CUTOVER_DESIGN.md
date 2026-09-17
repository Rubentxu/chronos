# REC-C1 Design Packet — ExecutionLog/events_read Truth Cutover

Status: DESIGN ONLY. No implementation in this branch. Branched from main
9cc44ce3; rebase after REC-C0 merge.

## 1. Goal

Make `ExecutionLog` (chronos-log) the single authoritative read path for
agent-visible event reads, closing:

- TRUTH-001 (ExecutionLog authoritative; retire EventBus-only/QueryEngine paths)
- TRUTH-002 (agent reads honor authoritative EventSeq cursors)
- TRUTH-003 (completeness/gaps evidence-derived, never assumed complete)
- DEF-001 (5 failing pagination/offset tests become the acceptance set)

## 2. Cursor contract

Authoritative cursor tuple:

```text
SessionId + EventSeq + schema_version
```

- `SessionId` scopes the log; reads against a different session MUST fail
  with `wrong_session`, never silently return foreign events.
- `EventSeq` is the last processed seq; `read_after` returns seq > last.
  EventSeq is append-only monotonic per session (LOG-001, already verified).
- `schema_version` travels with the read result; stale-schema reads return
  `stale_cursor` and force a resync, not a partial read presented as current.

Error taxonomy (must be typed, not strings):

| Error | Meaning |
|---|---|
| `malformed_cursor` | unparsable/ill-formed cursor payload |
| `stale_cursor` | cursor schema_version older than log segment schema |
| `wrong_session` | cursor SessionId != log SessionId |

## 3. Read result semantics (TRUTH-003)

`events_read` v2 must derive completeness from evidence:

- `Complete` — read reached the log tail with no gaps in the returned range.
- `Partial` — limit/retention truncated; reader knows more exists (cursor is
  resumable).
- `GapDetected(GapReason)` — one or more `Gap` entries intersect the range;
  the response lists gap reasons (KernelRingOverflow, AdapterBufferOverflow,
  ProcessDetached, TransportFailure, CorruptSegment, UnsupportedEvidence).
- `Unknown` — the log cannot prove completeness (e.g. compaction removed the
  probed range); NEVER presented as Complete.

Prohibition (DOG-005): no path may return `Complete` without gap knowledge.

## 4. Restart/resume and retention

- Cursors persist per `LogConsumerId`; consumers are independent
  (chronos-log/src/cursor.rs contract, LOG-001).
- Restart resume: a consumer re-attaching with an old cursor reads from
  `last_seq + 1`; if segments were compacted away, result is `Unknown` or
  `GapDetected(CorruptSegment/compaction)`, never a silent re-read from zero
  presented as current.
- Retention/compaction interplay: compaction counters
  (segments_removed_total, bytes_reclaimed_total) must be surfaced so readers
  can distinguish "caught up" from "history evicted".

## 5. Two-consumer invariant

Two independent `LogConsumerId`s (e.g. `index` and `ui`) reading the same
session: neither cursor affects the other; each gets its own gap/completeness
verdict. UAT: same as m1_acceptance dual-consumer test but through the
public `events_read` v2 surface, not the in-process API.

## 6. Cutover plan (EventBus → ExecutionLog)

1. events_read v2 `mode=query` backed by `SegmentedExecutionLog` reads only
   (drop the EventBus drain path behind the tool).
2. Deprecated v1 shims (`query_events`, `get_event`) keep JSON shapes but
   route through the same ExecutionLog-backed service.
3. `QueryEngine` boundary: query filters (event_types, thread_id, ts range,
   function_pattern) apply on ExecutionLog read pages; QueryEngine becomes a
   pure transform, not an evidence source.
4. Delete EventBus-only read path last (REC-C2), gated on DOG-003.

## 7. DEF-001 — five tests that define done

Suite: `chronos-sandbox/tests/query_filters.rs` + `query_edge_cases.rs`.

| # | Test | Current failure mode |
|---|---|---|
| 1 | query_filters::test_query_events_offset_pagination | offset not applied to read position (TRUTH-002) |
| 2 | query_filters::test_query_events_offset_beyond_total | returns a full page instead of empty |
| 3 | query_filters::test_query_events_limit_exact_pagination | page boundary off-by-N vs limit |
| 4 | query_edge_cases::test_query_events_offset_beyond_total | same as 2 (reproduced on main 9cc44ce3) |
| 5 | query_edge_cases::test_query_events_pagination_all_events | total mismatch > tolerance; pagination walks past tail (100100 fetched vs 2479 from stop) |

These become characterization tests FIRST: lock current (wrong) behavior in
`#[ignore]`-marked characterization tests with the observed values, then the
cutover flips them to the corrected expectations and removes DEF-001 from
reconstruction-contracts.toml `baseline_scope.deferred`.

## 8. 10k-event UAT (mandatory, from roadmap addendum)

- Seed ≥10k events into a SegmentedExecutionLog fixture.
- Two consumers paginate the full range through v2 events_read.
- Assert: seq continuity, per-consumer cursor independence, gap injection
  mid-range produces `GapDetected` with the right reason, restart resume picks
  up at cursor, zero events lost or duplicated relative to the log itself.

## 9. Out of scope

- EventBus deletion (REC-C2).
- Any sandbox backend work (SANDBOX-S0 track).
- Coverage/workflow changes.
