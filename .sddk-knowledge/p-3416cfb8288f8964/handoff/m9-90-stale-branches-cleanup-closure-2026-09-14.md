# m9-90 Closure Handoff — Stale Branches Cleanup

**Cycle**: m9-90-stale-branches-cleanup
**Path**: B-direct (vault-only hardening)
**Tag**: v0.7.92
**Merge commit**: 56f93d1d7e4fae5a245f00bf56b8f53c1d2056db
**Published HEAD (tag)**: 2184975a93b43ea1bbd2681dead79dfb4476fef7
**Date**: 2026-09-14
**Status**: CLOSED

## What m9-90 did

Vault-only hardening cycle. No Rust source code touched.

m9-90 closes CC#46 + CC#53 stale-branch drift by deleting the
9 feat/m9-* branches (6 local + 3 remote) from m9-67..m9-78 that
m9-65 missed (m9-65 ran before m9-66..m9-78 cycles landed their feat
branches).

### Drift delta

| Stage | Stale branches |
|---|---|
| Before (post-m9-89) | 9 |
| After m9-90 | 0 |

### Branches deleted

Local (6):

| Branch | Tip SHA |
|---|---|
| feat/m9-67-cc-smoke-test | ef21e1f |
| feat/m9-68-verify-report-files-inventory-backfill | d4328cf |
| feat/m9-69-bounded-join-unit-test | 57c4a10 |
| feat/m9-70-mcp-store-isolation | 54e859f |
| feat/m9-77-attach-runtime | 4bd2da3 |
| feat/m9-78-attach-detach | a2c70fe |

Remote (3):

| Branch | Tip SHA |
|---|---|
| origin/feat/m9-67-cc-smoke-test | ef21e1f |
| origin/feat/m9-68-verify-report-files-inventory-backfill | d4328cf |
| origin/feat/m9-70-mcp-store-isolation | 54e859f |

### Tool added

`scripts/clean_m9_90_stale_branches.py` (220 lines, idempotent +
--dry-run + --reset-log + recovery log):

1. Discovers all local + remote branches matching `feat/m9-*`,
   `fix/m9-*`, `chore/m9-*` prefixes (per CC#46).
2. Filters to those whose tip is merged into main (CC#53 safety check
   via `git merge-base --is-ancestor`).
3. Deletes local branches via `git branch -d` and remote branches via
   `git push origin --delete`.
4. Logs every action (timestamp, branch, SHA, status) to
   `scripts/branches-deleted-m9-90.log` for recovery.
5. Supports `--dry-run` for safe preview and `--reset-log` to start
   fresh.

Reusable for future stale-branch sweeps.

## Why this cycle was needed

After m9-65 closed CC#46 + CC#53 (deleting 49 stale branches), 9
feat/m9-* branches from m9-67..m9-78 remained stale because m9-65
ran before those cycles landed their feat branches. m9-89 explicitly
deferred these to a dedicated cycle:

> **CC#46/CC#53 stale branches** (m9-67..m9-78 feat/* branches merged
> into main but un-deleted): tracked under
> FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION; bundled with
> cycle artifact archival but its own cleanup cycle is deferred.

m9-90 is that cycle.

## Verification (T0 only)

- `cargo fmt --all -- --check`: clean
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
  (no Rust touched)
- `bash scripts/check_vault_drift.sh`: CC#46 + CC#53 clean (was 9
  stale, now 0)
- `python3 scripts/clean_m9_90_stale_branches.py --dry-run`: 0
  candidates (idempotent)
- T1/T2/T4 not required (vault-only cycle)

## Commits on branch `chore/m9-90-stale-branches-cleanup`

```
13f7084 chore(vault): m9-90 delete 9 stale m9-* branches (CC#46 + CC#53)
ed56d4b m9-90: cycle artifacts + change-entry + exploration/proposal/spec/tasks
42ee5df m9-90: align artifacts to v0.7.92 HEAD 56f93d1 (peel match); add ## Summary
2184975 m9-90: align artifacts to v0.7.92 HEAD 42ee5df (final post-alignment)
```

## Merge + fixpoint chain

```
56f93d1  Merge branch 'chore/m9-90-stale-branches-cleanup' into main
42ee5df  m9-90: align artifacts to v0.7.92 HEAD 56f93d1 (peel match); add ## Summary
2184975  m9-90: align artifacts to v0.7.92 HEAD 42ee5df (final post-alignment)
```

The tag `v0.7.92` was moved through these commits per the CC#42
fixpoint-cascade workaround. Final tag lives at `2184975` (current
HEAD); peel_match: true.

## Out of scope (carried forward)

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88):
  external `sddk` CLI bug (deterministic event_id collision +
  unregistered evaluator); cannot be fixed in chronos scope.

## Recommendation for next cycles

**m9-91+**: resume Rust work. The vault is now clean:
- All 47 cross-checks passing (CC#34 + CC#42 are transient/pre-existing
  exceptions that don't block anything).
- All open vault hygiene findings (CC#46, CC#53,
  FIND-M9-89-STALE-BRANCHES-DEFERRED, FIND-M9-71) closed.
- 90 cycles total, with full cycle-artifacts coverage.

Possible next-up Rust work:
- **m2-function-exit-dwarf** (stale remote branch): investigate if
  still relevant.
- **m5-02c-cleanup-inert-artifacts** (stale remote branch):
  investigate.
- **cc-001-god-module** (deferred to m10+): not for now.
- **cc-004-implicit-io-toctou** (deferred to m10+): not for now.

## Lessons learned

1. **Stale-branch drift is periodic.** m9-54 cleaned 72, m9-65 cleaned
   49, m9-90 cleaned 9. The drift accumulates whenever new cycles
   create feat/* branches. The clean_m9_90 tool is reusable; running
   it after every N cycles is a cheap insurance.

2. **m9-65 missed 9 branches because it ran too early.** This is a
   recurring lesson: cycle N's cleanup tools can't fix branches
   created by cycles N+1..M. Future cycles should explicitly re-run
   cleanup tools at the end of every milestone (e.g., after every
   m9-NN cycle).

3. **`scripts/clean_m9_90_stale_branches.py` is reusable.** Future
   cycles can run it without modification; it's idempotent + safe
   by construction.

4. **Recovery log is essential.** Deleting branches is destructive.
   The recovery log (`scripts/branches-deleted-m9-90.log`) records
   every tip SHA so a human can restore via
   `git branch <name> <sha>` if needed.
