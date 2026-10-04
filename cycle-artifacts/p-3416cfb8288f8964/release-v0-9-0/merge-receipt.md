# merge-receipt — v0.9.0

| Field | Value |
|---|---|
| Cycle | `release-v0-9-0` |
| Path | B-direct |
| Branch | `main` |
| Date | 2026-10-04 |
| Base SHA | `98c4cd2341058872102f458655791ec53f21bd8c` |
| Head SHA | `aac0965909935cfa0294fd3fcf389ea33a371f6a` |
| Main SHA | `aac0965909935cfa0294fd3fcf389ea33a371f6a` |
| Tag | `v0.9.0` |

## Postcondition

`HEAD == origin/main == aac0965909935cfa0294fd3fcf389ea33a371f6a`

This was measured at release time, which is the state the tag freezes. The
archive bookkeeping commit lands after the tag, so `git log v0.9.0..main` is not
empty afterwards. That is bookkeeping about the release, not part of it: the
release boundary is the tag, and the SHA it peels to is the one verified here.

| Check | Command | Exit | Result |
|---|---|---|---|
| local HEAD | `git rev-parse HEAD` | 0 | `aac0965909935cfa0294fd3fcf389ea33a371f6a` |
| remote main | `git rev-parse origin/main` | 0 | `aac0965909935cfa0294fd3fcf389ea33a371f6a` |
| unpushed commits | `git rev-list --count origin/main..HEAD` | 0 | `0` |
| worktree | `git status --porcelain` | 0 | empty |
| branch | `git rev-parse --abbrev-ref HEAD` | 0 | `main` |

## How the push happened, stated plainly

`sddk release apply` was run first and its `git push` failed with
`exit status -1: timed out`. It created the local annotated tag and left
`origin/main` untouched.

The tag push was then completed directly: `git push origin v0.9.0` -> exit 0,
`* [new tag] v0.9.0 -> v0.9.0`. The repository's own pre-push gate ran and was
green on that invocation (fmt OK, clippy OK, CC#4 clean across 102 manifests).
No `--no-verify` and no `SKIP_PRE_PUSH_GATE` was used.

A first retry of `git push origin v0.9.0` was refused by the same pre-push gate
with `clippy FALLO` and no lint diagnostic. Root cause was not a defect: another
project (`sddk-framework`) was running its own release against the shared
`CARGO_TARGET_DIR=/var/home/rubentxu/cargo-targets`, so the two clippy
invocations contended for the build-directory lock. Re-running the identical
command with the contending process finished gave exit 0 in 45.81s, and the
second push attempt passed the gate. Recorded because a gate that failed for an
environmental reason and then passed is exactly the kind of fact that must not
be quietly dropped.

The second `sddk release apply` then converged idempotently:
`converged: true`, `sha: aac0965909935cfa0294fd3fcf389ea33a371f6a`,
`tag: v0.9.0`, `applied: 0`, skipping `push-main` ("origin/main already at
aac09659...") and `tag` ("v0.9.0 already points to aac09659...").
