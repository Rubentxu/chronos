# Verify Report — m9-59

**Cycle**: m9-59-apply-checkpoint-remote-tag-canonical
**Path**: B-direct

## Summary

31 apply-checkpoint.json files were missing the canonical `remote_tag` field:
- 16 cycles had legacy `tag` field (m9-03..m9-18, pre-m9-19 era)
- 15 cycles had neither field (m9-19..m9-33, mid-era)

Each file normalized. Legacy `tag` renamed to canonical `remote_tag`. Missing `remote_tag` backfilled from cycles/index.md (4th column). Cross-check #50 added to detect this drift class.

## Subject

| Base | Head (final) | Dirty diff digest | CWD | Verified at |
|---|---|---|---|---|
| `34341fa` | `d54747a6eeaeb828fbba9c7f09a219656e96561d` | `sha256:e3b0c44...` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-12T18:46:00Z |

Files changed (31 apply-checkpoint.json + 1 maintenance doc):

| Group | Count | Pattern |
|---|---|---|
| Legacy `tag` → `remote_tag` (rename) | 16 | m9-03..m9-18 |
| `remote_tag` backfilled | 15 | m9-19..m9-33 |

## Findings

| ID | Severity | Category | Description | Status |
|---|---|---|---|---|
| F1 | medium | vault_hygiene.apply_checkpoint_schema_drift | 31 apply-checkpoint.json files missing canonical `remote_tag` field; 16 had legacy `tag`, 15 had neither | RESOLVED |

## Files Inventory

| Path |
|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/apply-checkpoint.json` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/verify-findings.json` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/verify-report.md` (this file) |
| `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/merge-receipt.md` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/release-receipt.md` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/release-report.md` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-59-*/change-entry.md` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-59-*/archive-manifest.md` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-{03..33}-*/apply-checkpoint.json` (31 modified) |
| `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` (modified, CC#50 added) |

## Cross-checks

- CC#50: passes (0 DRIFT lines after fix)
- CC#48 meta-check: passes (0 DRIFT lines across all 50 CCs)

## Verification

- C1-C49: pass
- C50: pass (after fix)

## History

m9-59 was discovered during a post-m9-58 sweep. m9-57 schema backfill (which added 12 fields to 21 apply-checkpoints) missed `remote_tag`, leaving 31 cycles either using legacy `tag` or missing the field entirely. m9-57 only backfilled cycles that didn't have ANY tag-related field; it didn't detect that 16 cycles had a non-canonical `tag` field. CC#50 detects both sub-classes explicitly.
