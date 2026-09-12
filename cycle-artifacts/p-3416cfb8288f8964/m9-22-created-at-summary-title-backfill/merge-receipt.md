# m9-22: Merge Receipt

| Field | Value |
|---|---|
| Cycle | m9-22-created-at-summary-title-backfill |
| Path | B-direct |
| Base SHA | `71e7d11` (main @ start) |
| Branch | `fix/m9-22-created-at-summary-title-backfill` |
| Merge target | `main` |
| Merged at | 2026-09-12T11:22:00Z |

## Changes merged

1. 16 prior `apply-checkpoint.json` files (m9-03..m9-18): added `created_at`, `title`, `summary` fields.
2. `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`: cross-check #15 added.
3. `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-22 row added, total cycles 37→38.
4. `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last archive updated.

## Backfill sources

- `created_at`: extracted from git log (`git log --diff-filter=A --format=%aI -- <file>`),
  normalized from +02:00 local time to UTC.
- `title`: derived from the cycle folder slug (e.g. `m9-11-cycles-index-metadata-drift`
  → `"cycles index metadata drift"`).
- `summary`: extracted from the cycle's `merge-receipt.md` first non-heading,
  non-table, non-code paragraph.
