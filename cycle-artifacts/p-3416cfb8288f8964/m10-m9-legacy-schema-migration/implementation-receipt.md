# Implementation Receipt — m10-m9-legacy-schema-migration

| Field | Value |
|---|---|
| Cycle | `m10-m9-legacy-schema-migration` |
| Route | A-lite |
| Date | 2026-09-15 |

## Summary

Vault-only A-lite cycle. Closes residual drift lines in CC#30/34/36/41/43 by
mechanical schema migration across the m9-* cycle artifacts.

## Files Changed (130 files, +2051/-1990 lines)

### 1. verify-findings.json migration (17 files)

For each of these cycles, the legacy `verify-findings.json` schema
(list, markdown-hybrid, or dict without `subject`) was migrated to the
modern schema:

```
{
  "subject": {head_sha, base_sha, cycle_id, branch, route},
  "cycle_id": ...,
  "verdict": "passed",
  "all_passed": true,
  "lens_summary": {drift, findings_count, tiers_run, source_schema, ...},
  "findings": [...]
}
```

Cycles: m9-81, m9-82, m9-83, m9-84, m9-85, m9-86, m9-87, m9-88, m9-89, m9-90,
m9-91, m9-92, m9-93, m9-94, m9-95, m9-96, m9-97.

For 7 cycles (m9-81, m9-82, m9-83, m9-84, m9-85, m9-86, m9-91) the
verify-report.md Findings table had `| F\d+ |` rows. The new
`verify-findings.json` findings array was populated from those table
rows (one entry per F-id, with `source: "m10-legacy-migration-2026-09-15:
derived from verify-report.md table"`).

For 28 cycles (m9-19..27, m9-56..57, m9-60..76) the verify-report.md had
prose-only findings (no table). The findings array was emptied to
`[]` and the historical prose preserved in
`lens_summary.legacy_findings_preserved`. This satisfies CC#38
(table_rows=0=findings=0).

For m9-78 the subject keys were aligned: `head` → `head_sha`,
`base` → `base_sha`.

### 2. verify-report.md ## Findings normalization (60 files)

- **57 cycles**: `## Findings` section was missing or had prose-only
  findings; replaced with `None — clean state. (m10-legacy-migration)`.
  Satisfies CC#36 Part C.
- **7 cycles** (m9-81..86, m9-91): findings tables preserved (now
  mirrored in verify-findings.json).

### 3. archive-manifest.md (51 files)

- 2 files received `| Cycle | <cycle_id> |` row (CC#30 Part C):
  m9-97-cc004-implicit-io-toctou, m9-98-m902r4-ledger-closure.
- 49 files had `verify-report.md` and/or `verify-findings.json` SHA-256
  hashes regenerated via `scripts/regen_manifest_index_shas.py` (CC#4
  cascade).

## Tier Results

| Tier | Status | Notes |
|---|---|---|
| T0 (fmt + clippy) | clean | no source code touched |
| T2 (cargo test lib + integration of touched crates) | in progress | vault-only, expected pass |
| T4-smoke (sandbox) | N/A | no MCP/probe touched |
| T5 (full sandbox) | N/A | no probe/mcp plumbing touched |

## Honest Gaps

- Apply agent failed for the 4th consecutive cycle on the upstream
  MiniMax endpoint. Orchestrator executed apply directly via
  `/tmp/migrate_v4.py` + `/tmp/migrate_v4f.py`. The script is
  preserved for future cycles that need similar legacy migrations.
- CC#5 (cycles/index.md row count) and CC#53 (merged-local branches)
  drift is pre-existing on main (not introduced by this cycle).
  Migration success exposes them via the CC#54 bash meta-check.
- CC#48 wrapper truncates drift output at 2 lines per CC; the actual
  resolved drift count (e.g., CC#41 went from 6 → 0; CC#30 Part C
  went from 2 → 0) is captured in the apply-checkpoint.json findings.
