# Release Receipt: m9-04-side-table-key-layout

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-04-side-table-key-layout |
| Tag | v0.7.2 |
| Peel SHA | d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc |
| Route | local |
| At | 2026-09-12T08:21:00Z |

## Evidence

```
$ git tag -a v0.7.2 d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc \
    -m "release(m9-04): side-table key layout v3 (blake3 prefix, 20-byte fixed keys) + range-scan friendly + CLI integration tests, v0.7.2"

$ git push origin refs/tags/v0.7.2
To github.com:Rubentxu/chronos.git
 * [new tag]         v0.7.2 -> v0.7.2

$ git ls-remote origin refs/tags/v0.7.2^{}
d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc  refs/tags/v0.7.2^{}
VERIFIED: remote tag peels to HEAD == d6b3b8c
```

## Receipt

- `git.tag` capability: annotated tag `v0.7.2` pushed to `origin`
- Remote annotated tag peels to verified main SHA `d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc`
- Exactly one annotated semver tag at verified SHA
- no-pending-effects: all required local Git effects complete