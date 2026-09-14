# m9-90 Spec — Stale Branches Cleanup

## Behavior

A single Python tool (`scripts/clean_m9_90_stale_branches.py`) that
deletes stale feat/m9-* branches merged into main.

### Inputs

- Optional `--dry-run`: print what would be deleted without acting.
- Optional `--reset-log`: truncate `scripts/branches-deleted-m9-90.log`
  before writing.

### Outputs

- 0 or more branches deleted (local + remote).
- 0 or more log lines appended to `scripts/branches-deleted-m9-90.log`.
- Exit code 0 on success, 1 on error, 2 on partial failure.

### Behavior

1. Verify inside a git work tree (else exit 1).
2. Verify current branch is not `main` (else exit 1 — refuse to
   operate on the trunk).
3. Discover candidate branches:
   - Local: `git branch --list 'feat/m9-*' 'fix/m9-*' 'chore/m9-*'`,
     excluding the current branch.
   - Remote: `git branch -r --list 'origin/feat/m9-*' 'origin/fix/m9-*' 'origin/chore/m9-*'`.
4. Filter to merged-into-main only:
   - For each candidate, run `git merge-base --is-ancestor <branch> main`.
   - If exit code 0, include in deletion set.
   - Otherwise, log as "preserved (not merged)" — never auto-delete.
5. Delete each merged branch:
   - Local: `git branch -d <branch>` (safe-delete; refuses if not merged).
   - Remote: `git push origin --delete <branch>`.
6. Append a log line per action:
   `[OK|FAIL|DRY-RUN] <timestamp> local|remote <branch> @ <tip-sha> [detail]`
7. Print summary: `N local + N remote deleted, N preserved`.

### Safety properties

- **Refuses outside git**: `git rev-parse --is-inside-work-tree` check.
- **Refuses on main**: refuse to operate while on the trunk.
- **Merged-into-main check**: never delete a branch not merged into main.
- **Recovery log**: every deletion recorded to
  `scripts/branches-deleted-m9-90.log` so a human can restore via
  `git branch <name> <sha>`.
- **Idempotent**: re-running after completion deletes 0 branches.

## Scenarios

### Scenario 1: 9 stale branches present

```
$ python3 scripts/clean_m9_90_stale_branches.py
Discovered: 6 local + 3 remote candidate branches
Merged into main: 6 local + 3 remote
NOT merged (preserved): 0 local + 0 remote
=== Local ===
[OK] local feat/m9-67-cc-smoke-test @ ef21e1f deleted
... (6 lines)
=== Remote ===
[OK] remote origin/feat/m9-67-cc-smoke-test @ ef21e1f deleted
... (3 lines)
Summary: 6 local + 3 remote deleted. 0 preserved.
Log written to scripts/branches-deleted-m9-90.log
```

### Scenario 2: re-run after completion

```
$ python3 scripts/clean_m9_90_stale_branches.py
Discovered: 0 local + 0 remote candidate branches
Merged into main: 0 local + 0 remote
NOT merged (preserved): 0 local + 0 remote
Summary: 0 local + 0 remote deleted. 0 preserved.
```

### Scenario 3: --dry-run

```
$ python3 scripts/clean_m9_90_stale_branches.py --dry-run
Discovered: 6 local + 3 remote candidate branches
[DRY-RUN] local feat/m9-67-cc-smoke-test @ ef21e1f (would delete)
... (9 lines)
Summary: 6 local + 3 remote [would be] deleted. 0 preserved.
(dry-run — nothing written to log)
```

### Scenario 4: refuse on main

```
$ git checkout main
$ python3 scripts/clean_m9_90_stale_branches.py
ERROR: refuse to operate on main branch (checkout a cycle branch first)
exit 1
```

## Acceptance criteria

1. After running the tool, `bash scripts/check_vault_drift.sh`
   reports 0 drift lines for CC#46 + CC#53.
2. `scripts/branches-deleted-m9-90.log` records all 9 deletions
   (timestamp + branch + tip SHA).
3. The tool can be re-run safely (idempotent; deletes nothing on
   second run).
4. The tool refuses to operate on `main`.
5. The tool refuses to delete a branch not merged into main.
