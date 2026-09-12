# m9-21: Merge Receipt

| Field | Value |
|---|---|
| Cycle | m9-21-route-and-schema-c14 |
| Path | B-direct |
| Base SHA | `efb9d93` (main @ start) |
| Branch | `fix/m9-21-route-and-schema-c14` |
| Merge target | `main` |
| Merged at | 2026-09-12T11:14:00Z |

## Changes merged

1. 8 prior `apply-checkpoint.json` files (m9-11..m9-18): `route` normalized from `"B-direct (T0 + light-verify)"` to bare `"B-direct"`.
2. `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`: cross-check #14 added enforcing m9-19+ cycles must not have legacy schema-v1 fields.
3. `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-21 row added, total cycles 36→37.
4. `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last archive updated.

## Note on legacy fields

Pre-m9-11 cycles (m9-03..m9-10) and m9-11..m9-18 use the schema-v1
legacy fields (`change`, `artifacts`, `commits_since_base`, etc.).
Cross-check #14 only applies to **m9-19+** cycles — the data on
older cycles is preserved as-is. This is the right call because the
legacy fields on m9-11..m9-18 contain useful information that was
not migrated to the new schema.

If a future cycle wants to backfill m9-03..m9-18 with the new
schema, that's a separate migration cycle.
