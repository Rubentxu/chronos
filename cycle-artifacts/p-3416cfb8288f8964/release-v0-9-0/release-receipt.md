# release-receipt — v0.9.0

- cycle: `p-3416cfb8288f8964/release-v0-9-0`
- path: B-direct
- tag: `v0.9.0` (annotated)
- tag object: `9debf57174ad31ef20f17b6ac71e89c084322bbd`
- peels to: `aac0965909935cfa0294fd3fcf389ea33a371f6a`
- same SHA as verified `main`: yes

## Remote postcondition

```
$ git ls-remote --tags origin 'v0.9.0*'
9debf57174ad31ef20f17b6ac71e89c084322bbd	refs/tags/v0.9.0
aac0965909935cfa0294fd3fcf389ea33a371f6a	refs/tags/v0.9.0^{}
```

Two lines, not one: the first is the annotated tag object, the second is its
peeled commit. A single line would mean a lightweight tag, which the contract
rejects.

| Check | Command | Exit | Result |
|---|---|---|---|
| tag is annotated | `git cat-file -t v0.9.0` | 0 | `tag` |
| local peel | `git rev-parse v0.9.0^{commit}` | 0 | `aac0965909935cfa0294fd3fcf389ea33a371f6a` |
| remote annotated object | `git ls-remote --tags origin 'v0.9.0*'` | 0 | object + peeled commit above |
| remote peel matches main | compare peeled vs `origin/main` | 0 | equal |

## Tag message

```
feat!: dominio de frames, consultas corregidas y tracer ptrace fiable
```

## Version authority

`0.9.0` derived from history, not chosen by hand. Three commits carry the
breaking marker and a `BREAKING CHANGE` footer: `0745ead4 feat(domain)!`,
`d8f1a414 fix(query)!`, `97d592c3 fix(sandbox)!`. Under this repository's 0.x
convention a breaking change is a MINOR bump. In the remote namespace
`0.1.0-0.1.4`, `0.2.0-0.8.0` and `v0.8.0` are all taken (`v0.2.0` was found
already published at `3bee3ad3`, 1978 commits behind `v0.1.4`), so `0.9.0` is
the lowest free MINOR. `sddk release plan --tag v0.9.0` reported
`version_authority: cross_checked`. `Cargo.toml` and `Cargo.lock` both read
`0.9.0`.
