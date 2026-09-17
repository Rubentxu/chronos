# REC-C2.1 — tasks

**Cycle**: `rec-c2.1-tripwire-evidence`
**Companion**: `proposal.md`

| Step | State | Notes |
|---|---|---|
| C2.1.0 planning | DONE | proposal.md + this file |
| C2.1.1 `ExecutionKind::TripwireFired` + `TripwireFiredEvidence` | DONE | appended last (Raw=0/GapMarker=1 pinned by test); payload codec; `NewExecutionRecord.kind` plumbed (backends previously hardcoded `Raw`) |
| C2.1.2 persist-first source acceptance | DONE | `dual_push` appends first, publishes only on success; no-log path stays COMPATIBILITY; attach-loop path untouched (COMPATIBILITY, C2.2) |
| C2.1.3 derive from accepted Raw + recursion barrier | DONE | `tripwire_evidence::{derive_firings_from_record, derive_firings_from_event, read_firings}`; barrier dispatches on `ExecutionKind`; failures reported, never a `Gap` |
| C2.1.4a async boundary + `resolve_session` | **DONE** | `observe` is async; explicit scope wins; `active_session` fallback; guard dropped before any I/O; 4 direct tests incl. lock-release; 25 call sites / 20 tests migrated |
| C2.1.4b log-backed firing reads + `EventsCursorV1` | **DONE** | list reads the log; strict cursor errors; scan budget; `session_id` + checkpoint cursor on the wire; FIRING-PAGE-1..4; fixtures migrated off `fired_buffer` |
| C2.1.5a bounded/progress-aware scan + count completeness facts | **DONE** | hidden 64-page cap removed; typed stall; FiringCountSnapshot |
| C2.1.5b async `query` + derived `fire_count` + honest retention | **DONE** | FIND-C2.0-01/02 closed |
| C2.1.6 delete the legacy fired queue | **DONE** | fired_buffer/record_fired/drain_fired/evaluate/evaluate_semantic/TripwiresService::list/TripwireFired DTO deleted; ratchet 36→31, fired_buffer 4→0, drain_fired 1→0, CANONICAL 11→6 |
| C2.1.7 restart/replay identity UAT + wire identities | **DONE** | two-process UAT; firing_seq/source_seq on the wire; stale docs corrected |

## C2.1.4a note

`observe` is async now; `create`/`delete`/`query` stay synchronous. The test
harness dropped `with_ctx` for `async fn observe(&self, input)` because an
`FnOnce -> R` closure cannot contain `.await`.

## Superseded: why C2.1.4 was originally blocked

`observe` is a **synchronous** service function:

```rust
pub fn observe(ctx: &ObserveContext<'_>, input: ObserveInput) -> Result<ObserveOutput, ServiceError>
```

To read firings from the session's ExecutionLog it needs the active session id,
which lives behind a `tokio::sync::Mutex` (`ProbeContext::active_session`). A
sync function cannot lock it. Two honest options, both larger than a
one-commit change:

1. make `observe` async (ripples through the MCP handler and ~40 tests), or
2. add a synchronous handle to the active session's log to the context
   (a second source of the active session id, which is exactly the kind of
   duplicated identity REC-C1.2a spent effort removing).

Option 1 is preferred; it deserves its own commit and a full test pass.

## What is already true after C2.1.0-3

- firings have a typed durable representation with `source_seq`;
- a firing can only derive from an `ExecutionLog`-accepted `Raw` record;
- the recursion barrier is by `ExecutionKind`, not by payload tag;
- the producer no longer observes (bus or semantic fan-out) anything the log
  refused;
- a failed derived append is surfaced, not hidden, and never becomes a `Gap`.

## Still missing from the DoD

- `observe list` still drains the legacy `fired_buffer` (FIND-C2.0-01),
  `fire_count` is still the stale mutable counter (FIND-C2.0-02);
- two consumers cannot yet read firings independently over the wire;
- the restart UAT asserts nothing yet for firings.

