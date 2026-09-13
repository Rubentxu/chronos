# Change: m9-76 cc4 regen tool in repo

## Summary

Closes `FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO`, the low process finding
confirmed for the fifth time in m9-75 — and re-confirmed inside this cycle, when
editing three files staled the artifact rows of six historical manifests.

CC#4 (artifact-index SHA-256 consistency in `archive-manifest.md`) had a **gate**
in `scripts/check_vault_drift.sh` but no **repair tool** in the repository. The
repair was a throwaway python script re-derived every cycle under the agent
scratch directory, and the naive whole-tree version churned the self-referential
rows of the manifests written before m9-11, which then had to be reverted by hand.
Two consequences followed, and the first is the interesting one:

- The gate's logic was never exercised by the tool meant to satisfy it. That is
  how CC#4's broken awk (m9-66) accumulated silent drift across many cycles with
  nothing to contradict it.
- The ritual left no trace a reviewer could re-run, so a correctly regenerated
  table was indistinguishable from a hand-edited one.

The cycle lands the repair half of the gate:

- `scripts/regen_manifest_index_shas.py` rewrites every stale Artifact-index row
  to a bounded fixpoint (`MAX_PASSES = 5`), because rewriting one manifest changes
  the bytes of any manifest another one lists. It mirrors CC#4's two edge rules,
  which is the whole justification for trusting it: the **self-referential row is
  preserved** (a file cannot contain its own hash, and CC#4 skips it by path
  comparison), and a row whose **target file no longer exists is left
  byte-for-byte untouched** and reported as `missing` (CC#4 guards every
  comparison with `[ -f "$path" ]`, so a dangling row is invisible to the gate and
  the tool must not silently blank it).
- `--check` writes nothing and exits `1` naming each stale row. This is the
  machine verification that the ritual was performed, and it is why the tool is
  more than a convenience: the gate now has an executable agreement partner.
  `--dry-run` reports and exits `0`; `--verbose` also lists untouched manifests;
  `--check` with `--dry-run` and a missing manifest both exit `2`.
- `scripts/tests/test_regen_manifest_index_shas.py` adds **13** zero-dependency
  `unittest` cases (row parsing, self row, dangling row, exit codes and
  exclusivity, the fixpoint across two interlinked manifests, discovery, explicit
  arguments, and the naming of stale rows).
- `scripts/smoke_test_ccs.sh` gains a **sixth** check that pins tool and gate
  together in four phases: unit tests pass; `--check` clean on a clean clone;
  injected drift flagged by both `--check` and CC#4 with the row named; the
  rewrite restores the true SHA and both go green. `setup_work_copy()` now also
  overlays the working-tree copies of the vault tooling, so the suite tests the
  tree about to merge rather than HEAD alone — in m9-75 the clone-of-HEAD behaviour
  cost a run, because the post-release commit had to land before the smoke could
  pass.

