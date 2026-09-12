# Archive Manifest — m9-35

| Field | Value |
|---|---|
| Cycle | m9-35-release-report-title-format-normalize |
| Head SHA | `a0b0d560365315b22012325e9d416f3ef1998faa` |
| Base SHA | `dbea58e61921eecb4b1b90f6f94dad21fea2e440` |
| Tag | `v0.7.33` |
| Path | B-direct |
| Date | 2026-09-12 |

## Summary

Normalized 9 release-report.md files (m9-19..m9-27) from
`# m9-NN: Release Report` to the canonical `# Release Report — m9-NN`
format. m9-28+ already used the canonical format. Brings all 33 m9
cycles into alignment.

Added cross-check #27 to vault-drift-sweep.md to enforce the canonical
title format going forward.

## Cross-checks

- C27: pass (after fixes applied)
- C1-C26: pass

## Evidence bindings

- `cycle-artifacts/p-3416cfb8288f8964/m9-35-release-report-title-format-normalize/apply-checkpoint.json` → sha256: `e4f04c492c1ec504c1790e4be09ed1c9064098bbdd90a0492910b55b2a683532`
- `cycle-artifacts/p-3416cfb8288f8964/m9-35-release-report-title-format-normalize/merge-receipt.md` → sha256: `52876dd4aa3450590f647b38140c2e901983102e854b91fdb74e07c9a9645b4c`
- `cycle-artifacts/p-3416cfb8288f8964/m9-35-release-report-title-format-normalize/release-receipt.md` → sha256: `9b4e5a494d25708a2d1653427f84f8bf008ec4e1480f861b0588e5750bff22a2`
- `cycle-artifacts/p-3416cfb8288f8964/m9-35-release-report-title-format-normalize/release-report.md` → sha256: `37da4605efba25f747d89de0531ca9f98e15c059609e96c09d0cf1ac353cc650`
- `cycle-artifacts/p-3416cfb8288f8964/m9-35-release-report-title-format-normalize/verify-findings.json` → sha256: `3593d560347f26036f2414735c2d21c1b926b731754d9e467497d551f6b83b1b`
- `cycle-artifacts/p-3416cfb8288f8964/m9-35-release-report-title-format-normalize/verify-report.md` → sha256: `9ee620607a26b4de4b59727d7bf62bf6ebbe9dcbb0d4e2c7ce366b56d6d3f94e`
