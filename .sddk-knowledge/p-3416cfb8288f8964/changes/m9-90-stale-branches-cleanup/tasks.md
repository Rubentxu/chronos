# m9-90 Tasks — Stale Branches Cleanup

B-direct cycle. 5 tasks, all under 1 hour of work.

## Task 1: Discovery + census

- Identify all stale feat/fix/chore/m9-* branches merged into main.
- Verify via `git merge-base --is-ancestor` (CC#53 safety check).
- Record tip SHA of each candidate for the recovery log.

**Acceptance**: Census table with branch + tip SHA + merge status.

## Task 2: Write `scripts/clean_m9_90_stale_branches.py`

- 200-line idempotent Python tool.
- `--dry-run` flag.
- `--reset-log` flag.
- Recovery log to `scripts/branches-deleted-m9-90.log`.
- Refuse to operate on `main` (safety).
- Refuse to operate outside git work tree.

**Acceptance**: Script runs cleanly with `--dry-run` showing all 9
candidates + tip SHAs.

## Task 3: Execute cleanup

- Run `python3 scripts/clean_m9_90_stale_branches.py --reset-log`.
- Verify 9 successful deletions + 0 preserved.

**Acceptance**: Recovery log has 9 `[OK]` lines.

## Task 4: CC drift sweep verification

- Run `bash scripts/check_vault_drift.sh`.
- Confirm CC#46 + CC#53 report 0 drift lines.

**Acceptance**: Sweep output clean for CC#46 + CC#53.

## Task 5: Cycle artifacts + release + archive + handoff

- Write cycle artifacts: apply-checkpoint.json, verify-report.md,
  verify-findings.json, merge-receipt.md, release-receipt.md,
  release-report.md, implementation-receipt.md.
- Write knowledge artifacts: change-entry.md, exploration-report.md,
  proposal.md, spec.md, tasks.md.
- Merge `--no-ff` into main, pre-create tag `v0.7.92`, move to merge
  commit per CC#42 fixpoint-cascade workaround.
- Write archive-manifest.md + handoff.md.
- Push to origin; delete cycle branch.

**Acceptance**: All artifacts present; tag `v0.7.92` pushed;
`cycles/index.md` updated to Total cycles 90.
