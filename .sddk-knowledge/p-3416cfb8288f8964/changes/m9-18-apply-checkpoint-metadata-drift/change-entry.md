# Change: m9-18 apply checkpoint metadata drift

## Summary

Drift closure cycle for this milestone.


| Campo | Valor |
|---|---|
| Cycle ID | `m9-18-apply-checkpoint-metadata-drift` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `cc995cdd2cf41bd0638a548fc3db169763bfc38d` (main HEAD before cycle) |
| Head SHA | `6dce3736df06d4fe09db861ad43a3667c0f0bc25` (fix commit on main) |
| Tag | `v0.7.16` |
| Published | 2026-09-12T10:44:30Z |

## What changed

The apply-checkpoint.json schema evolved across the m9 cycle series
without retroactive application to prior cycles. m9-18 backfills
3 classes of drift:

1. **archived_at backfill** (3 cycles): m9-14 introduced the field
   but m9-11, m9-12, m9-13 had `archived_at: null` even though
   archive-manifest.md files existed. Backfilled from each cycle's
   archive-manifest first-commit-time.
2. **findings_introduced field** (7 cycles): m9-09 introduced the
   field but m9-03, m9-05..m9-10 (7 cycles) were never updated.
   Added `{"no_action": []}` to all.
3. **status normalization** (8 cycles): pre-m9-11 used lowercase
   `"closed"`. m9-11+ uses uppercase `"CLOSED"`. Normalized 8 cycles.

Adds cross-check #11 to vault-drift-sweep.md enforcing all three
classes going forward.

## Files touched

- 11 apply-checkpoint.json files (m9-03 through m9-13)
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (add m9-18 row + bump counters)
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive → m9-18)
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` (cross-check #11)
