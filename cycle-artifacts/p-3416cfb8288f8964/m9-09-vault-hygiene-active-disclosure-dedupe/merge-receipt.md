# Merge Receipt: m9-09-vault-hygiene-active-disclosure-dedupe

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe |
| Branch | main |
| Main SHA | 07e731d61424160c4f67d769db00a171837ba62c |
| Base SHA | c183ad1822423419ffb7d5f92a8d5ac46423d653 |
| Route | local |
| At | 2026-09-12T07:14:57Z |

## Evidence

```
# Trivial B-direct: a one-line vault drift fix was committed directly to main
# (no separate feature branch; the diff is -1 line, file-local to
# `.sddk-knowledge/.../terms/index.md`).
$ git log --oneline c183ad1..07e731d
07e731d fix(m9-09): remove duplicate m9-01-R4 from active disclosures (already terminated by m9-06)
bc94a33 docs(m9-09): light-verify evidence (R1 PASS, vault drift closed)

$ git push origin main
To github.com:Rubentxu/chronos.git
   c183ad1..bc94a33  main -> main

$ git fetch origin main && test "$SHA" = "$(git rev-parse origin/main)"
VERIFIED: HEAD == origin/main == bc94a33 (post-verify)
```

The release tag `v0.7.7` is peeled to `07e731d61424160c4f67d769db00a171837ba62c` (the fix SHA, pre-docs) so the tag points to the state that carries the corrected index, matching m9-04..m9-08 convention.

## Receipt

- `git.push` capability: direct push to `origin/main`
- Full local HEAD SHA confirmed at `origin/main` after push
- Cycle landed directly on main (no separate `feat/m9-09-*` branch — one-line vault hygiene B-direct)
- Tree: clean (`git status --porcelain` shows only `.jcode/` which is gitignored)
