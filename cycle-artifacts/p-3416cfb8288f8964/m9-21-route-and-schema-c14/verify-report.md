# m9-21: Verify Report

| Field | Value |
|---|---|
| Cycle | m9-21-route-and-schema-c14 |
| Verify time | 2026-09-12T11:14:00Z |
| Verifier | self (B-direct light-verify) |
| Result | PASS |

## T0 gate

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0 warnings) |

## Cross-check sweep (14 checks)

All 14 cross-checks in `vault-drift-sweep.md` run via Python script.
Result: 0 drift across all 14 checks.

| # | Check | Result |
|---|---|---|
| 1 | Vault ID uniqueness | PASS |
| 2 | findings_closed ↔ Terminated terms | PASS |
| 3 | apply-checkpoint ↔ tag consistency | PASS |
| 4 | SHA-256 in archive-manifest | PASS |
| 5 | cycles/index.md metadata | PASS |
| 6 | terms/index.md "Last archive" | PASS |
| 7 | change-entry.md SHA consistency | PASS |
| 8 | apply-checkpoint + archive-manifest SHAs exist | PASS |
| 9 | archive-manifest 40-char SHAs | PASS |
| 10 | verify-findings + markdown SHAs | PASS |
| 11 | apply-checkpoint metadata fields | PASS |
| 12 | route + main_sha + empty folder | PASS |
| 13 | verify/release/archive_status backfill | PASS |
| 14 | m9-19+ no legacy schema-v1 fields + bare route | PASS (new) |

## Summary

8 cycles (m9-11..m9-18) had route normalized. Cross-check #14 added.
