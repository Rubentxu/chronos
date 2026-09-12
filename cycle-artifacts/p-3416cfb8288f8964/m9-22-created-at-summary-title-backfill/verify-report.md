# m9-22: Verify Report

| Field | Value |
|---|---|
| Cycle | m9-22-created-at-summary-title-backfill |
| Verify time | 2026-09-12T11:22:00Z |
| Verifier | self (B-direct light-verify) |
| Result | PASS |

## T0 gate

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0 warnings) |

## Cross-check sweep (15 checks)

All 15 cross-checks pass clean.

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
| 14 | m9-19+ no legacy fields + bare route | PASS |
| 15 | created_at + title + summary present | PASS (new) |

## Summary

16 cycles backfilled with created_at, title, summary. Cross-check #15 added.
