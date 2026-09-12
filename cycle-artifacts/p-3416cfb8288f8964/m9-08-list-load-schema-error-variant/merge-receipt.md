# Merge Receipt: m9-08-list-load-schema-error-variant

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-08-list-load-schema-error-variant |
| Branch | main |
| Main SHA | d89862bbe67256cf6274be1d71ee7f8857cb8808 |
| Base SHA | 50553969309b945156c239fd5a5a7f794431df21 |
| Route | local |
| At | 2026-09-12T07:02:39Z |

## Evidence

```
# Trivial B-direct: a two-file change was committed directly to main
# (no separate feature branch; the diff is +67 -5 lines, file-local to
# chronos-store, scoped to a new error variant and a 2-line loader swap).
$ git log --oneline 5055396..d89862b
d89862b fix(m9-08): dedicated StoreError::SchemaTooNew variant (closes FIND-M9-01-DV-COUP-02)
cb47fe8 docs(m9-08): light-verify evidence (R1 PASS, FIND-M9-01-DV-COUP-02 closed)

$ git push origin main
To github.com:Rubentxu/chronos.git
   5055396..cb47fe8  main -> main

$ git fetch origin main && test "$SHA" = "$(git rev-parse origin/main)"
VERIFIED: HEAD == origin/main == cb47fe8 (post-verify)
```

The release tag `v0.7.6` is peeled to `d89862b` (the fix SHA, pre-docs) so the
tag points to the code that carries the new variant, matching m9-04..m9-07
convention. See release-receipt.md for tag details.

## Receipt

- `git.push` capability: direct push to `origin/main`
- Full local HEAD SHA confirmed at `origin/main` after push
- Cycle landed directly on main (no separate `feat/m9-08-*` branch — two-file trivial B-direct)
- Tree: clean (`git status --porcelain` shows only `.jcode/` which is gitignored)
