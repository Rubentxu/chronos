# Release Report — m9-23

**Cycle**: m9-23-cycle-id-strip-prefix
**Path**: B-direct
**Tag**: v0.7.21

| Field | Value |
|---|---|
| Cycle | m9-23-cycle-id-strip-prefix |
| Path | B-direct (T0 + light-verify) |
| Cycles closed | 39 → 40 |
| Tags | v0.7.20 → v0.7.21 |

## Drift class closed

| Class | Cycles | Fix |
|---|---|---|
| `cycle_id` with workspace prefix | m9-03..m9-18 (16 cycles) | Strip prefix |

## Cross-check

Cross-check #16 added to `vault-drift-sweep.md`.

## Tier

B-direct: T0 only (file-local mechanical edit).
## Cross-checks

- C1-C10: pass

## What changed

Drift closure cycle. See release-receipt.md and change-entry.md for details.

## Verification

- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
