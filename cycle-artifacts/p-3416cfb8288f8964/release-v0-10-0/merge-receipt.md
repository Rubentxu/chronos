# merge-receipt — v0.10.0

| Field | Value |
|---|---|
| Cycle | `release-v0-10-0` |
| Path | B-direct |
| Branch | `main` |
| Date | 2026-10-05 |
| Base SHA | `aac0965909935cfa0294fd3fcf389ea33a371f6a` |
| Head SHA | `57c36040f29ffea2df11335324037a35d393641c` |
| Main SHA | `57c36040f29ffea2df11335324037a35d393641c` |
| Tag | `v0.10.0` |

## Postcondition

`HEAD == origin/main == 57c36040f29ffea2df11335324037a35d393641c`

Measured at release time, which is the state the tag freezes. The archive
bookkeeping commit lands after the tag, so `git log v0.10.0..main` is not
empty afterwards. That is bookkeeping about the release, not part of it: the
release boundary is the tag, and the SHA it peels to is the verified one.

| Check | Command | Exit | Result |
|---|---|---|---|
| local HEAD | `git rev-parse HEAD` | 0 | `57c36040f29ffea2df11335324037a35d393641c` |
| remote main | `git rev-parse origin/main` | 0 | `57c36040f29ffea2df11335324037a35d393641c` |
| unpushed commits | `git rev-list --count origin/main..HEAD` | 0 | `0` |
| worktree | `git status --porcelain` | 0 | empty |
| branch | `git rev-parse --abbrev-ref HEAD` | 0 | `main` |

## How the push happened

`sddk release apply` ran with the local route and converged:

```
converged: true
sha: 57c36040f29ffea2df11335324037a35d393641c
tag: v0.10.0
applied: 1
- git.tag cap-git-tag-37884a637e7e
- skipped: push-main (origin/main already at 57c36040...)
```

`applied: 1` rather than `0`, unlike the v0.9.0 release: this time the tag did
not exist and the typed runner created and pushed it. No manual Git step, no
`--no-verify`, and the pre-push gate ran green on every commit of the range.

One working-tree change was deliberately held out of the release: the ledger
note for `DEBT-WAIT-EVENT-UNBOUNDED-01`, stashed rather than committed, so the
tag would land on the SHA whose CI is green instead of on a commit that only
exists to carry documentation. It is committed immediately after the tag.
