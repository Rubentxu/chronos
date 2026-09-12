# Release Report — m9-19

| Field | Value |
|---|---|
| Cycle | m9-19-route-main-sha-and-empty-folder-drift |
| Path | B-direct (T0 + light-verify) |
| Cycles closed | 35 → 36 |
| Tags | v0.7.16 → v0.7.17 |

## Drift classes closed

| Class | Cycles | Fix |
|---|---|---|
| `route: "local"` placeholder | m9-03..m9-10 (8 cycles) | Normalized to `"B-direct"` |
| `main_sha != head_sha` | m9-11, m9-12, m9-13 | Set `main_sha = head_sha` |
| Empty cycle folder | 1 folder | `rmdir` |

## Cross-check

Cross-check #12 added to `vault-drift-sweep.md` enforcing all 3 classes.

## Tier

B-direct: T0 only (file-local mechanical remediations).
## Cross-checks

- C1-C10: pass

