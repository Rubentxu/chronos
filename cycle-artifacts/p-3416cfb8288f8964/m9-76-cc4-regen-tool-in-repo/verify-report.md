# Verify Report — m9-76-cc4-regen-tool-in-repo

| Campo | Valor |
|---|---|
| Cycle | `m9-76-cc4-regen-tool-in-repo` |
| Path | B-direct |
| Base SHA | `e59c44e64f40b475a3541c6de3e9176ebf5d0cdf` |
| Head SHA (pre-artifacts) | `fd2579f9b1ded8f71687f696d4b5b1f2234be892` |
| Diff digest | `sha256:bc726c11f39e072964a718c225b4b75f190841accda2cc6202394ca46581355a` |
| Branch | `feat/m9-76-cc4-regen-tool-in-repo` |
| Verified at | `2026-09-13T17:56Z` |
| Working tree | clean at verification time; artifact writes follow |
| Verdict | **passed** |

## Summary

Closes `FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO`, the low finding confirmed for
the fifth time in m9-75 and re-confirmed inside this cycle. CC#4 (artifact-index
SHA-256 consistency in `archive-manifest.md`) had a gate in
`scripts/check_vault_drift.sh` but no repair tool in the repository: the repair
was a throwaway python script re-derived every cycle under the agent scratch
directory. Nothing therefore exercised the gate's own logic, which is how the
broken awk of m9-66 survived many cycles, and the ritual left no trace a reviewer
could re-run.

The cycle lands the repair half of the gate:

- `scripts/regen_manifest_index_shas.py` rewrites every stale Artifact-index row
  to a fixpoint. It mirrors CC#4 exactly where it must: the self-referential row
  is preserved (a file cannot contain its own hash, and CC#4 skips it by path
  comparison), and a row whose target no longer exists is left byte-for-byte
  untouched and reported (`missing`), because CC#4 guards every comparison with
  `[ -f "$path" ]` and would never have flagged it.
- `--check` writes nothing and exits `1` naming each stale row: this is the
  machine verification that the ritual was performed.
- `--dry-run` reports and always exits `0`; `--verbose` also lists manifests that
  needed no change. `--check` and `--dry-run` are mutually exclusive (exit `2`),
  as is a missing manifest (exit `2`).
- `scripts/tests/test_regen_manifest_index_shas.py` adds **13** zero-dependency
  `unittest` cases: row parsing, self-row preservation, dangling-row
  preservation, `--check` / `--dry-run` exit codes and exclusivity, the fixpoint
  across two manifests that list each other, manifest discovery, explicit
  arguments, and the naming of stale rows.
- `scripts/smoke_test_ccs.sh` gains a **sixth** check, `test_regen_script()`,
  which pins tool and gate together in four phases: the unit tests pass;
  `--check` is clean on a clean clone; after injecting the same stale SHA that
  `test_cc4` injects, both `--check` and CC#4 flag it and `--check` names the
  row; after running the tool, the row carries the true SHA-256 and both
  `--check` and the gate are green.

The tool earned its keep inside its own cycle. Editing
`scripts/smoke_test_ccs.sh`, `maintenance/vault-drift-sweep.md` and `AGENTS.md`
staled the rows of six historical manifests (`m9-67`, `m9-68`, `m9-72`, `m9-73`,
`m9-74`, `m9-75`), and the vault-doc edit staled two more; the pre-m9-76 ritual
would have hand-managed all of them. One command plus `--check` did it, and
`CC#4` stayed green without any manual revert of self rows.

## Subject

- base_sha: `e59c44e64f40b475a3541c6de3e9176ebf5d0cdf`
- head_sha: `fd2579f9b1ded8f71687f696d4b5b1f2234be892` (pre-artifacts code commit)
- diff_digest: `sha256:bc726c11f39e072964a718c225b4b75f190841accda2cc6202394ca46581355a` (`git diff <base>..<head> | sha256sum`)
- cycle: m9-76
- branch: `feat/m9-76-cc4-regen-tool-in-repo`
- route: B-direct
- verified_at: `2026-09-13T17:56Z`
- working_tree_clean: true at verification time
- verdict: `passed`

## Closed finding

`FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO` (low, `process.undocumented_tooling`,
first raised in m9-73, deferred in m9-72/73/74/75).

**Gap.** `scripts/check_vault_drift.sh` implements CC#4, comparing every
artifact-index SHA-256 row against the current file content and skipping the
self-referential row. Its repair half lived only in
`/home/rubentxu/.jcode/scratch/`: a script re-derived each cycle, whose naive
whole-tree form churned the self-referential rows of the manifests written before
m9-11 (which had to be reverted by hand afterwards). Consequences: the gate's
logic was never exercised by the tool meant to satisfy it (m9-66's broken awk went
unnoticed for many cycles), and the ritual was invisible to the repository
reviewer.

