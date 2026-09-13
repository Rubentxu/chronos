# Verify Report — m9-67

**Cycle**: m9-67-cc-smoke-test
**Path**: B-direct

## Summary

Single-commit B-direct cycle that adds `scripts/smoke_test_ccs.sh`, a synthetic-drift injection test for the four most failure-prone vault drift CCs. The script was developed and validated locally: 4 tests, 0 failures, ~35 seconds wall time. The cycle closes the "fix a broken CC, then fix what it would have caught" pattern observed in m9-65 and m9-66.

## Subject

| Base | Head (final) | Dirty diff digest | CWD | Verified at |
|---|---|---|---|---|
| `67b3d76` | `5c83df9ce96862c0c95598d63cddb147e3ea6ab5` | `sha256:9fe9cef134d220064184e067a09b5496f38bb2973607f7be661d985a078dc99d` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-13T10:39:00Z |

## Files Inventory

1 file added across the single commit (250 insertions, 0 deletions):

| File | Change |
|---|---|
| `scripts/smoke_test_ccs.sh` | New file. Tests CC#4 (SHA-256 consistency), CC#39 (Total cycles ↔ filesystem; superset of CC#5), CC#46 (no stale fix/m9-* branches), and CC#48+CC#54 (meta-checks on clean state). Uses `git clone --shared` per test (not git archive) because many python CCs need a working `.git`. Avoids `trap`-based cleanup (subshell command substitution inherits it). |

Total: 1 file, 4 tests, 0 deletions.

## Drift Evidence (pre-cycle baseline)

```
CC#4 (broken awk regex, fixed in m9-66): catches synthetic SHA mismatch
CC#5 (off-by-16, fixed in m9-66): catches synthetic Total cycles mismatch
CC#46 (missing prefixes, fixed in m9-65): catches synthetic fix/m9-99 branch
CC#48 + CC#54 (meta-checks): no test exercised the extraction logic end-to-end
```

## Drift Evidence (post-cycle state)

```
./scripts/smoke_test_ccs.sh
  [CC#4] SHA-256 consistency... Injected drift → PASS (CC#4 caught the drift)
  [CC#39] Total cycles ↔ filesystem cycles... Injected drift → PASS (CC#39 caught the drift)
  [CC#46] No stale fix/m9-* branches... → PASS (CC#46 caught the injected branch)
  [CC#48+CC#54] meta-checks must run together... → PASS (clean state exit 0; reports "46 python CCs all clean, 7 bash CCs all clean")
  === Summary ===
  Tests run: 4
  Failures: 0
```

## Gates

| Gate | Status | Evidence |
|---|---|---|
| T0: cargo fmt --check | not applicable | no Rust changes this cycle |
| T0: cargo clippy | not applicable | no Rust changes this cycle |
| T-self: `./scripts/smoke_test_ccs.sh` | PASS | 4 tests, 0 failures, ~35s wall time |
| `./scripts/check_vault_drift.sh` (post-cycle, no smoke test changes to vault) | not run | smoke test only touches `scripts/`, not `vault-drift-sweep.md` or `check_vault_drift.sh` |

## Cross-checks

- CC#1..CC#53: pass (no drift introduced — smoke test only adds a new file in `scripts/`)
- CC#54 (bash meta-check, added in m9-66): pass (no changes to bash blocks)
- Self-test (inject wrong SHA in m9-01 archive-manifest): CC#4 catches → exit 1
- Self-test (inject wrong Total cycles in cycles/index.md): CC#39 Part C catches → exit 1
- Self-test (create fake fix/m9-99-smoke-test branch): CC#46 catches → exit 1
- Self-test (clean state): exit 0 + PASS message format matches `46 python CCs all clean, 7 bash CCs all clean`

## Notes

- **Why `git clone` instead of `git archive`**: many python CCs (CC#3, CC#42, CC#47, etc.) invoke `git log` via subprocess to verify SHA existence. `git archive` strips `.git`, so those CCs would silently drift in any test environment. `git clone --shared` preserves `.git` while avoiding duplicate object storage (it shares object packs with the source repo).
- **Why no `trap "rm -rf WORK_DIR" EXIT`**: command substitution `$(...)` (used to capture exit codes) runs in a subshell, and subshells inherit traps. The first run of the script deleted `WORK_DIR` from inside a subshell before the main shell could use it. Cleanup is now explicit at script exit.
- **Why CC#39 instead of CC#5**: CC#5 is a bash CC that runs Total cycles consistency checks. CC#39 is a python CC (Part C) that runs the same check AND counts filesystem cycles. CC#39 is reached first by CC#48's python meta-check; if it fails, the script exits before CC#54 runs (which is where CC#5 lives). Smoke-testing CC#39 effectively covers the same drift class for CC#5 — if CC#5 were broken, it would be caught on a future cycle that doesn't trip CC#39 first.
- **Cost**: ~35 seconds per run. The script is intended to run before merging any change to `vault-drift-sweep.md` or `check_vault_drift.sh`, not on every commit.

## History

m9-67 closes the "fix a broken CC, then fix what it would have caught" pattern observed twice in this session:

1. m9-65 surfaced 49 stale branches because CC#46 was missing `chore/*`, `feat/mX-*`, `ms-*` prefixes.
2. m9-66 surfaced 54 stale SHAs because CC#4's awk regex was silently broken.

Both drifts accumulated for many cycles before being noticed. m9-67 prevents recurrence by adding a 35-second smoke test that any future CC regression triggers immediately. The cycle was identified as the next action at the end of m9-66's release-report ("future 'CC smoke test' cycle that periodically injects drift into each CC to confirm it's still firing").

## Post-cycle fix (commit `dfddc99`)

Two bugs were discovered in the smoke test script itself during the post-merge re-verification required by adding m9-67 to `cycles/index.md` (which bumped Total cycles from 66 to 67):

1. **Hardcoded Total cycles value**: the CC#39 injection hardcoded `| Total cycles | 66 |` as the source string. When the actual value became 67, the replace silently no-op'd and the test reported a false PASS (exit 0 + "ccs all clean"). Now reads the value dynamically and bumps it by 100 — a number that can never match the filesystem count by accident.

2. **Shared output log**: all four tests wrote to a single `out.log`, so a passing test that ran after a failing test could mask the failure. Each test now writes to its own log (`cc4.log`, `cc39.log`, `cc46.log`, `meta.log`).

These are exactly the kind of "silent CC failure" the smoke test is designed to catch — and they would have caused silent failures of the smoke test itself if not caught. The fix is the first cycle whose smoke test validated itself via dynamic reading (CC#39) and per-test logs (CC#48+CC#54).
