# Merge Receipt — m9-18-apply-checkpoint-metadata-drift

| Campo | Valor |
|---|---|
| Cycle ID | `m9-18-apply-checkpoint-metadata-drift` |
| Path | B-direct |
| Branch | `fix/m9-18-apply-checkpoint-metadata-drift` |
| Base SHA | `cc995cdd2cf41bd0638a548fc3db169763bfc38d` (main @ start of cycle) |
| Head SHA | `6dce3736df06d4fe09db861ad43a3667c0f0bc25` |
| Tag | `v0.7.16` |
| Status | MERGED |

## Fix commit

| SHA | Subject |
|---|---|
| `6dce3736df06d4fe09db861ad43a3667c0f0bc25` | fix(m9-18): apply-checkpoint metadata drift — archived_at backfill + findings_introduced + status normalization + cross-check #11 |

## What was fixed

Three classes of apply-checkpoint.json metadata drift across prior cycles:

**1. archived_at backfill (m9-11, m9-12, m9-13)**

m9-14 introduced the `archived_at` field but didn't backfill the
prior 3 cycles that had archive-manifest.md files. Backfilled from
the first-commit-time of each cycle's archive-manifest:

| Cycle | archived_at | Source commit |
|---|---|---|
| m9-11 | `2026-09-12T08:26:41Z` | `f4818d13928c0` (2026-09-12 10:26:41 +0200) |
| m9-12 | `2026-09-12T08:55:40Z` | `bfb9edeeea1a5` (2026-09-12 10:55:40 +0200) |
| m9-13 | `2026-09-12T09:59:02Z` | `c4f1237f878c9` (2026-09-12 11:59:02 +0200) |

**2. findings_introduced field (7 cycles)**

m9-09 introduced the `findings_introduced` field but 7 cycles were
never updated. Added `{"no_action": []}` to all:

m9-03, m9-05, m9-06, m9-07, m9-08, m9-10 (m9-04 already had it)

**3. status normalization (8 cycles)**

Pre-m9-11 convention used lowercase `"status": "closed"`. m9-11+
uses uppercase `"CLOSED"`. Normalized all 8 affected cycles:
m9-03, m9-04, m9-05, m9-06, m9-07, m9-08, m9-09, m9-10.

## Procedure extension

Adds **cross-check #11** to `vault-drift-sweep.md` enforcing:
- `status` must be `'CLOSED'`
- `archived_at` must not be `null` when archive-manifest.md exists
- `findings_introduced` must be present (with `no_action` subfield)

## Why this matters

The apply-checkpoint.json schema has evolved across the m9 cycle
series (m9-09 added `findings_introduced`, m9-14 added `archived_at`,
m9-11 added uppercase `CLOSED` convention). Each new field was only
applied to the cycle that introduced it, leaving prior cycles
inconsistent. m9-18 catches all three classes of legacy schema drift
and adds cross-check #11 to enforce the post-m9-11 schema going forward.