The tool earned its keep inside its own cycle. Three edits
(`scripts/smoke_test_ccs.sh`, `maintenance/vault-drift-sweep.md`, `AGENTS.md`)
staled rows in six manifests (`m9-67`, `m9-68`, `m9-72`, `m9-73`, `m9-74`,
`m9-75`), and the vault-doc edit staled two more. Previously that was hand work
with a manual revert of self rows; here it was one command plus `--check`.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-76-cc4-regen-tool-in-repo` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `e59c44e64f40b475a3541c6de3e9176ebf5d0cdf` |
| Head SHA | `TBD_HEAD_SHA` |
| Tag | `v0.7.78` |

## Subject

- base_sha: `e59c44e64f40b475a3541c6de3e9176ebf5d0cdf`
- head_sha: `TBD_HEAD_SHA`
- diff_digest: `sha256:bc726c11f39e072964a718c225b4b75f190841accda2cc6202394ca46581355a`
- source commits: `fd2579f` (code + tests + docs), `TBD_ARTIFACTS_COMMIT` (artifacts)
- cycle: m9-76
- branch: `feat/m9-76-cc4-regen-tool-in-repo`
- date: `2026-09-13T17:57Z`
- tag: `v0.7.78`
- findings_closed: 1 (`FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO` low)
- findings_introduced: 0 new; 4 inherited low rows re-listed (one now mitigated)
- new tests: 14 (13 python unit tests + 1 smoke check with four phases)

## Files changed

- (new) `scripts/regen_manifest_index_shas.py` — 257 lines: `ROW`, `sha256`, `default_manifests`, `rows_of`, `rewrite` (self-row and dangling-row preservation), `main` with `--check` / `--dry-run` / `--verbose` and the bounded fixpoint
- (new) `scripts/tests/test_regen_manifest_index_shas.py` — 179 lines, 13 tests, plain `unittest`, no third-party dependency
- (modified) `scripts/smoke_test_ccs.sh` — `test_regen_script()` sixth check (4 phases) and `setup_work_copy()` overlaying the working-tree vault tooling
- (modified) `AGENTS.md` — §5 "Archive-manifest SHA rows are generated, not hand-edited"; §7 vault commands
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` — "Repair tool (added by m9-76)" under CC#4; resolution line now says run the tool, do not hand-edit
- (modified) six historical archive manifests (`m9-67`, `m9-68`, `m9-72`, `m9-73`, `m9-74`, `m9-75`) — regenerated rows for the three edited files
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-76-cc4-regen-tool-in-repo/*`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-76-cc4-regen-tool-in-repo/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-76-cc4-regen-tool-in-repo/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-76 row, `Total cycles` 75 → 76)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (`FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO` terminated as closed, `Last archive`)

## Cross-checks

CC#1..CC#56 pass with counts unchanged (48 python + 7 bash); no new CC was added,
because CC#4 already covers SHA consistency and a duplicate check would be noise.
The smoke suite therefore still asserts `48 python CCs all clean, 7 bash CCs all
clean`, and it now runs **6** checks instead of 5. T0 `cargo fmt` + `clippy
-D warnings` clean. T1: 921 passed across the workspace minus
`chronos-sandbox`/`chronos-native`/`chronos-e2e`, plus 101 passed for
`chronos-native` serial. T4-canary: `e2e_connectivity` 1/1 and
`store_open_failure` 2/2 with `CHRONOS_MCP_PATH` exported, since no Rust file
changed and the canary is a build-and-serve sanity check rather than a regression
gate. CC#4: `--check` clean over 75 manifests after the cycle's own regeneration.
CC#12: `main_sha == head_sha == remote_tag_peel` for `v0.7.78` (filled in the
post-release commit, because the tag points at the artifacts commit).

## Falsification evidence

| # | Reverted | Observed result |
|---|---|---|
| 1 | stale-row print quiet-gated again | `test_stale_row_is_named_in_check_output` FAILED (`'src.txt' not found in …`); smoke FAILED with `regen: --check did not name the drifted row` — **found organically** on the first smoke run, which is why the unit test exists |
| 2 | self-row comparison replaced by `if False:` | `test_clean_manifest_exits_zero` and `test_self_row_is_preserved` FAILED; the real tree reported a manifest's own path as stale (expected `903fe3b5…`) — the unfixable churn the finding describes |
| 3 | `--check` failure branch disabled | two unit tests FAILED; smoke FAILED (`regen: unit tests failed`) |
| 4 | dangling-row guard removed | `FileNotFoundError` on the missing target |
| 4b | dangling row blanked and counted as changed | `test_missing_target_is_preserved_and_invisible_to_check` FAILED — `AssertionError: 1 != 0` |
| 5 | fixpoint loop reduced to a single pass | `test_fixpoint_across_interlinked_manifests` FAILED |
| 6 | write path emits a wrong hash | smoke FAILED with `regen: rewrite exit=2 (expected 0)` — a wrong hash never converges, so the loop bound fires before the true-SHA assertion |

Each mutation was reverted and the file compared byte-identically (`cmp`) against
`/home/rubentxu/.jcode/scratch/regen.m9-76-good.py` before the next. Restored
state re-verified green: 13/13 unit tests, `--check` clean over 75 manifests,
vault gate PASS, smoke 6/6.

Two limits are recorded rather than hidden: phase (d)'s true-SHA assertion was
never observed to be the failing line (any wrong repair is caught earlier by the
loop bound), and the dangling-row rule is only observable under mutation 4b — the
inert variant is a no-op because writes are gated on `updated`.

## Follow-ups (deferred)

- **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** (low, unchanged): the
  opt-in degraded mode is logged but never surfaced in a tool response.
- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low, unchanged): make the ptrace
  tests serial structurally so the default `cargo test` cannot hang for 17 minutes.
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low, unchanged): four
  hand-rolled copies of the read-path table-error policy.
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, now mitigated): the
  operational churn is automated one command deep; the design shape (manifests
  listing shared mutable files) is unchanged.
- **Sandbox warm-up ordering** and **5+19 not-merged branches triage**: preserved;
  the latter still needs human review.
