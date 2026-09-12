# Change: m9-20 Status fields backfill

| Field | Value |
|---|---|
| Cycle | m9-20-status-fields-backfill |
| Base SHA | `d665691` |
| Head SHA | `07febc1` |
| Tag | `v0.7.18` (peels to `07febc1`) |
| Path | B-direct |
| Date | 2026-09-12 |
| Author | jcode (auto-mode B-direct) |

## Summary

m9-19 introduced three new fields in `apply-checkpoint.json`:
`verify_status`, `release_status`, and `archive_status`. m9-19 set
these fields on its own apply-checkpoint.json but did not backfill
the 17 prior CLOSED cycles (m9-03..m9-18), which had them as `null`.

m9-20 closes this drift class by setting:

- `verify_status: "passed"`
- `release_status: "released"`
- `archive_status: "archived"`

on all 17 prior CLOSED cycles. Values are derived from the
`status: CLOSED` fact (a CLOSED cycle must have been verified,
released, and archived).

## Cross-check

Cross-check #13 added to
`.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`
enforcing these three fields on CLOSED cycles.

## Verification

- T0 gate: `cargo fmt --check` + `cargo clippy -- -D warnings` → PASS
- All 13 cross-checks → PASS (0 drift)

## Pattern

Each cycle that introduces a new schema field should also add a
cross-check that enforces the field. m9-19 added the field but no
cross-check (since the field wasn't yet "officially required"). m9-20
formalizes the field requirement and adds the cross-check.
