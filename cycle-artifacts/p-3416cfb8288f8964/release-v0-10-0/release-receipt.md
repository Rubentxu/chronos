# release-receipt — v0.10.0

| Field | Value |
|---|---|
| Cycle | `release-v0-10-0` |
| Path | B-direct |
| Branch | `main` |
| Date | 2026-10-05 |
| Base SHA | `aac0965909935cfa0294fd3fcf389ea33a371f6a` |
| Head SHA | `57c36040f29ffea2df11335324037a35d393641c` |
| Remote tag | `v0.10.0` |
| Remote tag object | `b9fe0c4fa116504fb89e448e183f289b65793fa7` |
| Remote tag_peel | `57c36040f29ffea2df11335324037a35d393641c` |
| Peel match | `true` |

## Remote postcondition

```
$ git ls-remote --tags origin 'v0.10.0*'
b9fe0c4fa116504fb89e448e183f289b65793fa7	refs/tags/v0.10.0
57c36040f29ffea2df11335324037a35d393641c	refs/tags/v0.10.0^{}
```

Two lines, not one: the first is the annotated tag object, the second is its
peeled commit. A single line would mean a lightweight tag, which the contract
rejects.

| Check | Command | Exit | Result |
|---|---|---|---|
| tag is annotated | `git cat-file -t v0.10.0` | 0 | `tag` |
| local peel | `git rev-parse 'v0.10.0^{commit}'` | 0 | `57c36040f29ffea2df11335324037a35d393641c` |
| remote annotated object | `git ls-remote --tags origin 'v0.10.0*'` | 0 | object + peeled commit above |
| remote peel matches main | compare peeled vs `origin/main` | 0 | equal |
| version at the tag | `git show v0.10.0:Cargo.toml` | 0 | `version = "0.10.0"` |

## Tag message

```
feat!: probe_drain reporta la vivacidad real de la sesion
```

## Version authority

`0.10.0` derived from history, not chosen by hand. The range `v0.9.0..HEAD`
carries one breaking change: R6.6 (`0fb807e9`) alters the output contract of
the `probe_drain` MCP tool, which no longer returns a hardcoded `"running"`.
Under this repository's 0.x convention a breaking change is a MINOR bump. In
the remote namespace only `v0.9.0` was taken on the 0.9.x/0.10.x line, so
`0.10.0` is the lowest free MINOR. `sddk release plan --tag v0.10.0` reported
`version_authority: cross_checked`. `Cargo.toml` and `Cargo.lock` both read
`0.10.0`.

A detail worth recording: the bump was audited line by line because a lazy
regex had also moved `rand_chacha` and `untrusted`, two third-party crates that
happened to sit at `0.9.0`. They were restored, and `cargo check` confirmed it
did not rewrite the lock — so the 19 changed lines are exactly the workspace's
own crates.
