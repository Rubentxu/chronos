# Release Receipt: m9-08-list-load-schema-error-variant

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-08-list-load-schema-error-variant |
| Tag | v0.7.6 |
| Peel SHA | d89862bbe67256cf6274be1d71ee7f8857cb8808 |
| Route | local |
| At | 2026-09-12T07:03:02Z |

## Evidence

```
$ git tag -a v0.7.6 d89862bbe67256cf6274be1d71ee7f8857cb8808 \
    -m "release(m9-08): dedicated StoreError::SchemaTooNew variant — closes FIND-M9-01-DV-COUP-02, v0.7.6"

$ git push origin refs/tags/v0.7.6
To github.com:Rubentxu/chronos.git
 * [new tag]         v0.7.6 -> v0.7.6

$ git ls-remote origin refs/tags/v0.7.6^{}
d89862bbe67256cf6274be1d71ee7f8857cb8808  refs/tags/v0.7.6^{}
VERIFIED: remote tag peels to HEAD == d89862bbe67256cf6274be1d71ee7f8857cb8808
```

## Receipt

- `git.tag` capability: annotated tag `v0.7.6` pushed to `origin`
- Remote annotated tag peels to verified main SHA `d89862bbe67256cf6274be1d71ee7f8857cb8808`
- Exactly one annotated semver tag at verified SHA
- no-pending-effects: all required local Git effects complete
- Tag peeled to the fix commit (`d89862b`) rather than the docs commit (`cb47fe8`) so the tag points to the code that carries the new variant. This matches the m9-04..m9-07 convention where the peel SHA carries the runtime semantics of the release, not the docstring additions.
