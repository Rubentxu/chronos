# REC-C2.2 — tasks

**Companion**: `proposal.md`

| Step | State |
|---|---|
| C2.2.0 accepted-Raw seam + FIND-C2.1-03 | DONE |
| C2.2.1 spawn/attach producer convergence | DONE (attach now uses the same seam) |
| C2.2.2 `probe_drain` not a destructive authority | PENDING |
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
