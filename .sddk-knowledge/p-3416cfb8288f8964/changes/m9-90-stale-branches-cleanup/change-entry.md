# Change: m9-90 stale branches cleanup

## Identification

| Field | Value |
|---|---|
| Cycle ID | `m9-90-stale-branches-cleanup` |
| Workspace | `p-3416cfb8288f8964` |
| Path | B-direct (vault-only hardening) |
| Status | CLOSED |
| Base SHA | `20e2649822c0c24509ff6419a48fe3113591ecf0` |
| Head SHA | 2184975a93b43ea1bbd2681dead79dfb4476fef7 |
| Tag | `v0.7.92` |

## Subject

- base_sha: `20e2649822c0c24509ff6419a48fe3113591ecf0`
- head_sha: 2184975a93b43ea1bbd2681dead79dfb4476fef7
- cycle: m9-90
- branch: `chore/m9-90-stale-branches-cleanup`

## Problem

m9-89 deferred 9 stale feat/m9-* branches (m9-67..m9-78) to a
dedicated cleanup cycle. m9-65 had originally tried to clean these
but ran before m9-66..m9-78 cycles landed their feat branches, so
missed them. The drift accumulated as CC#46 + CC#53 stale-branch
drift.

## Approach

A single B-direct commit landing `scripts/clean_m9_90_stale_branches.py`
(220 lines) — idempotent Python tool that:

1. Discovers all local + remote branches matching `feat/m9-*`,
   `fix/m9-*`, `chore/m9-*` prefixes (per CC#46).
2. Filters to those whose tip is merged into main (CC#53 safety check).
3. Deletes local + remote branches.
4. Logs every action to `scripts/branches-deleted-m9-90.log` for recovery.

## Files changed

| Bucket | Files | Notes |
|---|---|---|
| `scripts/clean_m9_90_stale_branches.py` | NEW | 220 lines, idempotent + dry-run |
| `scripts/branches-deleted-m9-90.log` | NEW | 9 deletions + 0 preserved |
| `git: local feat/m9-* branches` | 6 deleted | m9-67, 68, 69, 70, 77, 78 |
| `git: remote feat/m9-* branches` | 3 deleted | origin/m9-67, 68, 70 |

**Total**: 2 files added + 9 branches deleted.

## Drift delta

Before: 9 stale feat/m9-* branches (6 local + 3 remote) merged into main.
After: 0.

## Out-of-scope

- **FIND-M9-81** (sddk CLI bug): external-deferred from m9-88;
  cannot be fixed in chronos scope.
- **m5-* / m3-* / m2-* feat branches**: out-of-scope for m9-90
  (different milestone prefix; can be cleaned by a future sweep if
  desired).
