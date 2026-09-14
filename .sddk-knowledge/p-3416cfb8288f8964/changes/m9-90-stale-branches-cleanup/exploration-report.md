# m9-90 Exploration Report — Stale Branches Cleanup

## Scope

m9-90 closes the deferred portion of FIND-M9-89-STALE-BRANCHES-DEFERRED
and FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION by deleting the
9 stale feat/m9-* branches from m9-67..m9-78 that m9-65 missed.

## Discovery

### Stale branch census (pre-m9-90)

| Branch | Tip SHA | Merged? |
|---|---|---|
| feat/m9-67-cc-smoke-test | ef21e1f | YES |
| feat/m9-68-verify-report-files-inventory-backfill | d4328cf | YES |
| feat/m9-69-bounded-join-unit-test | 57c4a10 | YES |
| feat/m9-70-mcp-store-isolation | 54e859f | YES |
| feat/m9-77-attach-runtime | 4bd2da3 | YES |
| feat/m9-78-attach-detach | a2c70fe | YES |
| origin/feat/m9-67-cc-smoke-test | ef21e1f | YES |
| origin/feat/m9-68-verify-report-files-inventory-backfill | d4328cf | YES |
| origin/feat/m9-70-mcp-store-isolation | 54e859f | YES |

**Total**: 9 stale branches (6 local + 3 remote), all merged into main.

### Why m9-65 missed them

m9-65 (97ff56e, 2026-09-13) ran the stale-branch cleanup before
m9-66..m9-78 cycles landed their feat branches. The feat branches
were created during those cycles and merged via `--no-ff`, but the
cleanup tool never re-ran to delete them.

### m5/m3/m2 feat branches — out-of-scope

The discovery also surfaced several feat/m5-*, feat/m3-*, feat/m2-*
branches. These are out-of-scope for m9-90 (different milestone
prefix; not the focus of CC#46/CC#53 which target m9-* prefixes).
Future sweeps can clean them if desired.

## Approach

Single B-direct commit landing `scripts/clean_m9_90_stale_branches.py`:

1. **Discovery**: scan `git branch --list` + `git branch -r --list`
   for `feat/m9-*`, `fix/m9-*`, `chore/m9-*` prefixes.
2. **Safety check (CC#53)**: filter to branches whose tip is merged
   into main via `git merge-base --is-ancestor`.
3. **Deletion**:
   - Local: `git branch -d <branch>` (safe-delete; only deletes merged
     branches; refuses if not merged).
   - Remote: `git push origin --delete <branch>`.
4. **Recovery**: append every action (timestamp, branch, SHA, status)
   to `scripts/branches-deleted-m9-90.log`.
5. **Idempotency**: `--dry-run` for preview; running twice is a no-op.

## Safety properties

1. Refuses to operate outside a git repo.
2. Refuses to operate on the `main` branch itself.
3. Only deletes branches verified merged into main.
4. Logs every deletion before action.
5. Dry-run mode shows what would be deleted without acting.

## Verification

- **T0** (CC drift sweep): CC#46 + CC#53 must show 0 stale branches.
- **T1** (lib unit tests): not required (no Rust touched).
- **T2** (per-crate integration): not required.
- **T4** (sandbox smoke): not required.
