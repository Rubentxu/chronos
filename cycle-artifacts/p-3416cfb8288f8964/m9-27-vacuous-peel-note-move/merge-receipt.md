# m9-27: Merge Receipt

| Field | Value |
|---|---|
| Cycle | m9-27-vacuous-peel-note-move |
| Path | B-direct |
| Base SHA | `cb7b05c` (main @ start) |
| Branch | `fix/m9-27-vacuous-peel-note-move` |
| Merge target | `main` |
| Merged at | 2026-09-12T11:48:00Z |

## Changes merged

1. `cycle-artifacts/p-3416cfb8288f8964/m9-14-m9-11-fabricated-sha/apply-checkpoint.json`: moved free-text note from `findings_introduced.no_action` to `findings_introduced.notes`.
2. `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`: cross-check #19 added.
3. `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-27 row added, total cycles 42→43.
4. `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last archive updated.

## Why

m9-14 had a free-text observation in `findings_introduced.no_action`
describing a deferred finding about vacuous peel_match on prior
cycles. Cross-check #2 expected `no_action` to contain only valid
term IDs (like `cc-002-env-coupling-test`), but this entry had
spaces and parenthetical content, which the check treated as drift.

m9-27 moves the note to a new `findings_introduced.notes` list and
adds cross-check #19 to enforce the separation.
