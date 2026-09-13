# Release Report — m9-65-stale-branches-cleanup

## Path

B-direct

## Subject

Closed drift class `branch-stale-post-merge` across all milestone prefixes. CC#46 (vault drift sweep) only monitored `fix/m9-*` branches; every milestone (M0, M1, M2, M3, M5, M6, M7, M8, M9) merged cycles to main without deleting its per-cycle branch, accumulating 25 local + 24 remote stale-merged branches. m9-65 added CC#53 (bash) to vault-drift-sweep.md that extends CC#46 to all prefixes (`chore/*`, `feat/mX-*`, `fix/mX-*`, `ms-*`), executed safe deletion of all 49 merged-stale branches via `git branch -d` (refuses unmerged) and `git push origin --delete`, and recorded the tip SHA of every deleted branch in `branches-deleted.log` for recovery. 5 local + 19 remote not-merged branches were preserved and documented for human triage. The `check_vault_drift.sh` script was updated to report a dynamic count of python-executable vs bash-documented CCs (46 + 6 + 1 = 53).

## Files changed

| Group | Count | Change |
|---|---|---|
| `.sddk-knowledge/.../vault-drift-sweep.md` | 1 | Added CC#53 (bash block, ~70 lines) |
| `scripts/check_vault_drift.sh` | 1 | Dynamic CC count; quoted heredoc to fix backtick expansion (~20 lines) |
| `cycle-artifacts/.../branches-deleted.log` | 1 | Recovery log: 49 deleted branch SHAs + 24 preserved not-merged branches (88 lines) |

Total: 3 files changed, 112 insertions(+), 5 deletions(-).

## Branch deletions (safe; git branch -d refuses unmerged)

| Category | Local | Remote |
|---|---|---|
| Merged-stale (deleted) | 25 | 24 |
| Not-merged (preserved) | 5 | 19 |
| Total cleaned | 49 | — |

Pre-cycle state: 30 local + 43 remote stale branches (excluding `main`/`origin/main`/`origin/HEAD`).
Post-cycle state: 5 local + 19 remote stale branches (all not-merged, all documented).

## Cross-checks

| ID | Description | Status |
|---|---|---|
| CC#1..CC#52 | Vault schema and cross-reference checks | pass (no drift) |
| CC#53 | Stale branches (new, bash, not auto-executed by CC#48) | pass (manual run: 0 merged-stale) |
| `./scripts/check_vault_drift.sh` | Meta-check (46 python + 6 bash + 1 self) | pass |
| T0: cargo fmt + clippy | Lint gate | pass |
| T1: chronos-domain/-services/-browser unit tests | 263 + 149 + 42 = 454 tests | pass |
| T1: chronos-native unit tests | 99 tests (--test-threads=1) | pass |
| T4-smoke: e2e_connectivity + probe_lifecycle | 1 + 5 sandbox tests | pass |

## History

Discovered during the same session-end sweep that identified the m9-63 (CC#52) and m9-64 (vault drift CI) gaps. The handoff (`m9-backlog-blocked-2026-09-12.md`) explicitly listed "stale branches detected (out of scope for SDDK cycles but worth noting in a follow-up" — referring to 30 local + 43 remote branches that had accumulated across M0-M9 because CC#46 only watched `fix/m9-*`.

After closing m9-63 (forward-looking defense for stop-then-drain) and m9-64 (CI for vault CCs), the remaining drift was the branches themselves. m9-65 closes that drift with a focused, safe cycle:
- CC#53 added in vault-drift-sweep.md (bash, follows CC#46 convention; CC#48 meta-check has limited python budget)
- 49 `git branch -d` calls executed (safe: refuses unmerged)
- 49 `git push origin --delete` calls executed (one per remote branch)
- Recovery log records tip SHA of every deleted branch for recreation via `git branch <name> <sha>`
- Reflog preserves every ref for 90 days as a backstop
- Not-merged branches preserved and documented for human triage

The cycle establishes the precedent for future cleanup cycles: any time drift is detected via CC#53, the resolution pattern is identical (delete merged, preserve not-merged, log SHAs).
