# Release Report — m9-78-safe-attach-detach (backfill)

**Path**: B-direct
**Cycle**: m9-78-safe-attach-detach
**Note**: Synthesized 2026-09-14 during the m9-79 archival sweep. Cycle shipped
2026-09-13T22:02:06Z, before SDDK ledger reconciliation.

## Subject

| Base | Head (verified) | Tag | Tag SHA | Tag peel | CWD | Verified at |
|---|---|---|---|---|---|---|
| `9d5331659f742e3be98995dfba26ff2a635f98ed` | `009b75037357d069775ad4d7a0661684e27fc9ca` | `v0.7.80` | `4cd169a49db2075169ddc73bc7dc84a2137ea873` | `a2c70fe9b68120093313c88386d1ee6c53c5391c` (code commit) | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-13T22:02:00+02:00 |

## Summary

| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **PASS** | normal | B-direct | 2 | 6/6 | 0 | 0 |

## Cross-checks

- CC#42: `v0.7.80` peel is `a2c70fe` (code commit, not merge `009b750`). Preserved drift.
- CC#51: cycle-artifacts folder exists for the m9-78 row.

## Notes

- The cycle is the second of three in the m9-77..m9-79 attach batch. m9-79
  refines the capability value.