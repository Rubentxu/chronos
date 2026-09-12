# Change: m9-21 Verbose route + cross-check #14

| Field | Value |
|---|---|
| Cycle | m9-21-route-and-schema-c14 |
| Base SHA | `efb9d93` |
| Head SHA | `14ecf16` |
| Tag | `v0.7.19` (peels to `14ecf16`) |
| Path | B-direct |
| Date | 2026-09-12 |
| Author | jcode (auto-mode B-direct) |

## Summary

Two drift classes closed in a single cycle:

1. **Verbose route strings**: 8 cycles (m9-11..m9-18) had `route`
   set to `"B-direct (T0 + light-verify)"`. Normalized to bare
   `"B-direct"`.

2. **Cross-check #14**: enforces that m9-19+ cycles do not
   introduce legacy schema-v1 fields (e.g. `change`, `artifacts`,
   `commits_since_base`, `findings_closed`, `ledger_state`,
   `next_cycle`, `notes`, `runtime_status`, `tag`) and use bare
   route labels. Pre-m9-19 cycles are exempt since their legacy
   data is preserved.

## Cross-check

Cross-check #14 added to
`.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`.

## Verification

- T0 gate: `cargo fmt --check` + `cargo clippy -- -D warnings` → PASS
- All 14 cross-checks → PASS (0 drift)
