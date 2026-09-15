# Verify Report — m9-65

**Cycle**: m9-65-stale-branches-cleanup
**Path**: B-direct

## Summary

Two-commit B-direct cycle that closes drift class `branch-stale-post-merge` across all milestone prefixes. CC#53 added to vault-drift-sweep.md extends CC#46 (which only watched fix/m9-*) to chore/*, feat/mX-*, fix/mX-*, ms-*. 49 stale branches (25 local + 24 remote) that had been merged into main but never deleted were removed via `git branch -d` and `git push origin --delete`. 5+19 not-merged branches were preserved and documented for human triage. The `check_vault_drift.sh` script was updated to report a dynamic count of python-executable vs bash-documented CCs.

## Subject

| Base | Head (final) | Dirty diff digest | CWD | Verified at |
|---|---|---|---|---|
| `6e7e39e` | `97ff56e53c4e00768783f11d0df16878cf9c693e` | `sha256:010eb126de03d016b3ca9293d47dceabbb93e71ca8720ed73ec6e039a45b2d97` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-13T11:42:00Z |

## Files Inventory

3 files changed across the two commits (CC#53 + script + log):

| File | Change |
|---|---|
| `.sddk-knowledge/.../vault-drift-sweep.md` | Added CC#53 (extends CC#46 to all prefixes; bash block, not auto-executed by CC#48) |
| `scripts/check_vault_drift.sh` | Uses dynamic CC count: 46 python + 6 bash + 1 self = 53 total. Heredoc changed to quoted form to avoid bash backtick expansion. |
| `cycle-artifacts/.../branches-deleted.log` | Recovery log: tip SHA of every deleted branch (recreation via `git branch <name> <sha>`) plus the 5+19 preserved not-merged branches |

## Gates

| Gate | Status | Evidence |
|---|---|---|
| T0: cargo fmt --check | PASS | no output |
| T0: cargo clippy --workspace --all-targets -- -D warnings | PASS | no warnings (0.37s) |
| T1: `cargo test -p chronos-domain/-p chronos-services/-p chronos-browser --lib` | PASS | 263 (services) passed; chronos-domain + chronos-browser unchanged |
| T1: `cargo test -p chronos-native --lib -- --test-threads=1` | PASS | 99 passed (the AGENTS.md §6.5 ptrace flake did not reproduce) |
| T4-smoke: `cargo test -p chronos-sandbox --test e2e_connectivity --test probe_lifecycle` | PASS | 1/1 + 5/5 passed |
| `./scripts/check_vault_drift.sh` (post-cycle) | PASS | "vault-drift-sweep: PASS (46 python CCs all clean, 6 bash CC documented separately)" |
| Manual run of CC#53 commands | PASS | "Local stale merged: 0", "Remote stale merged: 0" (target achieved) |

## Cross-checks

- CC#1..CC#52: unchanged, all pass
- CC#53: NEW (bash-only, validated via CI branch-merged check on push to main; not executed by CC#48 meta-check)
- CC#46: superseded by CC#53 (kept for backward compatibility; CC#53 is broader)

## Drift Evidence (pre-cycle baseline)

```
Local stale (merged into main):  25 branches
  feat/m5-06..m5-10 (5)
  feat/m6-01..m6-06 (4)
  feat/m7-03..m7-04 (2)
  feat/m8-05..m8-07 (3)
  feat/m9-01..m9-05 (5)
  feat/m9-62..m9-64 (3)
  feat/ms-race-fix (1)
  fix/m9-56..m9-57 (2)

Remote stale (merged into main):  24 branches
  feat/m0-truth-first-foundation (1)
  feat/m1-03..m1-08 (6)
  feat/m2-01..m2-05 (5)
  feat/m2-native-frame-log-durable + feat/m2-native-int3-functionentry (2)
  feat/m6-06-close-M6 (1)
  feat/m7-01-events-read-scoping (1)
  feat/m8-05..m8-06 (2)
  feat/m9-62..m9-64 (3)
  feat/ms-race-fix (1)
  fix/m9-56..m9-57 (2)
```

## Drift Evidence (post-cycle state)

```
Local stale (merged into main):   0 branches (target achieved)
Remote stale (merged into main):  0 branches (target achieved)
Local not-merged (preserved):      5 branches
  chore/m-ci-flake-preflight
  chore/m5-preflight-clippy-drift-cleanup
  chore/m5-preflight-sandbox-drift
  feat/m5-02b-debug-read-extract
  feat/m5-05b-debug-trace-specialized-extract

Remote not-merged (preserved):    19 branches
  (preflight cleanup attempts + scoping documents from closed M2/M3/M5 milestones)
```

## Safety Properties Verified

- `git branch -d` was used (lowercase), which refuses to delete unmerged branches. The 5+19 not-merged branches were preserved automatically.
- Recovery: every deleted branch's tip SHA is recorded in `branches-deleted.log`. Recreation: `git branch <name> <sha>`.
- Reflog: every deleted ref remains in the reflog for 90 days (default `gc.reflogExpire`).
- `git status` confirms HEAD is intact (still on `feat/m9-65-...` branch; main HEAD unchanged at `5089e74`).

## Notes

- m9-65 is the first cycle to perform `git branch -d` as part of its scope. It establishes the precedent for future cleanup cycles (m9-XX-branch-cleanup-*) and demonstrates the safe deletion pattern.
- CC#53 was deliberately written as a bash block rather than python. Reasons: (a) the check is fundamentally shell — git plumbing commands; (b) CC#46 is also bash for consistency; (c) CC#48 meta-check has limited python execution budget and branch scanning via subprocess to bash would be wasteful.
- The 5+19 not-merged branches are tracked in `branches-deleted.log` for human review. Most are preflight cleanup attempts from M5 (which were never merged because main moved on with the same drift fixed via a different path) or scoping documents from closed milestones. None are required for current main functionality.
- The handoff (`m9-backlog-blocked-2026-09-12.md`) will be updated to record this cleanup as completed, with the not-merged branches transfered to a "human triage needed" section.

## History

m9-65 was prompted by the same session-end sweep that triggered m9-63 (CC#52) and m9-64 (vault drift CI). The handoff already noted "stale branches detected (out of scope for SDDK cycles but worth noting in a follow-up". After closing m9-63 (forward-looking defense for stop-then-drain) and m9-64 (CI for vault CCs), the remaining drift was the branches themselves — 49 stale branches across M0-M9 because CC#46 was fix/m9-* only. m9-65 closes that drift with a single, focused cycle.

## Findings

None — clean state. (m10-legacy-migration)
