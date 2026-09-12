# Change: m9-24 verify-findings schema normalization

| Field | Value |
|---|---|
| Cycle | m9-24-verify-findings-schema-normalize |
| Base SHA | `dd38925` |
| Head SHA | `8e0bfbf` |
| Tag | `v0.7.22` (peels to `8e0bfbf`) |
| Path | B-direct |
| Date | 2026-09-12 |
| Author | jcode (auto-mode B-direct) |

## Summary

8 prior CLOSED cycles (m9-11..m9-18) had verify-findings.json with
legacy schema (subject_sha at top level + lens_summary + verdict +
evidence). m9-19+ use the simpler new schema with subject dict.

m9-24 normalizes the 8 prior cycles. Legacy fields are preserved
under _legacy for traceability.

## Cross-check

Cross-check #17 added to vault-drift-sweep.md.

## Verification

- T0 gate: cargo fmt --check + cargo clippy -- -D warnings → PASS
- All 17 cross-checks → PASS
