# Verification report — retire-stale-bus-doc-mentions

> Retroactively produced for the ledger sync. The work itself landed in
> commit `23757379` (merged via `ab863cf1`) on 2026-09-17. This report
> re-runs the gate evidence on 2026-09-18 against `main` @ `b9461a3f`.

## Tier applied

B-direct (per `proposal.md §Tier`) but executed as A-min to satisfy the
ledger's path requirement.

## T0 — lint gate

| Gate | Outcome | Evidence |
|---|---|---|
| `cargo fmt --all -- --check` | PASS | clean exit, no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | clean exit after `Default` impl patch in `23757379` |

## T1+T2 — lib unit + per-crate integration of touched crates

| Crate | Tests | Result |
|---|---|---|
| `chronos-log` | 77 / 77 | PASS in 0.42 s |
| `chronos-native` | 107 / 107 | PASS in 12.89 s |
| `chronos-services` | 370 / 370 | PASS in 31.12 s |
| `chronos-mcp` | 99 / 99 | PASS in 15.92 s |

Total: **653 / 653 passed**, 0 failed, 0 ignored.

## Ratchet

`python3 scripts/check_legacy_evb.py` → PASS, `baseline_total = 0`.

## Out-of-scope gates (not run)

- **T3** (full workspace): unnecessary for a doc-only chore.
- **T4-smoke** (sandbox subset): no MCP wire surface change (the
  `live_probes` field doc was rewritten in-place, no DTO change).
- **T5** (full sandbox): skipped per B-direct tier.

## Outcome

PASS. The chore is verified at the B-direct tier. The retroactive
ledger sync can proceed.