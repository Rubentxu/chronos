# Release Receipt: m9-09-vault-hygiene-active-disclosure-dedupe

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe |
| Tag | v0.7.7 |
| Peel SHA | 07e731d61424160c4f67d769db00a171837ba62c |
| Route | local |
| At | 2026-09-12T07:15:18Z |

## Evidence

```
$ git tag -a v0.7.7 07e731d61424160c4f67d769db00a171837ba62c \
    -m "release(m9-09): remove duplicate m9-01-R4 from active disclosures — vault hygiene, v0.7.7"

$ git push origin refs/tags/v0.7.7
To github.com:Rubentxu/chronos.git
 * [new tag]         v0.7.7 -> v0.7.7

$ git ls-remote origin refs/tags/v0.7.7^{}
07e731d61424160c4f67d769db00a171837ba62c  refs/tags/v0.7.7^{}
VERIFIED: remote tag peels to HEAD == 07e731d61424160c4f67d769db00a171837ba62c
```

## Receipt

- `git.tag` capability: annotated tag `v0.7.7` pushed to `origin`
- Remote annotated tag peels to verified main SHA `07e731d61424160c4f67d769db00a171837ba62c`
- Exactly one annotated semver tag at verified SHA
- no-pending-effects: all required local Git effects complete
- Tag peeled to the fix commit (`07e731d`) rather than the docs commit (`bc94a33`) so the tag points to the state that carries the corrected vault index. This matches the m9-04..m9-08 convention where the peel SHA carries the runtime semantics of the release.
