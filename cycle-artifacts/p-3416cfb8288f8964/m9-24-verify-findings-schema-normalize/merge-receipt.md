# m9-24: Merge Receipt

| Field | Value |
|---|---|
| Cycle | m9-24-verify-findings-schema-normalize |
| Path | B-direct |
| Base SHA | `dd38925` (main @ start) |
| Branch | `fix/m9-24-verify-findings-schema-normalize` |
| Merge target | `main` |
| Merged at | 2026-09-12T11:25:00Z |

## Changes merged

1. 8 prior `verify-findings.json` files (m9-11..m9-18): schema normalized from `{subject_sha, findings, lens_summary, verdict, evidence}` to `{cycle_id, subject: {head, head_sha}, findings, _legacy: {...}}`.
2. `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`: cross-check #17 added.
3. `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-24 row added, total cycles 39→40.
4. `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last archive updated.

## Schema migration

Old (m9-11..m9-18):
```json
{
  "findings": [...],
  "lens_summary": {...},
  "verdict": "PASS",
  "subject_sha": "abc...",
  "evidence": {...}
}
```

New (m9-19+):
```json
{
  "cycle_id": "m9-NN",
  "subject": {
    "head": "abc...",
    "head_sha": "abc..."
  },
  "findings": [...],
  "_legacy": {
    "lens_summary": {...},
    "verdict": "...",
    "evidence_keys": [...]
  }
}
```

The legacy fields are preserved under `_legacy` for traceability.
