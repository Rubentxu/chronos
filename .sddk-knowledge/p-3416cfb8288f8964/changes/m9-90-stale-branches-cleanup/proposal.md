# m9-90 Proposal — Stale Branches Cleanup

## Why this cycle

m9-89 closed 12 cascading CC drift lines but deferred CC#46 + CC#53
stale-branch cleanup to a dedicated cycle (FIND-M9-89-STALE-BRANCHES-DEFERRED).
m9-90 is that cycle.

## Goal

Close CC#46 (no stale local feat/fix/chore/m9-* branches) and CC#53
(no stale remote branches merged into main, all milestone prefixes)
by deleting the 9 stale feat/m9-* branches that m9-65 missed.

## Path

B-direct. Vault-only. No Rust source code touched.

## Deliverables

1. `scripts/clean_m9_90_stale_branches.py` (NEW, ~200 lines):
   idempotent Python tool for future stale-branch sweeps.
2. `scripts/branches-deleted-m9-90.log` (NEW): recovery log of all 9
   deletions.
3. 9 stale feat/m9-* branches deleted (6 local + 3 remote).
4. Cycle artifacts: apply-checkpoint.json, verify-report.md,
   verify-findings.json, merge-receipt.md, release-receipt.md,
   release-report.md, implementation-receipt.md.
5. Knowledge artifacts: change-entry.md, exploration-report.md,
   proposal.md, spec.md, tasks.md.
6. Archive: archive-manifest.md.
7. Handoff: handoff/m9-90-stale-branches-cleanup-closure-2026-09-14.md.

## Tag

`v0.7.92` (patch bump from `v0.7.91`).

## Out of scope

- m5/m3/m2 feat branches: different milestone prefix; not the focus
  of CC#46/CC#53.
- FIND-M9-81 (sddk CLI bug): external-deferred, not actionable in
  chronos scope.

## Verification

- T0: `bash scripts/check_vault_drift.sh` clean (CC#46 + CC#53).
- T0: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` clean.
- T1/T2/T4: not required (no Rust source code touched).
