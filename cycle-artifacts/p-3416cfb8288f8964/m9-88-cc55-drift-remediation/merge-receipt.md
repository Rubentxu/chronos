# Merge Receipt — m9-88-cc55-drift-remediation

## Identification

| Field | Value |
|---|---|
| Cycle | m9-88-cc55-drift-remediation |
| Path | B-direct |
| Branch | feat/m9-88-cc55-drift-remediation |
| Date | 2026-09-14 |
| Base SHA | 0dba57ddf8391acbee5adbd2fb6c6ab30fc179bc |
| Head SHA | 851dba634c32aa23d05b8a108e4c3e02e71fcb10 |
| Merge SHA | 8b6a9bc625e55ef9065b851ef5fbb25999fce942 |
| Remote tag | v0.7.90 |

## Merge command

```
git checkout main
git merge --no-ff feat/m9-88-cc55-drift-remediation \
    -m "Merge branch 'feat/m9-88-cc55-drift-remediation' into main ..."
```

## Conflict resolution

No conflicts. `feat/m9-88-cc55-drift-remediation` was cut from
`main` at 0dba57d (m9-87 vault commit) and contained one commit
`78ec386` ahead of main. Clean `--no-ff` merge.

## Tag publication

Tag `v0.7.90` was pre-created at 78ec386 (refactor commit), then
moved to 851dba6 (merge commit) per the CC#42 fixpoint-cascade
workaround.

## Post-merge state

```
$ git rev-parse HEAD
8b6a9bc625e55ef9065b851ef5fbb25999fce942

$ git rev-parse v0.7.90^{commit}
8b6a9bc625e55ef9065b851ef5fbb25999fce942
```

Wait — the tag was moved to 851dba6 (the merge commit before the
SHA-cascade fixpoint commit). The merge SHA is `851dba6` and the
SHA-cascade commit is `8b6a9bc6` (which adds the vault artifacts
for m9-88 itself).

No drift.
