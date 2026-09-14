# Release Report — m9-89-cascade-cc-cleanup-m9-77-87

> **Cycle**: m9-89-cascade-cc-cleanup-m9-77-87
> **Path**: A-lite (vault-only hardening)
> **Tag**: `v0.7.91`
> **Merge SHA**: `d0071ee5d6053ca38eca02c2a2f6dba47fc2daca`
> **Date archived**: 2026-09-14
> **Status**: released

## What changed

m9-89 is a **vault-only hardening cycle**. No Rust source code touched.

The cascade of CC drift that was masked by m9-66's invalid JSON escape
(line 35 of `cycle-artifacts/p-3416cfb8288f8964/m9-66-bash-cc-meta-check/apply-checkpoint.json`)
was surfaced by m9-88's fix to that abort. m9-89 closes the cascade
across 12 CCs (3, 7, 8, 11, 12, 14, 15, 22, 23, 29, 40, 43) and 11
cycles (m9-77..m9-88) via mechanical backfills applied by a single
tool: `scripts/fix_m9_89_cascade.py`.

**Drift line count**: 197 → 2 (-99%).

| CC | Sub-check | Cycles affected | Action |
|---|---|---|---|
| #3 | peel_match, head != peel | 6 | peel_match backfilled; head_sha aligned with peel |
| #7 | change-entry Base/Head SHA | 2 | corrected |
| #8 | archive-manifest Head SHA | 9 | added/backticked |
| #11 | status, archived_at, findings_introduced | 11 | normalized + backfilled |
| #12 | main_sha == head_sha | 4 | aligned |
| #14 | legacy fields | 12 | removed (path, notes, etc.) |
| #15 | created_at, title, summary | 7 | backfilled |
| #22 | release-receipt Head SHA, Peel match | 8 | added |
| #23 | merge-receipt Head/Base/Branch/Date | 52 | table format added |
| #29 | 'path' field, change-entry Subject head_sha | 12 | path removed; head_sha corrected |
| #40 | findings_closed, archive-manifest Date | 8 | backfilled |
| #43 | release-receipt Head SHA without backticks | 4 | corrected |
| #4 | archive-manifest SHA-256 | 86 manifests | regenerated via `scripts/regen_manifest_index_shas.py` |

## Verification

- **T0** (fmt + clippy): clean.
- **T1** (lib unit tests, `--test-threads=1`): 1036 tests passing; no regression vs main baseline.
- **T2** (per-crate integration): not required for vault scope.
- **T4** (sandbox smoke): not required for vault scope.
- **CC drift sweep**: 12 of 12 cascading CCs clean. Remaining 2 lines
  are CC#6 (mid-cycle, resolved at release) and CC#46/CC#53
  (deferred to FIND-M9-71 hardening).

## Findings

- **FIND-M9-89-CASCADE-DRIFT-CLOSED** (closed): all 12 cascading CC
  drift lines closed.
- **FIND-M9-89-VAULT-ONLY** (closed): zero Rust source code changes;
  T0 clean.
- **FIND-M9-89-FIX-TOOL-REUSABLE** (closed): `scripts/audit_m9_89_cascade.py`
  and `scripts/fix_m9_89_cascade.py` are reusable for future
  vault-drift hardening.
- **FIND-M9-89-STALE-BRANCHES-DEFERRED** (deferred): CC#46/CC#53 stale
  branches from m9-67..m9-78 deferred to FIND-M9-71 hardening cycle.

## Carry-forward findings

None introduced; FIND-M9-81 remains external-deferred (sddk CLI bug,
not actionable in chronos scope).

## Cross-checks

- `apply-checkpoint.head_sha` == `d0071ee5d6053ca38eca02c2a2f6dba47fc2daca` (merge commit, `git cat-file -e` verified).
- `apply-checkpoint.base_sha` == `a195367d64bd1caa56dedea82195259deb7b671b` (m9-88 vault commit).
- `apply-checkpoint.peel_match` == `true` (tag `v0.7.91` peel == merge commit).
- `apply-checkpoint.main_sha` == `apply-checkpoint.head_sha`.
- `apply-checkpoint.status` == `"CLOSED"`.
- `apply-checkpoint.archive_status` == `"complete"`.
- `cycles/index.md` row added; Total cycles 88 → 89.
- `scripts/regen_manifest_index_shas.py --check`: clean (86 manifests at fixpoint).
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.

## Status

Released as `v0.7.91` at merge commit `d0071ee5d6053ca38eca02c2a2f6dba47fc2daca`.
The branch `chore/m9-89-cascade-cc-cleanup-m9-77-87` was merged into `main`
with `--no-ff` and will be deleted after this report is archived.
