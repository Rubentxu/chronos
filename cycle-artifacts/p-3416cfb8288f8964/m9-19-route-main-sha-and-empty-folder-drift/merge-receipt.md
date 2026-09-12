# m9-19: Merge Receipt

| Field | Value |
|---|---|
| Cycle | m9-19-route-main-sha-and-empty-folder-drift |
| Path | B-direct |
| Base SHA | `6dce373` (main @ start) |
| Branch | `fix/m9-19-route-main-sha-and-empty-folder-drift` |
| Merge target | `main` |
| Merged at | 2026-09-12T11:09:00Z |

## Changes merged

1. `cycle-artifacts/p-3416cfb8288f8964/{m9-03..m9-10}-*/apply-checkpoint.json`: route field normalized from `"local"` to `"B-direct"` (8 cycles).
2. `cycle-artifacts/p-3416cfb8288f8964/{m9-11,m9-12,m9-13}-*/apply-checkpoint.json`: main_sha set to post-cycle HEAD (head_sha) instead of pre-cycle HEAD (base_sha) (3 cycles).
3. `cycle-artifacts/p-3416cfb8288f8964/m9-11-m9-04-findings-closed-drift-fix/` removed (empty folder from aborted investigation).
4. `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`: added cross-check #12.
5. `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-19 row added, total cycles 34→35.
6. `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last archive updated.
