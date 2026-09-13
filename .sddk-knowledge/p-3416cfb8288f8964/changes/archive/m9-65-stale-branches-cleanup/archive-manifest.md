# Archive Manifest — m9-65-stale-branches-cleanup

## Summary

m9-65 closes the drift class `branch-stale-post-merge` across all milestone prefixes (CC#46 only watched `fix/m9-*`). Two B-direct commits landed as 97ff56e on feat/m9-65-stale-branches-cleanup. CC#53 added (bash, not auto-executed by CC#48). 49 stale branches (25 local + 24 remote) that had been merged into main since M0 were deleted via `git branch -d` and `git push origin --delete`. Recovery log records tip SHA of every deleted branch. 5 local + 19 remote not-merged branches were preserved and documented for human triage.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-65-stale-branches-cleanup |
| Base SHA | `6e7e39ec35f7bd6c882833206d72b2d46fab1199` |
| Head SHA | `97ff56e53c4e00768783f11d0df16878cf9c693e` |
| Path | B-direct |
| Date | 2026-09-13T11:42Z |
| Branch | `feat/m9-65-stale-branches-cleanup` |
| Tag | `v0.7.67` |
| Tag peel SHA | `97ff56e53c4e00768783f11d0df16878cf9c693e` |
| Peel match | `97ff56e53c4e00768783f11d0df16878cf9c693e` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-65-BRANCH-DRIFT]`
- **`verify-findings.json`**: 1 finding (FIND-M9-65-BRANCH-DRIFT, severity low), verdict `pass_with_findings`
- **`verify-report.md`**: Subject, Files Inventory, Gates (T0 + T1 + T4-smoke + manual CC#53), Drift Evidence (pre/post-cycle), Safety Properties, Notes, History
- **`merge-receipt.md`**: `Base SHA | 6e7e39e…`, `Head SHA | 97ff56e…`
- **`release-receipt.md`**: `Remote tag | v0.7.67`, `Peel match | 97ff56e…` (true)
- **`branches-deleted.log`**: 49 deleted branch SHAs (recoverable via `git branch <name> <sha>`) + 24 preserved not-merged branches (with last-commit subject)

## Tangential modifications

3 files changed across two commits (112 insertions, 5 deletions):

| File | Net change |
|---|---|
| `.sddk-knowledge/.../vault-drift-sweep.md` | +~70 (CC#53 added) |
| `scripts/check_vault_drift.sh` | +20, -5 (dynamic CC count; quoted heredoc) |
| `cycle-artifacts/.../branches-deleted.log` | new file (+88) |

49 git branch operations executed (25 local `git branch -d` + 24 remote `git push origin --delete`):

| Prefix | Local deleted | Remote deleted |
|---|---|---|
| `chore/*` | 0 | 0 |
| `feat/m0-*` | 0 | 1 |
| `feat/m1-*` | 0 | 6 |
| `feat/m2-*` | 0 | 7 |
| `feat/m3-*` | 0 | 0 |
| `feat/m5-*` | 5 | 0 |
| `feat/m6-*` | 4 | 1 |
| `feat/m7-*` | 2 | 1 |
| `feat/m8-*` | 3 | 2 |
| `feat/m9-*` | 5 (+3 from this session's earlier cycles: m9-62, m9-63, m9-64) | 3 |
| `feat/ms-race-fix` | 1 | 1 |
| `fix/m9-*` | 2 | 2 |

## Cross-checks

- CC#1..CC#52: pass
- CC#48 meta-check: pass (46 python CCs all clean, 6 bash CC documented separately)
- CC#53 (new, bash, manual run): pass — 0 merged-stale branches remaining; 5 local + 19 remote not-merged preserved

## Follow-ups (deferred)

- **5 local + 19 remote not-merged branches**: tracked in `branches-deleted.log` and the cycle handoff. These are preflight cleanup attempts (chore/m-ci-flake-preflight etc.) and scoping documents from closed milestones (M2, M3, M5). None required for current main functionality. Human triage recommended before milestone close-out.
- **CC#46 supersedure**: CC#53 is broader; CC#46 (fix/m9-* only) is now redundant. Could be removed in a future cycle, but keeping for backward compatibility costs nothing.
- **CC for `## Files Inventory` in verify-report**: still missing for 22 cycles m9-32..m9-53 (cosmetic). Low priority; deferred.