## Ratchet

`fired_buffer` 4 -> 4 (removal is C2.1.6). CANONICAL destructive reads 11 -> 11
(`drain_fired` is the C2.1.4/6 target). The inventory dropped 37 -> 36 because a
string-literal false positive was corrected — the ratchet demanded the shrink.

## State after C2.1.4b

```text
Tripwire definitions -> TripwireManager   (plain snapshot, no drain)
Firing evidence      -> ExecutionLog only
List progress        -> caller-owned EventSeq cursor (ecv1:...)
```

`fired_buffer` still exists physically and is still *written* by
`TripwireManager::evaluate`/`evaluate_semantic` (probe_drain calls the latter),
but nothing in the production read path reads it any more:
`TripwiresService::list` is now referenced only from its own tests. That makes
C2.1.6 a pure deletion: `TripwiresService::list`, `fired_buffer`,
`record_fired`, `drain_fired`, and the evaluate side effect.

The ratchet does not move yet (`fired_buffer` 4 -> 4, `drain_fired` 1 -> 1)
because those symbols still exist; the count drops in C2.1.6.

## State after C2.1.5

```text
firing occurrence -> ExecutionLog
firing delivery   -> EventSeq cursor
fire_count        -> ExecutionLog projection (+ completeness facts)
retention         -> never destructive, never a delivery filter
Tripwire.fire_count -> present but not authoritative anywhere
```

`fired_buffer` is still written by `TripwireManager::evaluate`/`evaluate_semantic`
(probe_drain calls the latter) and is read only by `TripwiresService::list`,
which is now referenced only from its own tests. C2.1.6 is the deletion.

## Architecture after C2.1.6

```text
Tripwire definition  -> TripwireManager
matching             -> pure (matching / matching_semantic)
source occurrence    -> ExecutionLog::Raw
firing occurrence    -> ExecutionLog::TripwireFired
fire_count           -> projection over ExecutionLog (+ completeness facts)
consumer progress    -> EventsCursorV1
legacy fired queue   -> gone
```

## DoD status (C2.1.7)

| # | Item | State |
|---|---|---|
| 1 | `TripwireFiredSummary` exposes `firing_seq` + `source_seq` | DONE |
| 2 | Phase A creates Raw S1 and firing F1 | DONE (fixture writes the same bytes the derivation produces) |
| 3 | Runtime A destroyed completely | DONE (client dropped) |
| 4 | Phase B: empty manager, same durable root | DONE |
| 5 | `observe(list)` returns exactly F1 -> S1 | DONE |
| 6 | `tripwire_id` snapshot survives without the definition | DONE |
| 7 | count reconstructed from the reopened log | DONE (`rec_c2_1_count_reopen.rs`) |
| 8 | `observe(query)` does not invent the subscription | DONE (`subscriptions: []`) |
| 9 | pre-restart cursor works post-restart | DONE (A's cursor used by C) |
| 10 | wire docs stop describing a destructive buffer | DONE |
| 11 | no `fired_buffer` / `drain_fired` / `record_fired` | DONE (comments only; ratchet 31) |
| 12 | ratchet never rises | DONE (31, down from 36) |

## FIND-C2.1-03 (open, scoped to REC-C2.2)

The durable derivation (`derive_firings_from_record` /
`derive_firings_from_event`) has **no production caller**: its only caller is
the test helper. So in a live session nothing records a firing yet; the UAT
fixture writes the bytes the derivation would.

Wiring it correctly means deriving at the point where a `Raw` record is
accepted into the log, which is the producer seam (`dual_push`) — and the
native producer has no `TripwireManager`. Deriving inside `probe_drain`
instead would violate the rule that a firing may only derive from a source
already accepted by the ExecutionLog, because `probe_drain` still reads the
legacy bus. That is REC-C2.2 work (canonical consumers off EventBus), which is
why C2.1 closes without it.
