# Release Report — m9-76-cc4-regen-tool-in-repo

## Cycle

| Campo | Valor |
|---|---|
| Cycle | `m9-76-cc4-regen-tool-in-repo` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `e59c44e64f40b475a3541c6de3e9176ebf5d0cdf` |
| Code SHA | `fd2579f9b1ded8f71687f696d4b5b1f2234be892` |
| Branch | `feat/m9-76-cc4-regen-tool-in-repo` |
| Tag | `v0.7.78` |
| Findings closed | 1 (`FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO`, low) |
| Findings introduced | 0 new; 4 inherited low rows re-listed |

## What shipped

The CC#4 gate now has its repair half in the repository.
`scripts/regen_manifest_index_shas.py` rewrites every stale Artifact-index SHA-256
row to a bounded fixpoint, preserves the self-referential row (a file cannot
contain its own hash, and CC#4 skips it by path comparison), and leaves a row whose
target file no longer exists byte-for-byte untouched (CC#4 guards every comparison
with `[ -f "$path" ]`, so a dangling row is invisible to the gate and blanking it
would manufacture drift). `--check` writes nothing and exits `1` naming each stale
row, which is the machine verification that the ritual ran; `--dry-run` reports and
exits `0`; `--verbose` also lists untouched manifests.

The proof that the tool agrees with the gate is executable, not asserted:
`scripts/smoke_test_ccs.sh` gained a sixth check that runs the unit tests, asserts
`--check` is clean on a clean clone, injects the same stale SHA `test_cc4` injects
and asserts that **both** `--check` and CC#4 flag it with the row named, then
rewrites with the tool and asserts both go green. `setup_work_copy()` now overlays
the working-tree vault tooling, so the suite exercises the tree about to merge
rather than HEAD alone — in m9-75 the clone-of-HEAD behaviour cost a run.

13 zero-dependency unit tests cover the parsing and the rules. Documentation:
`maintenance/vault-drift-sweep.md` gains a repair-tool paragraph under CC#4 and a
resolution line that now says to run the tool instead of hand-editing, and
`AGENTS.md` §5 makes archive-manifest SHA rows generated rather than hand-edited
with the commands in §7.

## Commits

| SHA | Subject |
|---|---|
| `fd2579f9b1ded8f71687f696d4b5b1f2234be892` | `chore(vault): land the CC#4 manifest-SHA regeneration as a repo script (m9-76)` |
| `613b326d24191fd0ceaba5a500ef5b59857bd68d` | `feat(m9-76): cycle artifacts (apply-checkpoint, verify, release-report)` (tag `v0.7.78`) |

## Diff summary

| File | Lines |
|---|---|
| `scripts/regen_manifest_index_shas.py` | new, 257 |
| `scripts/tests/test_regen_manifest_index_shas.py` | new, 179 (13 tests) |
| `scripts/smoke_test_ccs.sh` | +140/−2 (`test_regen_script()` sixth check, working-tree overlay) |
| `AGENTS.md` | +12 (§5 rule, §7 commands) |
| `.sddk-knowledge/.../maintenance/vault-drift-sweep.md` | +26/−2 (repair-tool paragraph, resolution line) |
| `.sddk-knowledge/.../archive/m9-67\|68\|72\|73\|74\|75/archive-manifest.md` | regenerated rows (1–3 each) |

`git diff --numstat` for the code commit: 623 insertions, 13 deletions across 11
files. No Rust source file changed.

## Verification

Verify report: `cycle-artifacts/p-3416cfb8288f8964/m9-76-cc4-regen-tool-in-repo/verify-report.md`.

- T0: `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
- T1: 921 passed (workspace lib minus `chronos-sandbox`/`chronos-native`/`chronos-e2e`) plus 101 passed (`chronos-native --lib --test-threads=1`, 13.02 s).
- Unit: `python3 scripts/tests/test_regen_manifest_index_shas.py` 13/13 OK.
- CC#4 tool: `--check` clean over 75 manifests.
- Vault: `bash scripts/check_vault_drift.sh` PASS (48 python + 7 bash CCs).
- Smoke: `./scripts/smoke_test_ccs.sh` 6/6, `CC detection chain is healthy`.
- T4-canary: `e2e_connectivity` 1/1 and `store_open_failure` 2/2 with `CHRONOS_MCP_PATH` exported.

## Falsification evidence

| # | Reverted | Observed result |
|---|---|---|
| 1 | stale-row print quiet-gated again | naming unit test FAILED; smoke FAILED with `regen: --check did not name the drifted row` (found organically) |
| 2 | self-row comparison removed | two unit tests FAILED; the real tree reported a manifest's own path as stale |
| 3 | `--check` failure branch disabled | two unit tests FAILED; smoke FAILED |
| 4 / 4b | dangling-row guard removed / row blanked and counted | `FileNotFoundError` / preservation test FAILED with `1 != 0` |
| 5 | fixpoint loop reduced to one pass | interlinked-manifest test FAILED |
| 6 | write path emits a wrong hash | smoke FAILED with `regen: rewrite exit=2 (expected 0)` |

Each restored byte-identically (`cmp` against
`/home/rubentxu/.jcode/scratch/regen.m9-76-good.py`) and re-run green. Two limits
are recorded: phase (d)'s true-SHA assertion was never the failing line (the loop
bound catches a wrong repair first), and the dangling-row rule is only observable
under 4b because the inert variant is a no-op.

## Cross-checks

- CC#1..CC#56: pass, counts unchanged (48 python + 7 bash). No new CC: CC#4
  already covers SHA consistency and a duplicate check would be noise. The smoke
  suite runs 6 checks instead of 5 and still asserts the same counts.
- CC#12: `main_sha == head_sha == remote_tag_peel` for tag `v0.7.78` (filled in
  the post-release commit, since the tag points at the artifacts commit).
- CC#4: `--check` clean over 75 manifests after the cycle's own regeneration; six
  historical manifests had rows regenerated for the three edited files and every
  self row was left at its committed value.
- CC#24 / CC#31: `change-entry.md` and `archive-manifest.md` carry `Base SHA` and
  `Head SHA` rows.
- CC#36 / CC#55 / CC#39: `verify-report.md` carries `Path` at the head and
  `## Files Inventory`; `verify-findings.json` carries `lens_summary` after
  `subject`; this report and `archive-manifest.md` carry `## Cross-checks`.
- CC#33: `## Subject` present in `change-entry.md`, `verify-report.md` and
  `verify-findings.json`.
- CC#46 / CC#53: no `fix/m9-*` branch created or left behind; the cycle branch is
  deleted after the merge.
- No `docs/propuestas/` file touched; no test deleted, weakened or skipped.

## Follow-ups (deferred)

- **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** (low, unchanged).
- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low, unchanged).
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low, unchanged).
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, mitigated: the
  churn is automated one command deep; the design shape is unchanged).
- **Sandbox warm-up ordering** and **5+19 not-merged branches triage**: preserved
  (the latter needs human review).
