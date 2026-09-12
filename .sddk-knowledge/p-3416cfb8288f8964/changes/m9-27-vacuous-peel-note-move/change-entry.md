# Change: m9-27 free-text note move

| Field | Value |
|---|---|
| Cycle | m9-27-vacuous-peel-note-move |
| Base SHA | `cb7b05c` |
| Head SHA | `91338e3` |
| Tag | `v0.7.25` (peels to `91338e3`) |
| Path | B-direct |
| Date | 2026-09-12 |
| Author | jcode (auto-mode B-direct) |

## Summary

m9-14 had a free-text note in `findings_introduced.no_action`
describing a deferred observation about vacuous peel_match on prior
cycles. Cross-check #2 expected no_action to contain only valid
term IDs.

m9-27 moves the note to a new `findings_introduced.notes` list.

## Cross-check

Cross-check #19 added.

## Verification

- T0 gate: PASS
- All 19 cross-checks: PASS
