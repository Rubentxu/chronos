# Implementation Receipt — m9-90-stale-branches-cleanup

## What was implemented

A single B-direct commit deleting 9 stale feat/m9-* branches
(6 local + 3 remote) from m9-67..m9-78 that m9-65 missed.

| Bucket | Files | Notes |
|---|---|---|
| `scripts/clean_m9_90_stale_branches.py` | NEW | 220 lines, idempotent, --dry-run, --reset-log |
| `scripts/branches-deleted-m9-90.log` | NEW | 9 successful deletions + 0 preserved |
| `git: local branches deleted` | 6 | feat/m9-67, 68, 69, 70, 77, 78 |
| `git: remote branches deleted` | 3 | origin/feat/m9-67, 68, 70 |

## Local branches deleted

| Branch | Tip SHA |
|---|---|
| feat/m9-67-cc-smoke-test | ef21e1f |
| feat/m9-68-verify-report-files-inventory-backfill | d4328cf |
| feat/m9-69-bounded-join-unit-test | 57c4a10 |
| feat/m9-70-mcp-store-isolation | 54e859f |
| feat/m9-77-attach-runtime | 4bd2da3 |
| feat/m9-78-attach-detach | a2c70fe |

## Remote branches deleted

| Branch | Tip SHA |
|---|---|
| origin/feat/m9-67-cc-smoke-test | ef21e1f |
| origin/feat/m9-68-verify-report-files-inventory-backfill | d4328cf |
| origin/feat/m9-70-mcp-store-isolation | 54e859f |

## Recovery

If a deleted branch needs to be restored, use the recovery log:

```bash
# Example: restore feat/m9-67-cc-smoke-test
git branch feat/m9-67-cc-smoke-test ef21e1f
git push origin ef21e1f:refs/heads/feat/m9-67-cc-smoke-test
```

See `scripts/branches-deleted-m9-90.log` for the full record.

## Safety check (CC#53)

Every deletion was preceded by `git merge-base --is-ancestor <branch> main`
which returned exit code 0 (i.e., the branch tip was an ancestor of
main). No not-merged branches were deleted.

## Verification

- `git branch --list 'feat/m9-*' 'fix/m9-*' 'chore/m9-*' | wc -l`: 1
  (only the current cycle branch chore/m9-90-stale-branches-cleanup).
- `git branch -r --list 'origin/feat/m9-*' 'origin/fix/m9-*' 'origin/chore/m9-*' | wc -l`: 0.
- `python3 scripts/clean_m9_90_stale_branches.py --dry-run`: 0 candidates.
- `bash scripts/check_vault_drift.sh`: CC#46 + CC#53 clean.
