# REC-C2.1 — TripwireFired as ExecutionLog evidence (proposal)

**Cycle**: `rec-c2.1-tripwire-evidence`
**Route**: A-lite (new evidence kind + derivation seam + observe cutover)
**Base**: `main@09ef1c63` (REC-C2.0 CLOSED)
**Contract**: the identity split below is the user-approved C2.1 contract.

## The rule

> **Persist first, derive second, fan-out last.** ExecutionLog owns
> occurrence; live channels only transport observations.

## Identity split (non-negotiable)

```text
ExecutionRecord.seq            = identity of the firing as a durable fact
TripwireFiredEvidence.source_seq = authoritative identity of the cause
TripwireId                     = snapshot of which subscription fired
                                 (NOT an identity of the firing)
```

> Never use `TripwireId` or `event_id` as a substitute for either identity.

`source_seq: EventSeq` (typed), not a bare `u64`. `source_event_id` is kept as
metadata for UI/compat and is never the correlation key. `TripwireId` is not
promoted to a durable identity in this gate (its ids are runtime counters).

## Deliverables (ordered so each commit leaves the tree green)

| Step | Deliverable |
|---|---|
| C2.1.0 | planning + characterizations |
| C2.1.1 | `ExecutionKind::TripwireFired` + `TripwireFiredEvidence` ADT (appended last; `Raw=0`/`GapMarker=1` unchanged) |
| C2.1.2 | persist-first source-acceptance seam (canonical native path) |
| C2.1.3 | derive firings from an accepted `Raw` `EventSeq` + recursion barrier |
| C2.1.4 | `observe` reads firings from the log via an independent consumer cursor |
| C2.1.5 | `fire_count` derived; `retained_until_session_end` stops destroying |
| C2.1.6 | remove `fired_buffer` |
| C2.1.7 | real-process restart UAT (identity, not just count) + ratchet update |

## Ordering invariants

```text
Raw persistence            MUST precede Raw live fan-out
TripwireFired persistence  MUST precede TripwireFired notification
```

The two orderings are independent: a slow tripwire evaluation must not block
showing the raw event live. Deriving the firing may therefore happen after the
Raw fan-out, but a firing may only be **notified** once it is durable.

## On failure of the derived append

If the `Raw` record persisted but the `TripwireFired` append fails:

- do **not** revert the Raw (it is already an accepted fact);
- do **not** notify a firing that is not durable;
- emit an explicit signal (`TripwirePersistenceFailed { source_seq, tripwire_id, reason }`).

It is **not** a `Gap`: no source evidence is missing; a derivation failed.

## Recursion barrier

The evaluator dispatches on `ExecutionKind`:

```rust
match record.kind {
    ExecutionKind::Raw => evaluate_tripwires(...),
    ExecutionKind::GapMarker | ExecutionKind::TripwireFired => {}
}
```

Never a payload-tag string comparison.

## `observe`

`retention=drained` becomes **consumer-cursor** semantics:

```text
read from this consumer's cursor -> return firings -> advance THIS cursor
```

Two clients each see the evidence once, from their own position; nothing is
deleted globally. `retained_until_session_end` stops destroying: it simply
does not advance the cursor (lifecycle retention governs duration). No API
enum changes are made in this gate.

## Acceptance (DoD)

1. `TripwireFired` has a typed durable representation referencing `source_seq`.
2. A firing can only derive from a source already accepted by ExecutionLog.
3. The firing is persisted before it is notified / fanned out.
4. Restart/replay returns the same firing **identity and source_seq**, not
   merely the same count.
5. Two consumers can read firings independently.
6. `fired_buffer` is no longer a source for `observe` (ideally removed here).
7. `retained_until_session_end` stops lying (retains, or is rejected as
   `Unsupported`).
8. `fire_count` derives from persisted evidence.
9. UAT-REC-C2-02 passes across two real processes.
10. EventBus may survive temporarily as a live mirror but never decides that a
    firing "existed".

## Ratchet target

`fired_buffer` occurrences `4 -> 0`; the CANONICAL destructive-read count
drops; `--strict-legacy` still lists only what REC-C2.2/C2.3 must clear.

## Non-goals

- Deleting `dual_push` wholesale (other usages are COMPATIBILITY; C2.2/C2.3).
- A durable `SubscriptionId` (needs REC-C4/C5 persistent subscriptions).
- API enum simplification beyond stopping the destruction.
