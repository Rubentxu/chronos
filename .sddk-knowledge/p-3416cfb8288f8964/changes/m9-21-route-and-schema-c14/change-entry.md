# Change: m9-21 Verbose route + cross-check #14


## Subject

- base_sha: `efb9d93da368c4c6e1473c4668bbc945dff850c6`
- head_sha: `14ecf16ded816896cb612721875e97380bcadbc3`
- cycle: m9-21
- tag: `v0.7.19`
- route: B-direct
- date: 2026-09-12


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

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-21-route-and-schema-c14/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-21-route-and-schema-c14/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-21-route-and-schema-c14/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-21-route-and-schema-c14/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-21-route-and-schema-c14/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-21-route-and-schema-c14/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-21-route-and-schema-c14/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-21-route-and-schema-c14/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Verification

- T0 gate: `cargo fmt --check` + `cargo clippy -- -D warnings` → PASS
- All 14 cross-checks → PASS (0 drift)
