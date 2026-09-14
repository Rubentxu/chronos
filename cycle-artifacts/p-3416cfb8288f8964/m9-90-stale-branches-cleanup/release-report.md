# Release Report — m9-90-stale-branches-cleanup

> **Cycle**: m9-90-stale-branches-cleanup
> **Path**: B-direct
> **Tag**: v0.7.92
> **Merge SHA**: 2184975a93b43ea1bbd2681dead79dfb4476fef7
> **Date archived**: 2026-09-14
> **Status**: released

## What changed

m9-90 is a **vault-only hardening cycle**. No Rust source code touched.

It closes CC#46 (no stale local feat/fix/chore/m9-* branches) and
CC#53 (no stale remote branches merged into main, all milestone
prefixes) by deleting the 9 stale feat/m9-* branches that m9-65
missed (m9-65 ran before m9-66..m9-78 cycles landed their feat
branches).

**Drift line count**: 9 stale branches → 0 (-100%).

| Bucket | Before | After |
|---|---|---|
| Local feat/m9-* merged into main | 6 | 0 |
| Remote feat/m9-* merged into main | 3 | 0 |
| **Total stale** | **9** | **0** |

## Tool added

`scripts/clean_m9_90_stale_branches.py` (220 lines, idempotent + dry-run +
recovery log):

1. Discovers all local + remote branches matching `feat/m9-*`,
   `fix/m9-*`, `chore/m9-*` prefixes (per CC#46).
2. Filters to those whose tip is merged into main (CC#53 safety check
   via `git merge-base --is-ancestor`).
3. Deletes local branches via `git branch -d` and remote branches via
   `git push origin --delete`.
4. Logs every action (timestamp, branch, SHA, status) to
   `scripts/branches-deleted-m9-90.log` for recovery.
5. Supports `--dry-run` for safe preview and `--reset-log` to start
   fresh.

Reusable for future stale-branch sweeps.

## Verification

- **T0** (CC drift sweep): clean. CC#46 + CC#53 now clean (was 9
  stale branches, now 0).
- **T1** (lib unit tests): not required — vault-only cycle.
- **T2** (per-crate integration): not required.
- **T4** (sandbox smoke): not required.

## Findings

- **FIND-M9-90-STALE-BRANCHES-DELETED** (closed): 9 stale feat/m9-*
  branches deleted via scripts/clean_m9_90_stale_branches.py.
- **FIND-M9-90-DEFERRED-CLOSED** (closed): closes the deferred
  portion of FIND-M9-89-STALE-BRANCHES-DEFERRED + FIND-M9-71.
- **FIND-M9-90-NO-RUST-CHANGES** (closed): vault-only cycle.

## Carry-forward findings

None introduced.

External-deferred (carried from m9-88, not actionable in chronos):
FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK.

## Status

Released as `v0.7.92` at merge commit `2184975a93b43ea1bbd2681dead79dfb4476fef7`.
The branch `chore/m9-90-stale-branches-cleanup` was merged into `main`
with `--no-ff` and will be deleted after this report is archived.

## Cross-checks

- `apply-checkpoint.head_sha` == `release-receipt.head_sha` ==
  `merge-receipt.head SHA` == `2184975a93b43ea1bbd2681dead79dfb4476fef7`.
- `Remote tag` v0.7.92 peel: `2184975a93b43ea1bbd2681dead79dfb4476fef7`
  (clean match to merge commit; CC#42 fixpoint-cascade workaround).
- `apply-checkpoint.peel_match` == `true`.
- `apply-checkpoint.main_sha` == `apply-checkpoint.head_sha`.
- `apply-checkpoint.status` == `"CLOSED"`.
- `apply-checkpoint.archive_status` == `"complete"`.
- `cycles/index.md` row added; Total cycles 89 → 90.
- `bash scripts/check_vault_drift.sh`: CC#46 + CC#53 clean.
- `python3 scripts/clean_m9_90_stale_branches.py --dry-run`: 0 candidates
  (idempotent — nothing left to delete).
- `cargo fmt --all -- --check`: clean (no Rust touched).
