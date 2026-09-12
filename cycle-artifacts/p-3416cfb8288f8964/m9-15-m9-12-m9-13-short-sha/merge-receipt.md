# Merge Receipt — m9-15-m9-12-m9-13-short-sha

| Campo | Valor |
|---|---|
| Cycle ID | `m9-15-m9-12-m9-13-short-sha` |
| Path | B-direct |
| Branch | `fix/m9-15-m9-12-m9-13-short-sha` |
| Base SHA | `d55cdc874edd5863f5d8a65a42396b99d090b6dc` (main @ start of cycle) |
| Head SHA | `2441f6f3c679555dc4106ea2e8a422ed407a26a0` |
| Tag | `v0.7.13` |
| Status | MERGED |

## Fix commit

| SHA | Subject |
|---|---|
| `2441f6f3c679555dc4106ea2e8a422ed407a26a0` | fix(m9-15): expand m9-12 + m9-13 short SHAs to full 40-char + tighten C3 |

## What was fixed

Two prior cycles (m9-12 and m9-13) stored 7-character short SHAs in
the `head_sha` and `remote_tag_peel` fields of their
`apply-checkpoint.json` files, while `main_sha` and `base_sha` in the
same files used full 40-character SHAs (inconsistent format).

- m9-12: `head_sha = "0012f12"` → `"0012f1242cef949efc4cbd4c8d419a135ee3cf8a"`
- m9-13: `head_sha = "26848cf"` → `"26848cf8b26340d3fde99a3a7f398f2873943982"`

`peel_match: true` was technically correct because git accepts short
SHAs when unambiguous, but the format was inconsistent with sibling
fields (`main_sha`, `base_sha`) which used full 40-char SHAs.

All markdown references (merge-receipt, release-receipt, release-report,
verify-report) in the affected cycle folders were also expanded to
full SHAs, plus the `cycles/index.md` `Published SHA` columns for both
cycles.

## Why this matters

Short SHAs (7 chars) are ambiguous when two commits in the repo share
a common prefix. Cross-check #8 (added by m9-14) uses `git cat-file -e`
to verify SHA existence, but this succeeds for short SHAs whenever the
prefix is unique in the repo. So short-SHA storage is "vacuously
valid" but format-inconsistent with sibling fields.

m9-15 tightens cross-check #3 in `vault-drift-sweep.md` to explicitly
require:
1. `head_sha == remote_tag_peel`
2. `peel_match == True`
3. `len(remote_tag_peel) == 40` (the new check that catches this drift)

This makes short-SHA storage a C3 drift instead of an undocumented
inconsistency.
