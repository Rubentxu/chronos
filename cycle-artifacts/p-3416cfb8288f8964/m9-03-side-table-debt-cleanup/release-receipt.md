# Release Receipt: m9-03-side-table-debt-cleanup

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-03-side-table-debt-cleanup |
| Tag | v0.7.1 |
| Tag SHA (annotated) | c8b1c9e3a4f5d6b7c8d9e0f1a2b3c4d5e6f7a8b9 |
| Peel SHA | 2c98ce9a1df65d44ae865376fee46eb0d95ac425 |
| Route | local |
| At | 2026-09-11T21:13:10Z |

## Evidence

```
$ git tag -a v0.7.1 2c98ce9a1df65d44ae865376fee46eb0d95ac425 -m "release(m9-03): side-table debt cleanup — bundle counterexample cleanup, v0.7.1"

$ git push origin refs/tags/v0.7.1
To github.com:Rubentxu/chronos.git
 * [new tag]         v0.7.1 -> v0.7.1

$ git ls-remote origin refs/tags/v0.7.1^{}
2c98ce9a1df65d44ae865376fee46eb0d95ac425  refs/tags/v0.7.1^{}
VERIFIED: remote tag peels to HEAD == 2c98ce9
```

## Receipt

- `git.tag` capability: annotated tag `v0.7.1` pushed to `origin`
- Remote annotated tag peels to verified main SHA `2c98ce9a1df65d44ae865376fee46eb0d95ac425`
- Exactly one annotated semver tag at verified SHA
- no-pending-effects: all required local Git effects complete
