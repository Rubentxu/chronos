# Release Report — m9-59-apply-checkpoint-remote-tag-canonical

## Path

B-direct

## Subject

Normalize 31 apply-checkpoint.json files to use the canonical `remote_tag` field. 16 cycles (m9-03..m9-18) had legacy `tag` field (pre-m9-19 era); renamed. 15 cycles (m9-19..m9-33) had neither `tag` nor `remote_tag` (mid-era); backfilled from cycles/index.md. Add cross-check #50 to detect this drift class going forward.

## Files changed

| File pattern | Change |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-{03..18}-*/apply-checkpoint.json` (16 files) | `tag` field renamed to `remote_tag` (legacy → canonical) |
| `cycle-artifacts/p-3416cfb8288f8964/m9-{19..33}-*/apply-checkpoint.json` (15 files) | `remote_tag` backfilled from cycles/index.md |
| `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` | Added cross-check #50 |

| New files | Purpose |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-59-*/` | m9-59 cycle artifacts |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-59-*/change-entry.md` | m9-59 knowledge artifact |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-59-*/archive-manifest.md` | m9-59 archive manifest |
| `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | m9-59 row added |
| `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | timestamp updated |

## Cross-checks

| ID | Description | Status |
|---|---|---|
| #50 | apply-checkpoint.json must use canonical `remote_tag` field (not legacy `tag`) | closed by m9-59 |

## History

m9-59 closes the "missing `remote_tag` field" drift class. m9-57 schema backfill added 12 fields to 21 apply-checkpoints but missed `remote_tag` for both legacy `tag`-using cycles (m9-03..m9-18) and mid-era cycles (m9-19..m9-33) where `remote_tag` was added but not backfilled. CC#50 detects both sub-classes.
