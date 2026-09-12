# Release Report — m9-27

**Cycle**: m9-27-vacuous-peel-note-move
**Path**: B-direct
**Tag**: v0.7.25

| Field | Value |
|---|---|
| Cycle | m9-27-vacuous-peel-note-move |
| Path | B-direct (T0 + light-verify) |
| Cycles closed | 43 → 44 |
| Tags | v0.7.24 → v0.7.25 |

## Drift class closed

| Class | Cycles | Fix |
|---|---|---|
| Free-text note in `findings_introduced.no_action` | m9-14 (1 cycle) | Move to `notes` key |

## Tier

B-direct: T0 only (file-local mechanical edit).
## Cross-checks

- C1-C10: pass

## What changed

Drift closure cycle. See release-receipt.md and change-entry.md for details.

## Verification

- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
