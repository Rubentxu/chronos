# Merge receipt — rec-c3-ci-hygiene

> Hygiene cycle archived in branch `rec-c3-ci-hygiene`. Not fast-forwarded to
> `main` because the changes are pure vault + sandbox-harness hygiene with no
> behavior change; Tren B is unblocked independently by this cycle's closure.

## Merge metadata

| Field | Value |
|---|---|
| Head SHA | `2e761d4f76dc4c7b2a78972753770ac2aedf0357` |
| Base SHA | `fa5eb5827e997c874047c6c2318ad2b5b1c8f323` |
| Branch | `rec-c3-ci-hygiene` |
| Date | `2026-09-19T09:35:00Z` |
| Merged to | `rec-c3-ci-hygiene` (branch archive) |
| Fast-forwarded to main | NO (deliberate) |

## Commits on branch (relative to base)

1. `533304b4` test(sandbox): reconcile m1_02 Case 6 to REC-C1.5.2 strict replay contract
2. `2b5ff2e1` chore(checkpoint): record CIH-A head_sha and evidence
3. `7447ea56` test(sandbox): correct CIH-A atomicity comment — second reopen proves reproducibility, not atomicity
4. `b71c1adc` fix(sandbox): McpTestClient::start locates chronos-mcp binary via cargo metadata + auto-build
6. `10138acc` chore(checkpoint): record CIH-B + CIH-A-doc-fix head_sha and evidence
7. `5bcb2b63` chore(vault): reconcile Vault Drift to exit 0 and close rec-c3-ci-hygiene

## Why no fast-forward

The cycle is hygiene-only (vault metadata + sandbox harness). None of the
commits modify behavior of the production runtime; fast-forwarding to `main`
would carry the vault metadata and sandbox-harness changes, which is fine
but not required to unblock Tren B. The Tren B decision rule was "all five
gates GREEN at archive", which is satisfied by branch-local closure.

If a future cycle wants the hygiene changes on `main`, it can fast-forward
this branch into a single merge commit on `main` (operator rule 9: no squash,
no rebase).

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
| Head SHA | `5bcb2b6351f216ec95430b18e15ed5a6999d561e` |
| Base SHA | `fa5eb5827e997c874047c6c2318ad2b5b1c8f323` |
| Branch | `rec-c3-ci-hygiene` |
| Date | `2026-09-19T10:12:34Z` |