# Change: m9-89 cascade CC cleanup across m9-77..m9-88

## Identification

| Field | Value |
|---|---|
| Cycle ID | `m9-89-cascade-cc-cleanup-m9-77-87` |
| Workspace | `p-3416cfb8288f8964` |
| Path | A-lite (vault-only hardening) |
| Status | CLOSED |
| Base SHA | `a195367d64bd1caa56dedea82195259deb7b671b` |
| Head SHA | `d0071ee5d6053ca38eca02c2a2f6dba47fc2daca` |
| Tag | `v0.7.91` |

## Subject

- base_sha: `a195367d64bd1caa56dedea82195259deb7b671b`
- head_sha: `d0071ee5d6053ca38eca02c2a2f6dba47fc2daca`
- cycle: m9-89
- branch: `chore/m9-89-cascade-cc-cleanup-m9-77-87`

## Problem

m9-66's `apply-checkpoint.json` had an invalid JSON escape (`\|` instead
of `\\|`) on line 35. This caused the vault-drift meta-check (CC#48) to
abort before evaluating any CC. The abort masked ~150 lines of cascading
CC drift across 12 CCs and 11 cycles (m9-77..m9-87) that had
accumulated since m9-77.

m9-88 closed the m9-66 JSON abort, which surfaced the cascade. m9-89
closes the cascade itself.

## Approach

Two mechanical tools, applied once:

1. **`scripts/audit_m9_89_cascade.py`** — enumerates which cycles need
   which CC fixes (per-CC, per-cycle JSON report).
2. **`scripts/fix_m9_89_cascade.py`** — applies the mechanical backfills
   idempotently across `apply-checkpoint.json`, `release-receipt.md`,
   `merge-receipt.md`, `archive-manifest.md`, `change-entry.md`, and
   `verify-findings.json`.

After the fix pass, `scripts/regen_manifest_index_shas.py` was run to
rewrite the SHA-256 rows in 86 archive-manifests (CC#4 cascade).

## Files changed

| Bucket | Files | Notes |
|---|---|---|
| `apply-checkpoint.json` | 12 | status/legacy/peel/main_sha/created_at/title/summary/findings_closed |
| `release-receipt.md` | 8 | Head SHA + Peel match added |
| `merge-receipt.md` | 52 | Head/Base/Branch/Date table format |
| `verify-findings.json` | 12 | head_sha synced to apply-checkpoint |
| `archive-manifest.md` | 21 | Head SHA backticks + Date field + SHA-256 regen |
| `change-entry.md` | 3 | m9-67 Base SHA, m9-80 Head SHA, this entry |
| `terms/index.md` | 1 | Last archive = m9-89 |
| `scripts/audit_m9_89_cascade.py` | NEW | 234 lines, audit-only |
| `scripts/fix_m9_89_cascade.py` | NEW | 578 lines, mechanical backfill |

**Total**: 111 files changed (+1696 / -385 lines).

## Drift delta

Before: 197 drift lines across 12 CCs.
After: 2 drift lines (CC#6 mid-cycle, CC#46/CC#53 deferred).

## Out-of-scope

- **CC#46/CC#53 stale branches** (m9-67..m9-78 feat/* branches merged
  into main but un-deleted): tracked under
  FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION; bundled with
  cycle artifact archival but its own cleanup cycle is deferred.
- **FIND-M9-81** (sddk CLI bug): external-deferred from m9-88;
  cannot be fixed in chronos scope.
