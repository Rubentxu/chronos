# Merge Receipt — rec-c1-5-closure

| Field | Value |
|---|---|
| Head SHA | `a1a79c80122d3e8a859f44bb29f043f5422e1324` |
| Base SHA | `17367f2d5ebcd4510de43ba2eef6c8b61cedd6de` |
| Branch | `feat/rec-c1.5-closure` |
| Date | `2026-09-20` |
| Cycle | `p-3416cfb8288f8964/rec-c1-5-closure` |
| Route | `local` |
| Squash | false |
| Rebase | false |
| Cherry-pick | false |
| Force-push | false |

- merge_commit: a1a79c80122d3e8a859f44bb29f043f5422e1324
- merge_first_parent: efb4498953feabb1f0a0bb1c841ac03e93d46f1c
- merge_second_parent: 59e6ac9fd46e180de82f31fa0402e1946000c7b8
- tag_peel: 5bbf774888c3bced7967e3e060859d9a78d7035a

## Verification performed 2026-09-30

Every fact in this receipt was re-observed on the working tree at
`6effe2047816f856c963f7fbe9f4488508c3079e`, not copied from a prior claim.

- `git merge-base --is-ancestor a1a79c80 HEAD` → exit `0`, so the merged
  work is an ancestor of the current trunk.
- `git merge-base --is-ancestor 5bbf7748 HEAD` → exit `0`.
- `git ls-remote --tags origin rec-c1.5-closure` →
  `79193338994679e3e7ebb66fc0ff6a568348b295`, peel
  `5bbf774888c3bced7967e3e060859d9a78d7035a`.
- `git rev-list --no-merges --count efb44989..a1a79c80` → `39`, history
  preserved 1:1.

## Commits merged

39 non-merge commits across the branch history, headed by:

| SHA | Subject |
|---|---|
| `59e6ac9f` | fix(rec-c1.5): sandbox tests pass env vars via child process (CC#56) |
| `1a1ec68a` | style(rec-c1.5): apply fmt + clippy cleanup from clippy -D warnings |
| `a2aee5b9` | test(rec-c1.5): add try_new readiness invariant (R5) |
| `23023651` | test(rec-c1.5): add identical-stale restart UAT (R2) |
| `e75b5f6a` | test(rec-c1.5): add real-process unclean restart UAT (R1) |
| `1bb32f05` | feat(rec-c1.5): expose durable deletion paths |
| `38e02be4` | feat(rec-c1.5): durably seal logs on clean session stop |
| `6644bdbb` | feat(rec-c1.5): make session deletion remove durable logs |
| `94bd092a` | feat(rec-c1.5): bootstrap execution logs during MCP startup |
| `2c4da9ba` | feat(rec-c1.5): canonical ExecutionLog root resolver |
| `17367f2d` | fix(rec-c1.5.4): plan carries validated handles; delete never reopens other sessions |

## Note on the two base SHAs

`apply-checkpoint.json` records `base_sha: 17367f2d5ebcd...`, a **branch**
commit dated 10 minutes after `efb44989`. The ledger manifest records
`base: efb44989`, which is the merge's first parent on `main`.

Both are ancestors of the merge commit, so they are not in conflict: the
checkpoint names the branch point the work was cut from, and the manifest names
the trunk commit the merge landed on. The canonical table above follows the
checkpoint, because CC#23 compares the receipt against the checkpoint.

## Post-merge closure commits on trunk

| SHA | Subject |
|---|---|
| `f86cb2bc` | chore(rec-c1.5): finalize apply-checkpoint (release_status, main_sha, remote_tag) |
| `5bbf7748` | docs(rec-c1.5): add implementation-receipt for cycle closure |

These carry the tag `rec-c1.5-closure`.
