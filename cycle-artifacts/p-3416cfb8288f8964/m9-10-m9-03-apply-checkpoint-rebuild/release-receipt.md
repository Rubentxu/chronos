# Release Receipt: m9-10-m9-03-apply-checkpoint-rebuild

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild |
| Tag | v0.7.8 |
| Peel SHA | 69f200e2144bea2cd305c38903feb4814fb38806 |
| Route | local |
| At | 2026-09-12T07:22:36Z |

## Evidence

```
$ git tag -a v0.7.8 69f200e2144bea2cd305c38903feb4814fb38806 \
    -m "release(m9-10): rebuild m9-03 apply-checkpoint (vault-reorg gap), v0.7.8"

$ git push origin refs/tags/v0.7.8
To github.com:Rubentxu/chronos.git
 * [new tag]         v0.7.8 -> v0.7.8

$ git ls-remote origin refs/tags/v0.7.8^{}
69f200e2144bea2cd305c38903feb4814fb38806  refs/tags/v0.7.8^{}
VERIFIED: remote tag peels to HEAD == 69f200e2144bea2cd305c38903feb4814fb38806
```

## Receipt

- `git.tag` capability: annotated tag `v0.7.8` pushed to `origin`
- Remote annotated tag peels to verified main SHA `69f200e2144bea2cd305c38903feb4814fb38806`
- Exactly one annotated semver tag at verified SHA
- no-pending-effects: all required local Git effects complete
- Tag peeled to the fix commit (`69f200e`) rather than the docs commit (`6dc4609`) so the tag points to the state that carries the rebuilt artifact. This matches the m9-04..m9-09 convention where the peel SHA carries the runtime semantics of the release.
