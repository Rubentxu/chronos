# Release Receipt: m9-07-coup-01-invariant-assertion

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion |
| Tag | v0.7.5 |
| Peel SHA | 3edb01f0a17c3ac8877df1e7868d4e3cca374217 |
| Route | local |
| At | 2026-09-12T06:54:48Z |

## Evidence

```
$ git tag -a v0.7.5 3edb01f0a17c3ac8877df1e7868d4e3cca374217 \
    -m "release(m9-07): loader rejects envelope/summary schema_version mismatch — closes FIND-M9-01-DV-COUP-01, v0.7.5"

$ git push origin refs/tags/v0.7.5
To github.com:Rubentxu/chronos.git
 * [new tag]         v0.7.5 -> v0.7.5

$ git ls-remote origin refs/tags/v0.7.5^{}
3edb01f0a17c3ac8877df1e7868d4e3cca374217  refs/tags/v0.7.5^{}
VERIFIED: remote tag peels to HEAD == 3edb01f0a17c3ac8877df1e7868d4e3cca374217
```

## Receipt

- `git.tag` capability: annotated tag `v0.7.5` pushed to `origin`
- Remote annotated tag peels to verified main SHA `3edb01f0a17c3ac8877df1e7868d4e3cca374217`
- Exactly one annotated semver tag at verified SHA
- no-pending-effects: all required local Git effects complete
- Tag peeled to the fix commit (`3edb01f`) rather than the docs commit (`5c5b3ec`) so the tag points to the code that carries the invariant. This matches the m9-04..m9-06 convention where the peel SHA carries the runtime semantics of the release, not the docstring additions.
