# Change: m9-59 apply checkpoint remote tag canonical

## Summary

Post-m9-58 sweep found 31 apply-checkpoint.json files missing the canonical `remote_tag` field. 16 cycles (m9-03..m9-18) had legacy `tag` field; 15 cycles (m9-19..m9-33) had neither. m9-59 normalizes each file: rename `tag` → `remote_tag`, backfill `remote_tag` from cycles/index.md. Adds CC#50.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-59-apply-checkpoint-remote-tag-canonical` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `34341fa72f8ad003ce687493588e8b58b34d3dfb` |
| Head SHA | `d54747a6eeaeb828fbba9c7f09a219656e96561d` |

## Subject

- base_sha: `34341fa72f8ad003ce687493588e8b58b34d3dfb`
- head_sha: `d54747a6eeaeb828fbba9c7f09a219656e96561d`
- cycle: m9-59
- branch: `fix/m9-59-apply-checkpoint-remote-tag-canonical`
- date: 2026-09-12
- tag: `v0.7.61`
- findings_closed: 1 (FIND-M9-59-APPLY-CHECKPOINT-MISSING-REMOTE-TAG)
- findings_introduced.no_action: 0

## Files changed
- 31 apply-checkpoint.json files in cycle-artifacts/p-3416cfb8288f8964/m9-{03..33}-*/
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` (CC#50 added)
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/release-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/release-report.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-59-*/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-59-*/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-59 row added)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive bumped)

## Cross-checks

m9-59 added cross-check #50 (apply-checkpoint must use canonical `remote_tag` field). See `vault-drift-sweep.md`.
