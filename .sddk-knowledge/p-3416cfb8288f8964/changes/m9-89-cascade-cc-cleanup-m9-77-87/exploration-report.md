# Exploration Report — m9-89 cascade CC cleanup across m9-77..m9-88

## Context

m9-88 closed the m9-66 JSON abort that was masking ~150 lines of
cascading CC drift across 12 CCs and 11 cycles (m9-77..m9-87). m9-88's
fix exposed the cascade as 12 drift lines from `bash
scripts/check_vault_drift.sh`:

```
DRIFT: CC#3 reported 12 drift lines
DRIFT: CC#7 reported 1 drift lines
DRIFT: CC#8 reported 9 drift lines
DRIFT: CC#11 reported 30 drift lines
DRIFT: CC#12 reported 5 drift lines
DRIFT: CC#14 reported 12 drift lines
DRIFT: CC#15 reported 8 drift lines
DRIFT: CC#22 reported 13 drift lines
DRIFT: CC#23 reported 39 drift lines
DRIFT: CC#29 reported 13 drift lines
DRIFT: CC#40 reported 13 drift lines
DRIFT: CC#43 reported 5 drift lines
```

Total: ~197 drift lines (the "12 CCs × varying line counts" sum).

## Cascade root cause

The m9-83 cycle introduced the "cycle-artifacts commit + SHA-cascade
fixpoint" pattern as a workaround for the CC#42 tag-peel fixpoint
problem. The pattern is:

1. The cycle's branch lands a refactor commit at SHA X (the cycle's
   "real" head).
2. Cycle artifacts are written at SHA Y (the "cycle-artifacts commit").
3. The branch merges into main with `--no-ff` at SHA Z (the merge commit).
4. Tag `v0.7.NN` is pre-created at Y, then moved to Z.

CC#3 wants `head_sha == remote_tag_peel`. In this pattern, `peel == Z`
(merge commit), but `head_sha == X` (refactor commit). So `head != peel`
in the docs-peel era is reported as drift.

m9-83..m9-87 adopted this pattern without retroactively normalizing
their cycle artifacts. The cascade accumulated.

## Cascade scope

| Cycle | Pattern adopted? | Notes |
|---|---|---|
| m9-77 | yes (tag at refactor, no merge commit) | peel = refactor, head = refactor. But peel was the refactor, not merge. |
| m9-78 | yes (fix-peel) | era=fix-peel; CC#3 exempt |
| m9-79 | yes | peel=head=merge commit, all aligned |
| m9-80 | yes | peel=merge, head=refactor → drift |
| m9-81 | yes | peel=merge, head=refactor → drift |
| m9-82 | yes | peel=merge, head=refactor → drift |
| m9-83 | yes | peel=merge, head=refactor; new fields required by CC#40/CC#15/CC#14 |
| m9-84 | yes | peel=merge, head=refactor |
| m9-85 | yes | peel=merge, head=cycle-artifacts (Y, not X) |
| m9-86 | yes | peel=merge, head=cycle-artifacts (Y) |
| m9-87 | yes | peel=merge, head=refactor; new fields required by CC#55 Files Inventory |
| m9-88 | yes | peel=merge, head=refactor |

## Resolution pattern

For cycles with `head != peel` in docs-peel era, the canonical fix is to
**align `head_sha` with `peel`** (= merge commit). This restores CC#3
compliance without breaking any other invariant.

For cycles missing fields (status, archived_at, findings_introduced,
created_at, title, summary, findings_closed, Head SHA in
archive-manifest, Head SHA / Peel match in release-receipt, etc.), the
canonical fix is mechanical backfill from existing data.

## Approach

Build two scripts:

1. `scripts/audit_m9_89_cascade.py` — per-cycle, per-CC fix catalog
   (JSON output). Useful for documentation and follow-up cycles.
2. `scripts/fix_m9_89_cascade.py` — applies the mechanical backfills.
   Idempotent: running it on a clean tree produces 0 changes.
   `--dry-run` mode previews changes without writing.

Both scripts are reusable for future vault-drift hardening cycles.

## Scope decision

**A-lite** (vault-only hardening, no Rust code). Rationale:
- No production code touched.
- 12 CCs across 11 cycles — bounded scope.
- All fixes are mechanical (no semantic judgement).
- T0 + T2 sufficient (no T3+ needed).

## Out-of-scope (deferred)

- **CC#46/CC#53 stale branches** (12 merged-but-un-deleted feat/*
  branches): FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION.
  Bundle with future cleanup cycle.
- **FIND-M9-81** (sddk CLI bug): external-deferred from m9-88.
- **CC#6 mid-cycle drift** (terms vs cycles index): resolves
  automatically when m9-89 row is added during release.

## Expected outcome

After m9-89:
- 197 drift lines → 2 (mid-cycle CC#6 + deferred CC#46/53).
- 12 cascading CCs (3, 7, 8, 11, 12, 14, 15, 22, 23, 29, 40, 43) all clean.
- CC#4 (archive-manifest SHA-256) regenerated.
- Zero Rust source code touched.
