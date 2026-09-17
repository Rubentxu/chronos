# REC-C2.2 — tasks

**Companion**: `proposal.md`

| Step | State |
|---|---|
| C2.2.0 accepted-Raw seam + FIND-C2.1-03 | DONE |
| C2.2.1 spawn/attach producer convergence | DONE (attach now uses the same seam) |
| C2.2.2 `probe_drain` not a destructive authority | IN PROGRESS | canonical reader + DRAIN-1..6 + explicit ResolveContext landed; the wire switchover remains |
| C2.2.3 `probe_stop`/`session_snapshot` consumers off EventBus | PENDING |
| C2.2.4 remaining CANONICAL destructive reads -> 0 | PENDING (6 left) |
| C2.2.5 UAT-C2-01/02/03 characterization | PENDING |

## Evidence

- `accept_and_publish` ordering test (`probe_backend::tests::c2_2_*`): the
  observer runs AFTER the record is durable, BEFORE the event is fanned out,
  and never runs when the append is refused.
- `chronos-sandbox/tests/rec_c2_2_producer_derivation.rs`: the productive UAT.
  NEGATIVE assertion included: if the observer is unwired, it fails loudly
  instead of passing on hand-written fixtures.
- Ratchet: 31 -> 28 (`dual_push` 3 -> 0). CANONICAL destructive reads
  unchanged at 6; those are C2.2.2-4.

## C2.2.2 in progress

Landed: `NativeProbeBackend::project_semantic(&TraceEvent) -> SemanticEvent`, a
pure projection that never touches the EventBus (test:
`c2_2_projecting_semantics_reads_no_bus`). It is the piece `probe_drain` needs
to build its wire events from durable `Raw` records.

Remaining (measured blast radius, not yet changed):

- `ProbeService::drain` reads `EventBus::read_since` and recomputes
  `tripwires_fired` with `matching_semantic`. Target: one scan of the
  session's `ExecutionLog`, `Raw -> project_semantic`, `TripwireFired ->
  tripwires_fired += 1`, `GapMarker -> explicit completeness`, and
  `TripwireManager` dropped from the drain path entirely.
- Cursor: `EventsCursorV1` directly (same laws as events_read/observe:
  foreign malformed/stale are typed errors, never a silent re-anchor). The
  legacy ring cursor (`total_pushed`/`snapshot_len`) is NOT translated; if
  the old wire shape is kept it stays a separate COMPATIBILITY field.
- Page boundary: the cursor advances **after the last ExecutionRecord
  examined**, and a `Raw` plus its immediately following `TripwireFired`
  records are one logical read unit, so a page can never end with
  `tripwires_fired = 2` and zero source events.
- Wire change: `ProbeDrainInput.cursor` / `ProbeDrainResult.new_cursor`
  move from the ring cursor to the opaque `ecv1` token; the MCP handler and
  every sandbox test that touches `probe_drain` move with them (this is the
  blast radius that made it a separate commit).

Mandatory tests for the step:
- UAT-REC-C2-01 on this surface: drain the EventBus (or perturb it) with the
  log intact -> same events, same `tripwires_fired`, same cursor.
- Double-evaluation killer: log holds `Raw S` + `TripwireFired F(source=S)`
  and the `TripwireManager` is EMPTY -> the Raw event is visible and
  `tripwires_fired == 1`.

## C2.2.2 second increment

Landed:
- `project_semantic(event, ctx: &ResolveContext)` — the caller supplies the
  session's resolution context, so replaying the same durable `Raw` produces
  the same projection the producer produced. `resolve_context(binary_path)`
  exposes the backend's own context. (Rebuilding the context from ambient
  state was a real divergence risk: live capture uses
  `binary_path: Some(program_path)`, the old projector used `None`.)
- `crates/chronos-services/src/canonical_drain.rs` — the canonical reader:
  one scan over the `ExecutionLog`, `Raw -> project_semantic`,
  `TripwireFired -> tripwires_fired += 1`, gap -> explicit completeness.
  **No `TripwireManager` parameter and no EventBus.**
- Six DRAIN properties + the falsification, all green:
  DRAIN-2 evidence-not-configuration, DRAIN-3 Raw+derived as one logical unit
  (`max_raw=1` over `Raw0 F1 F2 Raw3` -> Raw + 2 firings + cursor at 3),
  DRAIN-4 a cursor inside a derived cluster is a typed `InvalidInput` (never a
  re-anchor), DRAIN-5 early stop still progresses, DRAIN-6 a traversed gap
  contaminates the page, and the falsification that bus perturbation cannot
  change the output.
- Budgets: `max_raw_events` + `max_examined_records` + `max_derived_per_source`
  (a pathological cluster fails explicitly instead of making the unit
  unbounded).

Remaining (the wire switchover, deliberately its own commit):
- `ProbeService::drain` calls the reader and drops `matching_semantic`;
  `TripwireManager` leaves the signature entirely.
- `ProbeDrainInput`/`ProbeDrainResult` stop using `EventCursor`/`CursorDto`
  for the canonical route: `evidence_cursor: "ecv1:..."` is CANONICAL, and any
  surviving ring cursor is a separate, explicitly-named COMPATIBILITY field.
  The ring cursor is NEVER converted into an EventSeq.
- `cursor_stale: bool` is replaced (on the canonical route) by the
  completeness vocabulary: Complete / GapDetected / Partial.
- The MCP handler and every sandbox test touching `probe_drain` move with it.
