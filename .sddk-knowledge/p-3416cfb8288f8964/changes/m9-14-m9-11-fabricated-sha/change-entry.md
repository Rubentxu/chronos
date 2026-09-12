# Change: m9-14 m9 11 fabricated sha

## Summary

Drift closure cycle for this milestone.


| Campo | Valor |
|---|---|
| Cycle ID | `m9-14-m9-11-fabricated-sha` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `c4f1237f878c900208e9506f8127b77d43a066af` (main HEAD before cycle) |
| Head SHA | `38699061891b76f90ef316914d3ba15d6eb53f83` (fix commit on main) |
| Tag | `v0.7.12` |
| Published | 2026-09-12T10:03:00Z |

## What changed

m9-11's `apply-checkpoint.json` had a fabricated `head_sha` /
`remote_tag_peel` (`cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33`) that
does not exist in the repository. The real m9-11 fix commit is
`cd0115fd8f942058cde109c72a975cab7ea7473c`. Same fabrication appeared
in `cycles/index.md` m9-11 row. Both corrected.

Standing procedure (vault-drift-sweep.md) lacked a cross-check for
"apply-checkpoint SHA fields exist in repo". Cross-check #8 added
(uses `git cat-file -e`).

## Why this matters

m9-11 was authored in a previous session by an LLM that
appeared to invent `cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33` instead
of looking up the real m9-11 commit via `git rev-list -n 1 cd0115f`.
The fabrication passed C3 and C7 because both checks use prefix
comparison (`startswith`) and the fabricated SHA shared the same
7-char prefix as the real one.

`peel_match: true` was a lie: `remote_tag_peel` did not equal
`git rev-list -n 1 v0.7.9`.

C8 closes the gap by asking git itself whether the SHA is real.

## Files touched

- `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/apply-checkpoint.json` (head_sha, remote_tag_peel)
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-11 row, add m9-14 row, bump counters)
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive → m9-14)
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` (add cross-check #8)
