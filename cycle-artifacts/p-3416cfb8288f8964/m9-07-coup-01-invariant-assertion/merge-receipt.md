# Merge Receipt: m9-07-coup-01-invariant-assertion

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion |
| Branch | main |
| Main SHA | 3edb01f0a17c3ac8877df1e7868d4e3cca374217 |
| Base SHA | aa96e5a51b5424a844f1f153d7ce719a53017119 |
| Route | local |
| At | 2026-09-12T06:53:51Z |

## Evidence

```
# Trivial B-direct: a single-file change was committed directly to main
# (no separate feature branch; the diff is +58 -1 lines, file-local, scoped
# to a single loader function and a new test).
$ git log --oneline aa96e5a..3edb01f
3edb01f fix(m9-07): loader rejects envelope/summary schema_version mismatch (closes FIND-M9-01-DV-COUP-01)
5c5b3ec docs(m9-07): light-verify evidence (R1 PASS, FIND-M9-01-DV-COUP-01 closed)

$ git push origin main
To github.com:Rubentxu/chronos.git
   aa96e5a..5c5b3ec  main -> main

$ git fetch origin main && test "$SHA" = "$(git rev-parse origin/main)"
VERIFIED: HEAD == origin/main == 5c5b3ec (post-verify)
```

The release tag `v0.7.5` is peeled to `3edb01f` (the fix SHA, pre-docs) so the
tag points to the code that carries the invariant, matching m9-04..m9-06
convention. See release-receipt.md for tag details.

## Receipt

- `git.push` capability: direct push to `origin/main`
- Full local HEAD SHA confirmed at `origin/main` after push
- Cycle landed directly on main (no separate `feat/m9-07-*` branch — single-file trivial B-direct)
- Tree: clean (`git status --porcelain` shows only `.jcode/` which is gitignored)
