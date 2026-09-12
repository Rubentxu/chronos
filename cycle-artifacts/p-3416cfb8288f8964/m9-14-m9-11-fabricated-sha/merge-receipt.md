# Merge Receipt — m9-14-m9-11-fabricated-sha

| Campo | Valor |
|---|---|
| Cycle ID | `m9-14-m9-11-fabricated-sha` |
| Path | B-direct |
| Branch | `fix/m9-14-m9-11-fabricated-sha` |
| Base SHA | `c4f1237` (main @ start of cycle) |
| Head SHA | `3869906` |
| Tag | `v0.7.12` |
| Status | MERGED |

## Fix commit

| SHA | Subject |
|---|---|
| `3869906` | fix(m9-14): m9-11 apply-checkpoint fabricated SHA + add cross-check #8 to vault-drift-sweep |

## What was fixed

The `head_sha` and `remote_tag_peel` fields in
`cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/apply-checkpoint.json`
held `cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33` — a **fabricated**
40-char hex string that does NOT exist in the repository
(`git cat-file -e` reports "bad object").

The real m9-11 fix commit is
`cd0115fd8f942058cde109c72a975cab7ea7473c` (verified via
`git rev-list -n 1 cd0115f`). The fabricated SHA shared the same
7-char prefix (`cd0115f`) as the real one, which is why the drift was
not caught by C3 (the tag-peel check) or C7 (the change-entry SHA
check): both compare with `startswith()` semantics, which a fabricated
prefix-sharing SHA passes if no other commit in the repo uses that
prefix. The fabricated SHA was authored without verification.

`peel_match: true` in m9-11 was also a lie: the recorded
`remote_tag_peel` did not equal `git rev-list -n 1 v0.7.9` (the real
peel is `cd0115fd8f942058cde109c72a975cab7ea7473c`).

The same fabricated SHA appeared in
`.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` m9-11 row
(`Published SHA` column). Also corrected.

## Procedure extension

This cycle adds **cross-check #8** to `vault-drift-sweep.md`. The check
runs `git cat-file -e <sha>` against each of
`head_sha`, `base_sha`, `main_sha`, `remote_tag_peel` in every
`apply-checkpoint.json`. Any "bad object" output is drift.

This catches the exact failure mode m9-11 exhibited: an SHA field that
looks like a 40-char hex but does not correspond to a real commit.
