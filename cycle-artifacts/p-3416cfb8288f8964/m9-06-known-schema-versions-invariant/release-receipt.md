# Release Receipt: m9-06-known-schema-versions-invariant

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-06-known-schema-versions-invariant |
| Tag | v0.7.4 |
| Peel SHA | 3383905d93ac66b6e90b6de8cea760c1ad4e2c99 |
| Route | local |
| At | 2026-09-12T08:43:51Z |

## Evidence

```
$ git tag -a v0.7.4 3383905d93ac66b6e90b6de8cea760c1ad4e2c99 \
    -m "release(m9-06): KNOWN_BUNDLE_SCHEMA_VERSIONS compile-time invariant — remove dead_code allow, pin CURRENT∈KNOWN at build time, v0.7.4"

$ git push origin refs/tags/v0.7.4
To github.com:Rubentxu/chronos.git
 * [new tag]         v0.7.4 -> v0.7.4

$ git ls-remote origin refs/tags/v0.7.4^{}
3383905d93ac66b6e90b6de8cea760c1ad4e2c99  refs/tags/v0.7.4^{}
VERIFIED: remote tag peels to HEAD == 3383905
```

## Receipt

- `git.tag` capability: annotated tag `v0.7.4` pushed to `origin`
- Remote annotated tag peels to verified main SHA `3383905d93ac66b6e90b6de8cea760c1ad4e2c99`
- Exactly one annotated semver tag at verified SHA
- no-pending-effects: all required local Git effects complete