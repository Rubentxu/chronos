# Release Report — m9-90-stale-branches-cleanup

> **Cycle**: m9-90-stale-branches-cleanup
> **Path**: B-direct
> **Tag**: v0.7.92
> **Tag peel (immutable)**: `5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d` (post-cascade HEAD)
> **Merge SHA (cycle-artifacts)**: 2184975a93b43ea1bbd2681dead79dfb4476fef7
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

Released as `v0.7.92`. The branch `chore/m9-90-stale-branches-cleanup`
was merged into `main` with `--no-ff` and deleted after release.

Per m9-92 (CC#34 + CC#42 cleanup), the cycle's documented SHAs were
re-anchored from the cycle-artifacts location (2184975a) to the
immutable post-cascade tag location (5cdb4e1a) to match m9-89's pattern
and CC#3 era-awareness:

| Field | Cycle source | Re-anchored (m9-92) |
|---|---|---|
| `apply-checkpoint.head_sha` | 2184975a | **5cdb4e1a** |
| `apply-checkpoint.main_sha` | 2184975a | **5cdb4e1a** |
| `apply-checkpoint.remote_tag_peel` | 2184975a | **5cdb4e1a** |
| `release-receipt.Head SHA` | 2184975a | **5cdb4e1a** |
| `release-report.Merge SHA (cycle-artifacts)` | 2184975a | (kept as historical record) |

## Cross-checks

- `apply-checkpoint.head_sha` (re-anchored 5cdb4e1a) ==
  `release-receipt.head_sha` (5cdb4e1a) ==
  `merge-receipt.head_sha` (5cdb4e1a).
- `Remote tag` v0.7.92 peel: `5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d`
  (immutable post-cascade HEAD; cycle source 2184975a was advanced
  through the SHA-cascade fixpoint per CC#42 workaround documented in
  m9-83 handoff).
- `apply-checkpoint.peel_match` == `true`.
- `apply-checkpoint.main_sha` == `apply-checkpoint.head_sha` == 5cdb4e1a.
- `apply-checkpoint.status` == `"CLOSED"`.
- `apply-checkpoint.archive_status` == `"complete"`.
- `cycles/index.md` row added; Total cycles 89 → 90.
- `bash scripts/check_vault_drift.sh`: CC#46 + CC#53 clean.
- `python3 scripts/clean_m9_90_stale_branches.py --dry-run`: 0 candidates
  (idempotent — nothing left to delete).
- `cargo fmt --all -- --check`: clean (no Rust touched).
- `Merge SHA (cycle-artifacts)` retained as historical record of
  m9-90's original source commit (2184975a); the re-anchored
  documentation above reflects the post-cascade immutable state.
