# Change: m9-27 free-text note move


## Subject

- base_sha: `cb7b05c8fe65b9de24ccd7f7f447374fcfd95f11`
- head_sha: `91338e3b2fc58835662776507bde5308ec36568b`
- cycle: m9-27
- tag: `v0.7.25`
- route: B-direct
- date: 2026-09-12


## Summary

m9-14 had a free-text note in `findings_introduced.no_action`
describing a deferred observation about vacuous peel_match on prior
cycles. Cross-check #2 expected no_action to contain only valid
term IDs.

m9-27 moves the note to a new `findings_introduced.notes` list.

## Cross-check

Cross-check #19 added.

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-27-vacuous-peel-note-move/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-27-vacuous-peel-note-move/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-27-vacuous-peel-note-move/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-27-vacuous-peel-note-move/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-27-vacuous-peel-note-move/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-27-vacuous-peel-note-move/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-27-vacuous-peel-note-move/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-27-vacuous-peel-note-move/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Verification

- T0 gate: PASS
- All 19 cross-checks: PASS
