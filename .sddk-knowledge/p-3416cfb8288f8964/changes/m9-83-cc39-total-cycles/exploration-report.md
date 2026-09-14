# Exploration: m9-83 CC#39 Total cycles off-by-one

> **Cycle**: `p-3416cfb8288f8964/m9-83-cc39-total-cycles`
> **Status**: exploration-complete
> **Date**: 2026-09-14
> **Path**: B-direct

## Problem

`bash scripts/check_vault_drift.sh` reports `CC#39 reported 1 drift lines`:
`DRIFT C: index says 84, actual 82`.

The drift has been pre-existing across multiple cycles (m9-80 noted
"CC#39 off-by-one — was off-by-one after m9-79 archival sweep; m9-80 +
m9-81 each added one row without correcting the underlying count",
m9-82 noted "Total cycles 84 vs 82 — i.e. 2 missing folders including
m9-78"). Neither cycle closed it because they were scoped to their own
deliverables.

## Root cause

`cycles/index.md` line 115 says `Total cycles | 84` but the file
contains 82 rows starting with `m9-01` and ending with `m9-82`.

CC#39 (in `maintenance/vault-drift-sweep.md`) computes the actual count
as the sum of:
- `cycle-artifacts/m9-*` (legacy): 2 folders
- `cycle-artifacts/p-3416cfb8288f8964/m9-*`: 79 folders
- `.sddk-knowledge/.../changes/m9-*` not in cycle-artifacts: 1 folder (m9-54)
- **Total: 82**

There are no missing folders. The off-by-2 is purely in the metadata
field, not in any actual cycle row. The "Total cycles | 84" line was
manually bumped over time and never reconciled against the actual
folder count.

## Fix

Single-literal change: update line 115 of
`.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` from
`| Total cycles | 84 |` to `| Total cycles | 82 |`.

This closes CC#39 (part C: cycles index Total cycles consistency).

## Why this is B-direct

- One file changed, one number changed.
- No new tests needed (CC#39 is itself a test).
- No architectural decisions.
- No downstream impact.

## Carry-forward closed

- CC#39 (part C: cycles index Total cycles).
