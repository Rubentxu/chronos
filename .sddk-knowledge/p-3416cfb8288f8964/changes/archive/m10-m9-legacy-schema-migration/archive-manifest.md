# Archive Manifest — m10-m9-legacy-schema-migration

| Field | Value |
|---|---|
| Cycle | `m10-m9-legacy-schema-migration` |
| Path | A-lite |
| Date | 2026-09-15 |
| Tag | `v0.7.109` |
| Base SHA | `9a6f52c8935057194273a4496cc89b0e1744094f` |
| Head SHA | `47d89b1a6421a690cab65647df93fae229288634` |
| Main merge SHA | `bbc65a70deac7d06628e17871e2f55cbbe8eddb8` |
| Cycle | `m10-m9-legacy-schema-migration` |

## Summary

Vault-only A-lite cycle. Closes residual drift lines in CC#30/34/36/41/43
after closing m10-cc30-cc34-cc35-cc36-cc41-cc43-schema-fix. Mechanical
schema migration across 130 files (+2051/-1990).

## CC drift closed

- CC#30 Part C (verify-findings verdict + archive-manifest Cycle): 2 → 0
- CC#34 Part C (verify-findings cycle_id): 2 → 0
- CC#36 Part C (verify-report Findings format): 2 → 0
- CC#41 Part A (verify-findings lens_summary): 6 → 0
- CC#43 Part B (verify-findings head_sha consistency): 1 → 0
- CC#38 (findings array vs table rows): clean
- CC#4 (SHA-256 cascade): 40 rows regenerated across 98 manifests

## Files changed

| File | Count |
|---|---|
| verify-findings.json migrated | 17 |
| verify-report.md ## Findings normalized | 60 |
| archive-manifest.md Cycle row added | 2 |
| archive-manifest.md SHA-256 cascade | 51 |
| Subject keys aligned (m9-78) | 1 |
| **Total files changed** | **130** |
| **Total +2051 / -1990 lines** | |

## Artifacts

| File | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m10-m9-legacy-schema-migration/apply-checkpoint.json` | computed below |
| `cycle-artifacts/p-3416cfb8288f8964/m10-m9-legacy-schema-migration/verify-findings.json` | computed below |
| `cycle-artifacts/p-3416cfb8288f8964/m10-m9-legacy-schema-migration/verify-report.md` | computed below |
| `cycle-artifacts/p-3416cfb8288f8964/m10-m9-legacy-schema-migration/merge-receipt.md` | computed below |
| `cycle-artifacts/p-3416cfb8288f8964/m10-m9-legacy-schema-migration/release-receipt.md` | computed below |
| `cycle-artifacts/p-3416cfb8288f8964/m10-m9-legacy-schema-migration/release-report.md` | computed below |
| `cycle-artifacts/p-3416cfb8288f8964/m10-m9-legacy-schema-migration/implementation-receipt.md` | computed below |
| `/tmp/migrate_v4.py` | migration script (preserved) |

## Index updates

- `terms/index.md`: no change (not applicable to this cycle)
- `cycles/index.md`: no change (Cycle ID unchanged)

## Carry-forward

None. CC#5 and CC#53 drift exposed via CC#54 was pre-existing on main;
not introduced by this cycle.

## No new specs introduced.

Bounded vault-only hardening.
