# Archive Manifest — m9-32

| Field | Value |
|---|---|
| Cycle | m9-32-verify-report-cross-checks-backfill |
| Head SHA | `b3bfa59e85b1145c81ae84ce7cac6be6238b2a58` |
| Base SHA | `128a224dff9924e087647013df32edeb965efa75` |
| Tag | `v0.7.30` |
| Path | B-direct |
| Date | 2026-09-12 |

## Summary

Adds `## Cross-checks` section to 25 verify-report.md files (m9-03
through m9-27) that were authored before the cross-check annotation
format was introduced in m9-28. Adds cross-check #24 to
vault-drift-sweep.md enforcing `## Cross-checks` section in all
verify-report.md files.

## Artifact index

| Path | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-32-verify-report-cross-checks-backfill/apply-checkpoint.json` | `d963b4e27d1b6166741a321804345e240a376287af2763bb4d12056a3545e4ee` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-32-verify-report-cross-checks-backfill/merge-receipt.md` | `1b1e9ed66a92697736ee4f0dbc9dc36a6da0e6c46ebff25dec8a1f8248583329` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-32-verify-report-cross-checks-backfill/release-receipt.md` | `c6b384a05e46aa953ba76e4238b43e3d29908a95b9b6f3ad943f856ce9406c8e` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-32-verify-report-cross-checks-backfill/release-report.md` | `5e0869f1c2e1f402c21810e833cf87d385a863f93f1610652cc181b194c5860a` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-32-verify-report-cross-checks-backfill/verify-findings.json` | `cb4024ebf886266b2f21c2f0e194bbb3e4386545666ae2562af36254fd784554` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-32-verify-report-cross-checks-backfill/verify-report.md` | `579ead0fa63468650179b16c7d02a123010030bcdf8e703980ee58aac6ada264` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-32-verify-report-cross-checks-backfill/change-entry.md` | `1ced6748e294b91b5e3eb849fa4b8c3d0a334a4c9c9a28080815705aa91cdb80` |

## Evidence bindings

Vault metadata only. Each cycle artifact listed with its computed
SHA-256 in the Artifact index above.

## Drift summary

25 verify-report.md files (m9-03..m9-27) were missing the canonical
`## Cross-checks` section. m9-32 backfills it for all 25 prior cycles.

## Cross-checks added

- #24 (`vault-drift-sweep.md`): verify-report.md must have
  `## Cross-checks` section.
