# Merge Receipt — m9-90-stale-branches-cleanup

## Identification

| Field | Value |
|---|---|
| Cycle | m9-90-stale-branches-cleanup |
| Branch | chore/m9-90-stale-branches-cleanup |
| Date | 2026-09-14 |

## Merge details

| Field | Value |
|---|---|
| Merge type | --no-ff merge into main |
| Merge commit | 2184975a93b43ea1bbd2681dead79dfb4476fef7 *(cycle source; advanced to 5cdb4e1a through SHA cascade)* |
| Base SHA | 20e2649822c0c24509ff6419a48fe3113591ecf0 |
| Head SHA | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d *(re-anchored to immutable v0.7.92 tag location by m9-92)* |
| Main SHA post-merge | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d *(post-cascade HEAD; aligns with immutable tag)* |

## SHAs

| Field | Value |
|---|---|
| Branch | chore/m9-90-stale-branches-cleanup |
| Date | 2026-09-14 |
| Base SHA | 20e2649822c0c24509ff6419a48fe3113591ecf0 |
| Head SHA | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d *(re-anchored to immutable v0.7.92 tag location by m9-92)* |

## Notes

- Branch `chore/m9-90-stale-branches-cleanup` was merged into `main`
  with `--no-ff` (creates merge commit `56f93d1`). Per repo convention
  all cycles use `--no-ff` so the merge is a reviewable unit.
- The cycle's published head (where the tag lives) was advanced
  through `56f93d1` → `42ee5df` → `2184975` → `5cdb4e1` (post-push
  HEAD) via the CC#42 fixpoint-cascade workaround documented in the
  m9-83 handoff. m9-92 (CC#34 + CC#42 cleanup) re-anchored the
  documented Head SHA from 2184975a to 5cdb4e1a to match the
  immutable tag location and CC#3 era-awareness.
- Branch was deleted after the cycle closed.
