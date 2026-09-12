# Change: m9-23 cycle_id workspace prefix strip

| Field | Value |
|---|---|
| Cycle | m9-23-cycle-id-strip-prefix |
| Base SHA | `f5fbbd7` |
| Head SHA | `b549c74` |
| Tag | `v0.7.21` (peels to `b549c74`) |
| Path | B-direct |
| Date | 2026-09-12 |
| Author | jcode (auto-mode B-direct) |

## Summary

16 prior CLOSED cycles (m9-03..m9-18) had `cycle_id` set to the
full path `p-3416cfb8288f8964/m9-NN-slug`. m9-19+ use bare slugs
without the workspace prefix. m9-23 strips the prefix from the 16
prior cycles.

## Cross-check

Cross-check #16 added to vault-drift-sweep.md.

## Verification

- T0 gate: cargo fmt --check + cargo clippy -- -D warnings → PASS
- All 16 cross-checks → PASS
