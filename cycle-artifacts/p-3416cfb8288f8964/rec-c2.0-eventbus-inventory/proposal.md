# REC-C2.0 — EventBus inventory + characterization (proposal)

**Cycle**: `rec-c2.0-eventbus-inventory`
**Route**: A-min
**Base**: `main@063cbdf8`
**Companion**: `characterization.md` (the measurements)

## Why now

M1 made the durable log trustworthy (a gap-bearing segment reopens). REC-C2
now retires the second source of truth. Before deleting anything, C2.0
establishes *what the legacy coupling actually is* and arms a ratchet so the
surface can only shrink. Not one line of runtime behavior changes here.

## What ships

1. `legacy-evb-inventory.json` — 37 production occurrences, each classified
   CANONICAL / COMPATIBILITY / INTERNAL_LIVE with an owner (REC-C2.1/2/3), a
   replacement, and a removal gate.
2. `scripts/check_legacy_evb.py` — generator + ratchet with four rules (no
   new use, no stale waiver, count only shrinks, `--strict` rejects any
   CANONICAL destructive read). Wired into
   `scripts/check_architecture_contracts.py` (`--strict-legacy`).
3. Six characterizations (CHAR-C2-01..06) measuring today's behavior.

## What the measurements change

Two of the six are not structural legacy but real defects:

- `retained_until_session_end` drains the evidence and returns nothing
  (CHAR-C2-03). It must either retain for real or be rejected as
  `Unsupported`.
- `fire_count` is never incremented by any firing path (CHAR-C2-04). It must
  be derived from persisted evidence.

Both become REC-C2.1 DoD items.

## The architectural rule for C2.1

> **Persist first, derive second, fan-out last.** ExecutionLog owns
> occurrence; live channels only transport observations.

`dual_push` today does the exact opposite (CHAR-C2-01). C2.1 inverts it for
tripwire evidence and gives the firing a `source_seq` identity; C2.2 then
moves the remaining consumers off the bus; C2.3 deletes it.

## Acceptance

- `python3 scripts/check_legacy_evb.py` PASSES (37 tracked, baseline 37).
- `python3 scripts/check_architecture_contracts.py` PASSES.
- `--strict-legacy` FAILS with exactly the 7 CANONICAL destructive reads
  (the gate REC-C2 must clear).
- The six characterizations are GREEN.
- T0 / T3 unchanged.
