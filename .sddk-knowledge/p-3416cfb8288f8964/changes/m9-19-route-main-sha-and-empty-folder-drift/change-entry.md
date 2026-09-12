# Change: m9-19 Route + main_sha + empty folder drift fix


## Subject

- base_sha: `735c57b7178c93ea25f9cb603a3cb97b9ca7f81e`
- head_sha: `ec58934689b73a6a78cb65b8ece96c6cdda13b35`
- cycle: m9-19
- tag: `v0.7.17`
- route: B-direct
- date: 2026-09-12


## Summary

Three classes of drift closed in a single cycle:

1. **`route` field normalization (8 cycles)**: pre-m9-11 cycles
   (`m9-03` through `m9-10`) had `apply-checkpoint.json` `route` set
   to the lowercase placeholder `"local"`. m9-11+ uses descriptive
   path labels like `"B-direct"` / `"A-min"` / `"A-lite"` / `"A-full"`.
   All 8 cycles normalized to `"B-direct"`.

2. **`main_sha` convention (3 cycles)**: m9-11, m9-12, m9-13 had
   `main_sha` set to `base_sha` (pre-cycle HEAD) instead of `head_sha`
   (post-cycle HEAD). This violates the m9-04+ convention documented
   in `vault-drift-sweep.md` cross-check #3. All 3 cycles fixed.

3. **Empty cycle folder cleanup (1 folder)**: the untracked folder
   `cycle-artifacts/p-3416cfb8288f8964/m9-11-m9-04-findings-closed-drift-fix/`
   was a leftover from an aborted false-positive investigation in a
   prior session. Removed via `rmdir`.

## Cross-check

Cross-check #12 added to
`.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`
enforcing all three drift classes.

## Verification

- T0 gate: `cargo fmt --check` + `cargo clippy -- -D warnings` → PASS
- All 12 cross-checks → PASS (0 drift)
- All cycle artifacts created under
  `cycle-artifacts/p-3416cfb8288f8964/m9-19-route-main-sha-and-empty-folder-drift/`

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-19-route-main-sha-and-empty-folder-drift/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-19-route-main-sha-and-empty-folder-drift/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-19-route-main-sha-and-empty-folder-drift/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-19-route-main-sha-and-empty-folder-drift/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-19-route-main-sha-and-empty-folder-drift/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-19-route-main-sha-and-empty-folder-drift/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-19-route-main-sha-and-empty-folder-drift/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-19-route-main-sha-and-empty-folder-drift/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Lessons

Pre-m9-11 cycles were authored under a different schema (`route:
"local"`, no `findings_introduced` field, lowercase `status`). Each
post-m9-11 cycle that introduces a new schema field has to do a
backfill sweep of all prior cycles. The new cross-check #12 is
designed to catch this exact class of drift so that future schema
evolutions only need to fix the cycle that introduces them, not
backfill all prior ones.

The `main_sha != head_sha` bug in m9-11/12/13 is a more interesting
case: I introduced it when authoring those cycles myself, before the
`main_sha == head_sha` convention was formalized in m9-04+ docs. The
fix is mechanical and the cross-check prevents future me from
re-introducing it.
