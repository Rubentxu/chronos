# Merge Receipt — release-pipeline-honesty-v014

- cycle_id: p-3416cfb8288f8964/release-pipeline-honesty-v014
- route: local
- branch: main
- base: main
- prior_origin_main: 181886958013751172fa5d26b62b3f8a31e17083
- head_sha: 98c4cd2341058872102f458655791ec53f21bd8c
- origin_main_sha: 98c4cd2341058872102f458655791ec53f21bd8c
- head_equals_origin_main: true
- git_push_receipt: cap-git-push-f81988cbd29f
- squash: false
- rebase: false
- cherry_pick: false
- force_push: false
- merge_commit: none (direct trunk fast-forward; `git rev-list --left-right --count origin/main...HEAD` returned `0 4` before push)
- release_at: 2026-09-29T15:28:51Z

## Direct trunk publication

Publication route is a direct push to `main` (no PR, no forge integration).
`no-pending-effects` explicitly excludes CI/CD, GitHub Actions, hosted
releases, assets, signatures, and any optional post-tag distribution
(git-contract Rule 4 and Rule 5), so none of those are pending local effects
for this receipt.

## Commits published

| SHA | Subject |
|---|---|
| `3d935b6a` | fix(release): gate linux-only capture code and make release workflows honest |
| `5842075e` | chore(release): bump workspace version 0.1.3 -> 0.1.4 |
| `edd607a3` | docs(cycle): add release-pipeline-honesty-v014 implementation and verification artifacts |
| `a744e8c0` | docs(cycle): version the remaining historical cycle artifacts |
| `98c4cd23` | docs(session): version the rec-c3-ci-hygiene close handoff and ignore per-machine skill-registry state |

5 commits, history preserved 1:1, no rewriting of any previously authorized
commit.

## Worktree state at publication

`git status --porcelain` was empty before `sddk release apply`. The release
additionally refuses to run from a dirty worktree, so this is a machine-checked
precondition, not an assertion.

## Independent postcondition check (OBSERVED)

Re-verified after the push with a fresh
`git fetch origin main --tags`, not read from the capability response:

```
$ git rev-parse HEAD          -> 98c4cd2341058872102f458655791ec53f21bd8c
$ git rev-parse origin/main   -> 98c4cd2341058872102f458655791ec53f21bd8c
$ git ls-remote origin main   -> 98c4cd2341058872102f458655791ec53f21bd8c  refs/heads/main
```

Trunk convergence confirmed. Full release postconditions, including the
annotated tag peel, are recorded in `release-receipt.md`.

## Scope drift check

`--base main` with `branch == base == main` and a direct trunk push. No branch
name divergence, no scope drift.