**Fix.** `scripts/regen_manifest_index_shas.py` (rewrite to fixpoint / `--check` /
`--dry-run` / `--verbose`, with the two CC#4-mirroring preservation rules),
`scripts/tests/test_regen_manifest_index_shas.py` (13 tests), the sixth smoke
check, `maintenance/vault-drift-sweep.md` (repair-tool paragraph under CC#4; the
resolution line now says to run the tool instead of hand-editing), `AGENTS.md`
§5 bullet plus §7 commands.

## Files Inventory

| File | Change |
|---|---|
| `scripts/regen_manifest_index_shas.py` | new, 257 lines: `ROW` regex, `sha256`, `default_manifests`, `rows_of`, `rewrite`, `rel`, `main` with `--check` / `--dry-run` / `--verbose`, bounded fixpoint (`MAX_PASSES = 5`) |
| `scripts/tests/test_regen_manifest_index_shas.py` | new, 179 lines, 13 `unittest` cases, no third-party dependency |
| `scripts/smoke_test_ccs.sh` | +140/−2: `test_regen_script()` sixth check (4 phases) and `setup_work_copy()` now overlays the working-tree vault tooling |
| `AGENTS.md` | +12: §5 "Archive-manifest SHA rows are generated, not hand-edited"; §7 vault commands |
| `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` | +26/−2: "Repair tool (added by m9-76)" under CC#4 and the run-the-tool resolution line |
| `.sddk-knowledge/.../archive/m9-67-cc-smoke-test/archive-manifest.md` | regenerated row (`scripts/smoke_test_ccs.sh`) |
| `.sddk-knowledge/.../archive/m9-68-verify-report-files-inventory-backfill/archive-manifest.md` | regenerated rows (`scripts/smoke_test_ccs.sh`, `vault-drift-sweep.md`) |
| `.sddk-knowledge/.../archive/m9-72-read-path-table-error-classification/archive-manifest.md` | regenerated row (`AGENTS.md`) |
| `.sddk-knowledge/.../archive/m9-73-sandbox-client-store-isolation/archive-manifest.md` | regenerated rows (`scripts/smoke_test_ccs.sh`, `vault-drift-sweep.md`, `AGENTS.md`) |
| `.sddk-knowledge/.../archive/m9-74-cas-put-many-batching/archive-manifest.md` | regenerated row (`AGENTS.md`) |
| `.sddk-knowledge/.../archive/m9-75-fail-closed-store-open/archive-manifest.md` | regenerated row (`AGENTS.md`) |
| `cycle-artifacts/p-3416cfb8288f8964/m9-76-cc4-regen-tool-in-repo/*` | this report, `verify-findings.json`, `apply-checkpoint.json`, `release-report.md`, `release-receipt.md`, `merge-receipt.md` |
| `.sddk-knowledge/.../changes/m9-76-cc4-regen-tool-in-repo/change-entry.md` | new |
| `.sddk-knowledge/.../changes/archive/m9-76-cc4-regen-tool-in-repo/archive-manifest.md` | new |
| `.sddk-knowledge/.../cycles/index.md` | m9-76 row, `Total cycles` 75 → 76 |
| `.sddk-knowledge/.../terms/index.md` | `FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO` terminated as closed; `Last archive` |

## Falsification evidence

Every guard was reverted, the failure observed, and the file restored
byte-identically (`cmp` against
`/home/rubentxu/.jcode/scratch/regen.m9-76-good.py`) before the next mutation.
Row 1 was found **organically** (the first smoke run after the feature was
written), which is why the naming unit test exists.

| # | Reverted | Observed result |
|---|---|---|
| 1 | stale-row print quiet-gated again (`if write:` instead of unconditional) | `test_stale_row_is_named_in_check_output` FAILED — `'src.txt' not found in '…: 1 row(s) stale\nregen-manifest-index-shas: DRIFT: stale SHA row(s) in the table above'`; smoke suite FAILED with `regen: --check did not name the drifted row` |
| 2 | self-row comparison replaced by `if False:` | `test_clean_manifest_exits_zero` and `test_self_row_is_preserved` FAILED; on the real tree `--check` reported a manifest's own path as stale (expected `903fe3b5…`) — the unfixable churn the finding describes |
| 3 | `--check` failure branch disabled (`if False:`) | `test_stale_row_is_reported_and_exits_one` and `test_stale_row_is_named_in_check_output` FAILED; smoke suite FAILED (`regen: unit tests failed`) |
| 4 | dangling-row guard removed | `FileNotFoundError` on the missing target (an IO error, not a silent pass) |
| 4b | dangling row blanked and counted as changed | `test_missing_target_is_preserved_and_invisible_to_check` FAILED — `AssertionError: 1 != 0` |
| 5 | fixpoint loop reduced to a single pass | `test_fixpoint_across_interlinked_manifests` FAILED |
| 6 | write path emits a wrong hash | smoke suite FAILED with `regen: rewrite exit=2 (expected 0)` — a wrong hash never converges, so the bounded loop reports rather than spinning |

Restored state re-verified: `13/13` unit tests `OK`, `--check` clean across **75**
manifests, vault gate `PASS (48 python CCs all clean, 7 bash CCs all clean)`,
smoke suite `6/6`, and both mutated files `cmp`-identical to the backups.

Two observations worth recording as limits of the evidence rather than as
successes:

- Phase (d)'s "the rewritten row carries the true SHA-256" assertion is
  belt-and-suspenders: every wrong-repair mutation that stays wrong is caught
  earlier by the fixpoint bound (row 6), so the assertion was not observed to be
  the failing line. The reachable failure for a wrong repair is the loop bound.
- Mutation 4b is the only way to make the dangling-row rule observable: the
  original mutation (dropping the branch) crashes instead, and the inert variant
  (appending a blanked row without counting it as changed) changes nothing
  because writes are gated on `updated`. The rule is still worth having — CC#4
  ignores dangling rows, so the tool must too — but its test is only meaningful
  under 4b.

## Gates

| Gate | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check` | clean |
| T0 | `cargo clippy --workspace --all-targets -- -D warnings` | clean (cached, rc=0) |
| T1 | `cargo test --workspace --lib --no-fail-fast --exclude chronos-sandbox --exclude chronos-native --exclude chronos-e2e` | **921 passed / 0 failed** |
| T1 | `cargo test -p chronos-native --lib --no-fail-fast -- --test-threads=1` | **101 passed / 0 failed** (13.02 s) |
| unit | `python3 scripts/tests/test_regen_manifest_index_shas.py` | **13/13 OK** |
| CC#4 | `python3 scripts/regen_manifest_index_shas.py --check` | clean, **75** manifests |
| vault | `bash scripts/check_vault_drift.sh` | **PASS** (48 python + 7 bash CCs) |
| smoke | `./scripts/smoke_test_ccs.sh` | **6/6**, `CC detection chain is healthy` |
| T4-canary | `cargo test -p chronos-sandbox --test e2e_connectivity --test store_open_failure -- --test-threads=1` with `CHRONOS_MCP_PATH` exported | `e2e_connectivity` 1/1 (5.66 s), `store_open_failure` 2/2 (5.17 s) |

No Rust source file changed in this cycle, so the tier table's B-direct minimum
(T0 + T1) is what the crate gates exercise; the canary run confirms the tree still
builds, serves and fails closed after the vault edits. T3/T5 were not run and the
sandbox warm-up flake was not re-characterised, for the same reason: there is no
probe/MCP/transport change in the diff.

## Residual risk

- The tool is only as correct as its agreement with CC#4's awk. That agreement is
  now asserted by the smoke check (which injects the same drift `test_cc4`
  injects) instead of assumed, but if CC#4's row-shape rules ever change, the tool
  must change with them; the smoke check is what will complain first.
- `--check` is not wired into CI or the pre-merge commands, so a cycle can still
  forget to run it. The vault-drift workflow catches the underlying drift on
  `push` to `main` for vault paths, which is the safety net; adding the check to
  CI would need a CI file change with its own blast radius.
- The six regenerated historical manifests re-write committed rows from previous
  cycles. This is inherent to CC#4 (the rows are hashes of files that later
  cycles modify); the diffs are one to three rows each and every one is a
  recomputation of the same file, not an edit of a claim.
- The finding's underlying shape (`FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION`)
  is mitigated, not removed: manifests still list shared mutable files, so each
  cycle still rewrites rows across the archive. The automation makes it
  mechanical, not free.

## Cross-checks

- CC#1..CC#56: pass, counts unchanged at **48 python + 7 bash**. No new CC was
  added: CC#4 already covers SHA consistency, and a duplicate check would add
  noise rather than signal. `scripts/smoke_test_ccs.sh` therefore asserts the same
  message (`48 python CCs all clean, 7 bash CCs all clean`).
- CC#4: `--check` clean over 75 manifests after the regeneration; the cycle's own
  regeneration was performed by the new tool, not by hand.
- CC#36 / CC#55 / CC#39: this report carries `Path` at the head and a
  `## Files Inventory` section; `verify-findings.json` carries `lens_summary`
  after `subject`; this report and `archive-manifest.md` carry `## Cross-checks`.
- CC#24 / CC#31: `change-entry.md` and `archive-manifest.md` carry the `Base SHA`
  and `Head SHA` rows.
- CC#12: `main_sha == head_sha == remote_tag_peel` for tag `v0.7.78` (filled in
  the post-release commit, since the tag points at the artifacts commit).
- CC#33: `## Subject` present in `change-entry.md`, this report and
  `verify-findings.json`.
- The smoke suite's own wiring is falsified too: with `--check` broken it reports
  `regen: unit tests failed` / `regen: --check did not detect the injected
  drift`, so the sixth check is not a check that always passes.

## Follow-ups (deferred)

- **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** (low, unchanged): the
  opt-in degraded mode is logged but never surfaced in a tool response.
- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low, unchanged): make the ptrace
  tests serial structurally so the default `cargo test` cannot hang for 17 minutes.
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low, unchanged):
  four hand-rolled copies of the read-path table-error policy.
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, now mitigated):
  the operational churn is automated; the design shape is not changed.
- **Sandbox warm-up ordering** and **5+19 not-merged branches triage**: preserved,
  not re-characterised (human review needed for the latter).

## Findings

None — clean state. (m10-legacy-migration)
