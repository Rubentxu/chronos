# Change: m9-15 m9 12 m9 13 short sha

## Summary

Drift closure cycle for this milestone.


| Campo | Valor |
|---|---|
| Cycle ID | `m9-15-m9-12-m9-13-short-sha` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `d55cdc874edd5863f5d8a65a42396b99d090b6dc` (main HEAD before cycle) |
| Head SHA | `2441f6f3c679555dc4106ea2e8a422ed407a26a0` (fix commit on main) |
| Tag | `v0.7.13` |
| Published | 2026-09-12T10:11:00Z |

## What changed

m9-12 and m9-13 stored short 7-character SHAs in `head_sha` and
`remote_tag_peel` while `main_sha` and `base_sha` in the same files
used full 40-character SHAs. Both cycles expanded to full 40-char SHAs
in their apply-checkpoint.json + markdown files + cycles/index.md
Published SHA columns.

Standing procedure (vault-drift-sweep.md) had a check #3 that
accepted short SHAs (`git cat-file -e` succeeds for unambiguous short
SHAs). Tightened C3 to:
1. Require `head_sha == remote_tag_peel`
2. Require `peel_match == True`
3. Require `len(remote_tag_peel) == 40`

## Why this matters

m9-14's cross-check #8 (SHA existence via `git cat-file -e`) did not
catch the short-SHA drift because git accepts short SHAs when
unambiguous. C3's new `len() == 40` requirement closes this gap.

The "vacuously valid" short-SHA storage was format-inconsistent with
sibling fields (`main_sha`, `base_sha`) in the same apply-checkpoint
files, and inconsistent with the format used by all other m9 cycles
(m9-01 through m9-09 + m9-10 + m9-11 use full SHAs throughout).

## Files touched

- `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/apply-checkpoint.json` + 4 markdown files
- `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/apply-checkpoint.json` + 4 markdown files
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-12, m9-13 rows + add m9-15 + bump counters)
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive → m9-15)
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` (tighten C3)
