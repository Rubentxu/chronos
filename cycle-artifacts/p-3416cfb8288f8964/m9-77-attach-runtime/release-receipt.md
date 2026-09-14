# Release Receipt — m9-77-attach-runtime (backfill)

| Field | Value |
|---|---|
| Tag | `v0.7.79` |
| Tag SHA (object) | `622891adef13d31486a2261f6da14d4a830574a3` |
| Tag peel (`^{commit}`) | `4bd2da365d938cff734aa7f287597f854862ea58` (code commit) |
| Merge commit | `9d5331659f742e3be98995dfba26ff2a635f98ed` |
| Code commit | `4bd2da365d938cff734aa7f287597f854862ea58` |
| Base | `c53171a` (m9-76 post-release head) |
| Branch | `feat/m9-77-session-attach-runtime` |
| Cycle | `m9-77-attach-runtime` |
| Path | A-min |
| Released at | 2026-09-13T21:24:27+02:00 |

## Peel note

Tag `v0.7.79` is on the **code commit** `4bd2da3`, not the merge commit
`9d53316`. This is preserved drift (the cycle shipped before SDDK ledger
reconciliation). CC#12 does not apply.

## Backfill

This artifact was synthesized 2026-09-14 during the m9-79 archival sweep
because `cycles/index.md` requires a cycle-artifacts folder (CC#51) for every
row, and the cycle shipped before the SDDK ledger was reconciled.