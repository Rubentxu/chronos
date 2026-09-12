# m9-26: Merge Receipt

| Field | Value |
|---|---|
| Cycle | m9-26-verify-findings-cycle-id-strip |
| Path | B-direct |
| Base SHA | `38e0e3a` (main @ start) |
| Branch | `fix/m9-26-verify-findings-cycle-id-strip` |
| Merge target | `main` |
| Merged at | 2026-09-12T11:28:00Z |

## Changes merged

1. `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/verify-findings.json`: stripped workspace prefix from `cycle_id`.
2. `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`: cross-check #16 extended to cover verify-findings.json (was only checking apply-checkpoint.json).
3. `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-26 row added, total cycles 41→42.
4. `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last archive updated.

## Why this is a separate cycle

m9-23 stripped the workspace prefix from apply-checkpoint.json
cycle_id for the 16 prior cycles. m9-26 strips the same prefix
from verify-findings.json cycle_id. m9-04 had this drift (cycle_id
"p-3416cfb8288f8964/m9-04-side-table-key-layout") but m9-23 missed
it because m9-23 only checked apply-checkpoint.json.

m9-26 extends cross-check #16 to also cover verify-findings.json
so this won't be missed again.
