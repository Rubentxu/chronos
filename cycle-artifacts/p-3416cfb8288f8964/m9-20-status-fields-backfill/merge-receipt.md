# m9-20: Merge Receipt

| Field | Value |
|---|---|
| Cycle | m9-20-status-fields-backfill |
| Path | B-direct |
| Base SHA | `d665691` (main @ start) |
| Branch | `fix/m9-20-status-fields-backfill` |
| Merge target | `main` |
| Merged at | 2026-09-12T11:13:00Z |

## Changes merged

1. 17 prior `apply-checkpoint.json` files (m9-03..m9-18): added `verify_status: "passed"`, `release_status: "released"`, `archive_status: "archived"` (all three were `null` before).
2. `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`: added cross-check #13 enforcing these fields.
3. `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-20 row added, total cycles 35→36.
4. `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last archive updated.

## Schema note

m9-19 introduced the `verify_status`, `release_status`, and
`archive_status` fields but did not backfill the 17 prior CLOSED
cycles. m9-20 closes the backfill and adds cross-check #13 to
prevent recurrence of "new field added, no backfill" drift.
