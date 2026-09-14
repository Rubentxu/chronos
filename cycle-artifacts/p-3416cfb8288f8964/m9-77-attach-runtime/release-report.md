# Release Report — m9-77-attach-runtime (backfill)

**Note**: Synthesized 2026-09-14 during the m9-79 archival sweep. Cycle shipped
2026-09-13T21:24:27Z, before SDDK ledger reconciliation.

## Subject

| Base | Head (verified) | Tag | Tag SHA | Tag peel | CWD | Verified at |
|---|---|---|---|---|---|---|
| `c53171a` | `9d5331659f742e3be98995dfba26ff2a635f98ed` | `v0.7.79` | `622891adef13d31486a2261f6da14d4a830574a3` | `4bd2da365d938cff734aa7f287597f854862ea58` (code commit) | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-13T21:24:00+02:00 |

## Summary

| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **PASS** | normal | A-min | 3 | 6/6 | 0 | 0 |

## Cross-checks

- CC#42: `v0.7.79` peel is `4bd2da3` (code commit, not merge `9d53316`). This
  is a preserved drift; CC#12 (`main_sha == head_sha == remote_tag_peel`) does
  not apply because the tag was authored before the SDDK ledger existed.
  Recorded in terms/index.md m9-77 row.
- CC#51: cycle-artifacts folder exists for the m9-77 row.

## Notes

- The cycle is the first of three in the m9-77..m9-79 attach batch. m9-78
  refines detach safety; m9-79 distinguishes the capability value.

## Deviations

None.