# Merge receipt — rec-c3-ci-hygiene

> Hygiene cycle implemented in branch `rec-c3-ci-hygiene`. CIH-A / CIH-B /
> CIH-C executed and closed. Fast-forward integration into `main` is REQUIRED
> before Tren B (REC-C3.3.3) is unblocked; the prior narrative that branch-local
> closure was sufficient has been retracted.

## Merge metadata

| Field | Value |
|---|---|
| Head SHA | `2e761d4f76dc4c7b2a78972753770ac2aedf0357` |
| Base SHA | `fa5eb5827e997c874047c6c2318ad2b5b1c8f323` |
| Branch | `rec-c3-ci-hygiene` |
| Date | `2026-09-19T09:35:00Z` |
| Merged to | `main` (fast-forward pending; this receipt records implementation, integration is a later event) |
| Integration status | pending — fast-forward required |

## Commits on branch (relative to base)

1. `533304b4` test(sandbox): reconcile m1_02 Case 6 to REC-C1.5.2 strict replay contract
2. `2b5ff2e1` chore(checkpoint): record CIH-A head_sha and evidence
3. `7447ea56` test(sandbox): correct CIH-A atomicity comment — second reopen proves reproducibility, not atomicity
4. `b71c1adc` fix(sandbox): McpTestClient::start locates chronos-mcp binary via cargo metadata + auto-build (CIH-B)
5. `10138acc` chore(checkpoint): record CIH-B + CIH-A-doc-fix head_sha and evidence
6. `5bcb2b63` chore(vault): reconcile Vault Drift to exit 0 and close rec-c3-ci-hygiene (CIH-C)
7. `2e761d4f` chore(vault): rec-c3-ci-hygiene final closure artifacts (release/merge receipts + archive-manifest + live verify-findings verdict)
8. `27c5c811` chore(vault): rec-c3-ci-hygiene — sync SHAs (CC#22/23) + base_sha in apply-checkpoint
9. `3fdcbf13` chore(vault): rec-c3-ci-hygiene — finalize apply-checkpoint ledger fields, sync SHA-driven artifacts to HEAD
10. `6f03f58d` chore(vault): rec-c3-ci-hygiene — CIH-C.1 cycle artifact reconciliation
11. `8639a3ff` chore(vault): rec-c3-ci-hygiene — restore historical SHA anchor + reconcile receipt contradictions

## Integration plan (REQUIRED before Tren B)

- CIH-A / CIH-B / CIH-C executed and closed at the implementation commit
  `2e761d4f76dc4c7b2a78972753770ac2aedf0357` (historical cycle anchor).
- CIH-C.1 (`6f03f58d`) and the post-closure SHA-anchor restoration (`8639a3ff`)
  are post-closure maintenance commits; they do not invalidate `head_sha` =
  `2e761d4f` as the cycle's implementation anchor.
- Fast-forward of `rec-c3-ci-hygiene` into `main` is REQUIRED before Tren B
  is unblocked. Five gates must be GREEN on the integrated HEAD of `main`,
  not only on the branch.
- The fast-forward is to be performed without squash, rebase, or merge commit
  (operator rule 9); all 11 commits on the branch are preserved.
- `main_sha` in this cycle's apply-checkpoint is the historical cycle anchor
  `2e761d4f76dc...` per CC#12 (m9-19 convention: post-cycle main = head_sha);
  the actual integration commit becomes the new HEAD of `main`, but the
  cycle's `head_sha` and `main_sha` anchor remain `2e761d4f`.

## Cross-checks

- `bash scripts/check_vault_drift.sh` exit 0 (CIH-C).
- `python3 scripts/check_architecture_contracts.py` PASSED (no change in this cycle).
- `python3 scripts/check_hex_boundary.py` OK (no change in this cycle).
- `cargo fmt --all -- --check` exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` OK.
- `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e -- --test-threads=1` → 62 suites, 0 failed.
- Smoke subset (e2e_connectivity + analytics_tools + session_persistence) → 9/9 passed.

## Canonical SHA fields

| Field | Value |
|---|---|
| Head SHA | `2e761d4f76dc4c7b2a78972753770ac2aedf0357` |
| Base SHA | `fa5eb5827e997c874047c6c2318ad2b5b1c8f323` |
| Branch | `rec-c3-ci-hygiene` |
| Date | `2026-09-19T10:12:34Z` |