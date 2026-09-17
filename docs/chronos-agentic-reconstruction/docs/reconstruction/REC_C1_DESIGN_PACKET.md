# REC-C1 Design Packet — ExecutionLog/events_read truth cutover

Status: DESIGN / CHARACTERIZATION ONLY. No implementation in this branch.
Branch: design/rec-c1-truth-cutover (rebased onto post-REC-C0 main b2568199).
Owner gate: REC-C1. Inputs: reconstruction-contracts.toml (TRUTH-001/002/003,
LOG-001/002, DEF-001), SANDBOX-S0 roadmap, ROADMAP_INTEGRATION_VERIFICATION_LAB.

## 1. Current-state ground truth (verified against code on this branch)

### Cursor model (chronos-log)

- `ConsumerCursor { consumer: LogConsumerId, last_seq: EventSeq }`.
- `ReadResult`: `Ok { records, gaps, next_cursor }`, `CursorStale { consumer,
  expected, current }`, `SessionNotFound`.
- Cursors are per-consumer independent; resume = pass back `next_cursor`
  (or `ConsumerCursor::fresh` after `CursorStale`).
- `GapReason` tags: KernelRingOverflow, AdapterBufferOverflow,
  ProcessDetached, TransportFailure, CorruptSegment, UnsupportedEvidence.

### MCP v2 events_read surface (chronos-mcp/server.rs)

- `mode=query` (supersedes v1 `query_events`) and `mode=by_id` (supersedes
  `get_event`). Deprecated v1 shims preserve legacy JSON shapes.
- Wire cursor: `CursorDto { total_pushed, snapshot_len }` → domain
  `EventCursor`; `to_domain()` returns None on malformed payload.
- Completeness today: v2 reads hardcode `completeness: "complete"` in at
  least one path — this is the TRUTH-003 gap. The v2 wrapper does NOT
  apply the caller cursor to query position in all paths — TRUTH-002 gap.
- Query path reads from the live probe's SegmentedExecutionLog
  (`chronos-log::SegmentedExecutionLog` source tag) or legacy EventBus
  depending on backend config (m1-03 dual-write, m1-08 legacy EventBus
  read path still exists).

## 2. DEF-001: the five failing tests = characterization suite

Owner REC-C1. Current behavior observed on main (also reproduced
independently at 9cc44ce3 during REC-C0 verification):

| test | expected by test | actual server behavior |
|---|---|---|
| query_filters::test_query_events_offset_pagination | disjoint pages via offset 0/10 | passes (no overlap) |
| query_filters::test_query_events_offset_beyond_total | empty for offset>total | returns full first page regardless |
| query_filters::test_query_events_limit_exact_pagination | exact page slicing | passes with tolerance |
| query_edge_cases::test_query_events_offset_beyond_total | empty for offset=1e6 | returns 10 events |
| query_edge_cases::test_query_events_pagination_all_events | total pages ≈ stop.total_events | fetched 100100 vs stop 2479 (page limit hit) |

Characterization decision: convert all five into
`#[characterization]`-style tests (renamed `*_current_behavior`) that
assert TODAY'S behavior with explicit `// CHARACTERIZATION: TRUTH-002 gap`
annotations, so the cutover PR must consciously flip them to the new
assertions. The offset/pagination fix belongs to the events_read v2
dispatcher applying the ExecutionLog EventSeq cursor, not the EventBus
snapshot position.

## 3. Target contract (delta)

1. **Cursor authority**: `mode=query` pages are defined by
   `SessionId + EventSeq(last_seq) + schema_version`; wire cursor remains
   opaque CursorDto but maps 1:1 to `ConsumerCursor`. Server rejects
   (malformed) cursors whose payload cannot map to domain.
2. **Wrong-session/stale errors**: cursor bound to SessionId; foreign
   session cursor → `SessionNotFound`-equivalent MCP error; stale →
   `CursorStale` mapping with `expected`/`current` in the error payload.
3. **Completeness is evidence-derived** (TRUTH-003): map
   `ReadResult::Ok{gaps:[]}` → `complete`; `gaps non-empty` →
   `gap_detected` (+gap list); retention-truncated → `partial`; backend
   without ExecutionLog attached → `unknown` (never silently complete).
4. **Restart/resume**: consumer re-attaches with its cursor; log reopen
   (second SegmentedExecutionLog handle, per m1_04 test) must yield the
   same seq space.
5. **Retention**: bounded loss always surfaces as `Gap` (LOG-002), never
   shifts `complete`.
6. **Two independent consumers**: isolation tests already exist in
   m1_acceptance; add the same shape over the MCP v2 surface.
7. **Cutover**: `mode=query` reads ONLY from SegmentedExecutionLog;
   EventBus demoted to producer-side feed; legacy v1 shims keep old JSON
   shape but inherit new completeness values (documented divergence).
8. **Boundaries**: QueryEngine stops being authority for agent-visible
   reads (TRUTH-001); stays for in-memory analytics.

## 4. UAT (mandatory, from roadmap)

- UAT-10k: 10 000 synthetic events through a real SegmentedExecutionLog;
  two consumers page independently to exhaustion; assert zero overlap,
  monotonic EventSeq, final completeness derived from gaps.
- UAT-gap: inject a GapReason mid-stream; consumer observes gap_detected
  and the gap payload.
- UAT-stale: cursor older than retention → CursorStale error, not silent
  re-anchor.
- UAT-cutover: with ExecutionLog disabled, v2 reports completeness
  `unknown` (No Silent Lies), not complete.

## 5. Non-goals

- No EventBus deletion (REC-C2 / DOG-003).
- No sandbox/ExecutionEnvironment coupling; DOG-001/002 adoption happens
  through the Verification Lab seam only if S0 results justify it.
- No change to v1 shim JSON shapes beyond completeness values.

## 6. Risks

- `probe_drain` cursor coupling (CursorDto is shared) — needs explicit
  version bump or dual-field cursor to avoid breaking live probes.
- 100k-event pagination behavior under cutover: page limit loop
  protections must survive the new cursor semantics.

## 7. Proposed task slice order (for sddk-tasks phase)

1. Characterization commit: rename/annotate the five DEF-001 tests.
2. Cursor mapping + error mapping in services layer (pure, unit-tested).
3. Completeness derivation from ReadResult (pure, unit-tested).
4. Dispatcher cutover + UAT-10k/gap/stale/unknown.
5. Flip characterization assertions to target contract; DEF-001 retires.
