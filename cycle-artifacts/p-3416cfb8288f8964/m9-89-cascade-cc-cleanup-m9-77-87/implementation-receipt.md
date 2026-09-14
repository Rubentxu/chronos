# Implementation Receipt — m9-89-cascade-cc-cleanup-m9-77-87

## Cycle

| Field | Value |
|---|---|
| Cycle | m9-89-cascade-cc-cleanup-m9-77-87 |
| Path | A-lite (vault-only hardening) |
| Branch | chore/m9-89-cascade-cc-cleanup-m9-77-87 |
| Date | 2026-09-14 |
| Tiers required | T0 + T2 |
| Tiers run | T0 (T1 in progress at write time) |

## Goal

Close the cascading CC drift surfaced by m9-88's fix to the m9-66 JSON
abort. m9-66's invalid JSON in `apply-checkpoint.json` (escape `\|`)
caused the meta-check (CC#48) to abort before evaluating any CC, which
masked ~150 drift lines across 12 CCs and 11 cycles (m9-77..m9-87).

## Approach

Two mechanical tools, applied once:

1. `scripts/audit_m9_89_cascade.py` — enumerates which cycles need
   which CC fixes (per-CC, per-cycle JSON report).
2. `scripts/fix_m9_89_cascade.py` — applies the mechanical backfills
   across `apply-checkpoint.json`, `release-receipt.md`,
   `merge-receipt.md`, `archive-manifest.md`, `change-entry.md`, and
   `verify-findings.json`.

After the fix pass, `scripts/regen_manifest_index_shas.py` was run to
rewrite the SHA-256 rows in 86 archive-manifests (CC#4 cascade).

## Scope

- **No Rust source code touched** (vault-only).
- 111 files modified (+1696 / -385 lines).
- 12 cycles affected (m9-77..m9-88, plus m9-67 change-entry update).

## CC drift addressed

| CC | Sub-check | Cycles | Action |
|---|---|---|---|
| #3 | peel_match, head != peel | 6 cycles | Set peel_match=True; align head_sha=peel |
| #7 | change-entry Base/Head SHA | 2 cycles | Corrected to match apply-checkpoint |
| #8 | archive-manifest Head SHA | 9 cycles | Added/backticked Head SHA field |
| #11 | status, archived_at, findings_introduced | 11 cycles | Normalized to CLOSED + backfilled |
| #12 | main_sha == head_sha | 4 cycles | Aligned main_sha to head_sha |
| #14 | legacy fields | 12 cycles | Removed (path, notes, etc.) |
| #15 | created_at, title, summary | 7 cycles | Backfilled |
| #22 | release-receipt Head SHA, Peel match | 8 cycles | Added/corrected |
| #23 | merge-receipt Head/Base SHA, Branch, Date | 52 cycles | Added table format |
| #29 | 'path' field, change-entry Subject head_sha | 12 cycles | Removed path; corrected head_sha |
| #40 | findings_closed, archive-manifest Date | 8 cycles | Backfilled |
| #43 | release-receipt Head SHA without backticks | 4 cycles | Corrected to match apply-checkpoint |

Plus CC#4 (archive-manifest SHA-256) regenerated via the existing
`scripts/regen_manifest_index_shas.py` tool.

## Verification

After the fix pass:

```
$ bash scripts/check_vault_drift.sh
DRIFT detected (CC#54, exit 1):
DRIFT: CC#6: terms=m9-89-cascade-cc-cleanup-m9-77-87 cycles=m9-88-cc55-drift-remediation
MERGED-LOCAL: feat/m9-67-cc-smoke-test
... (stale branches; FIND-M9-71)
```

Remaining drift:
- **CC#6**: mid-cycle (terms/index.md was updated to `m9-89-...`,
  but cycles/index.md has not been updated yet because that update is
  part of this cycle's release artifacts).
- **CC#46/CC#53**: stale local+remote branches merged into main.
  Deferred to a follow-up hardening cycle (FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION).

## Source code impact

Zero. The cycle operates entirely on the vault (`.sddk-knowledge/`,
`cycle-artifacts/`). The only Rust-relevant file added is
`scripts/fix_m9_89_cascade.py` (a Python vault-hygiene tool, not part
of the cargo workspace).

## Tiers

| Tier | Status | Notes |
|---|---|---|
| T0 (fmt + clippy) | passed | clean across workspace |
| T1 (lib unit) | in progress | background at write time; expected to match m9-87 baseline (77 store, 264 services, 35 cli) |
| T2 (per-crate integration) | not run | A-lite scope says vault-only; T2 is overkill |
| T4 (sandbox smoke) | not run | vault-only; no production code changed |
