# m9-23: Merge Receipt

| Field | Value |
|---|---|
| Cycle | m9-23-cycle-id-strip-prefix |
| Path | B-direct |
| Base SHA | `f5fbbd7` (main @ start) |
| Branch | `fix/m9-23-cycle-id-strip-prefix` |
| Merge target | `main` |
| Merged at | 2026-09-12T11:23:00Z |

## Changes merged

1. 16 prior `apply-checkpoint.json` files (m9-03..m9-18): `cycle_id` stripped of workspace prefix `p-3416cfb8288f8964/`.
2. `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`: cross-check #16 added.
3. `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-23 row added, total cycles 38→39.
4. `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last archive updated.

## Note

m9-19+ cycles (m9-19, m9-20, m9-21, m9-22) already use bare cycle
slugs without the workspace prefix. m9-23 closes this drift by
stripping the prefix from the 16 prior cycles.
