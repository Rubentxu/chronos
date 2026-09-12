# Release Receipt: m9-05-side-table-overeng-cleanup

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup |
| Tag | v0.7.3 |
| Peel SHA | 07d01d5869ff6e3ffed29315b761e6e476f3b70d |
| Route | local |
| At | 2026-09-12T08:40:00Z |

## Evidence

```
$ git tag -a v0.7.3 07d01d5869ff6e3ffed29315b761e6e476f3b70d \
    -m "release(m9-05): side-table overeng cleanup — extract decode/range helpers, drop events_count Option, narrow pub visibility with chokepoints, v0.7.3"

$ git push origin refs/tags/v0.7.3
To github.com:Rubentxu/chronos.git
 * [new tag]         v0.7.3 -> v0.7.3

$ git ls-remote origin refs/tags/v0.7.3^{}
07d01d5869ff6e3ffed29315b761e6e476f3b70d  refs/tags/v0.7.3^{}
VERIFIED: remote tag peels to HEAD == 07d01d5
```

## Receipt

- `git.tag` capability: annotated tag `v0.7.3` pushed to `origin`
- Remote annotated tag peels to verified main SHA `07d01d5869ff6e3ffed29315b761e6e476f3b70d`
- Exactly one annotated semver tag at verified SHA
- no-pending-effects: all required local Git effects complete