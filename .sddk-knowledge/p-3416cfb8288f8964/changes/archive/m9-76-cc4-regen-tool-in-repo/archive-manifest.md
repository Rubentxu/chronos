# Archive Manifest — m9-76-cc4-regen-tool-in-repo

## Summary

m9-76 closes `FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO`, the low process finding
confirmed for the fifth time in m9-75 and re-confirmed inside this cycle. CC#4
(artifact-index SHA-256 consistency in `archive-manifest.md`) had a gate in
`scripts/check_vault_drift.sh` but no repair tool in the repository: the repair was
a throwaway python script re-derived every cycle in the agent scratch directory,
whose naive whole-tree form churned the self-referential rows of the pre-m9-11
manifests, which then had to be reverted by hand.

The consequence that matters is epistemic. Because the tool that was supposed to
satisfy the gate lived outside the repository, nothing exercised the gate's own
logic — which is how CC#4's broken awk (m9-66) accumulated silent drift for many
cycles with nothing to contradict it. The ritual was also invisible to reviewers:
a correctly regenerated table and a hand-edited one looked the same.

The repair half now lives in `scripts/regen_manifest_index_shas.py`, deliberately
mirroring CC#4's two edge rules because agreement with the gate is the entire
justification for the tool. The self-referential row is preserved (a file cannot
contain its own hash, and CC#4 skips it by path comparison). A row whose target
file no longer exists is left byte-for-byte untouched and reported as `missing`
(CC#4 guards every comparison with `[ -f "$path" ]`, so a dangling row is invisible
to the gate, and a repair tool that blanked it would manufacture drift).
`--check` writes nothing and exits `1` naming each stale row — the machine
verification that the ritual ran; `--dry-run` reports and exits `0`; the default
rewrites to a bounded fixpoint, because rewriting one manifest changes the bytes of
any manifest another one lists. 13 zero-dependency unit tests plus a sixth smoke
check pin tool and gate together in both directions (detect and repair), and
`setup_work_copy()` now overlays the working-tree vault tooling so the smoke suite
tests the tree about to merge rather than HEAD alone.

The tool was used on its own cycle: editing the smoke suite, the vault drift doc
and `AGENTS.md` staled rows in six historical manifests (`m9-67`, `m9-68`,
`m9-72`, `m9-73`, `m9-74`, `m9-75`), plus two more after the vault-doc edit. One
command and a `--check` confirmation replaced what used to be hand work with a
manual revert of self rows. One commit on
`feat/m9-76-cc4-regen-tool-in-repo`. Tag `v0.7.78`.

## Cycle

| Campo | Valor |
|---|---|
| Cycle ID | `m9-76-cc4-regen-tool-in-repo` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `e59c44e64f40b475a3541c6de3e9176ebf5d0cdf` |
| Head SHA | `613b326d24191fd0ceaba5a500ef5b59857bd68d` |
| Branch | `feat/m9-76-cc4-regen-tool-in-repo` |
| Tag | `v0.7.78` |
| Route | B-direct |

## Deliverables

| Artifact | Kind |
|---|---|
| `scripts/regen_manifest_index_shas.py` | the CC#4 repair tool (rewrite / `--check` / `--dry-run` / `--verbose`) |
| `scripts/tests/test_regen_manifest_index_shas.py` | 13 unit tests, no third-party dependency |
| `scripts/smoke_test_ccs.sh` | sixth check pinning tool and gate together; working-tree overlay in `setup_work_copy()` |
| `AGENTS.md` | §5 generated-rows rule; §7 vault commands |
| `maintenance/vault-drift-sweep.md` | repair-tool paragraph under CC#4; resolution line now names the tool |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO]`
- **`verify-findings.json`**: 1 finding closed (low `process.undocumented_tooling`), 4 deferred (low `code.silent_degradation`, low `test.environmental_failure`, low `code.duplicated_policy`, low `process.growing_regeneration_set`), verdict `passed`
- **`verify-report.md`**: Path, Summary, Closed finding, Files Inventory, Falsification evidence, Gates, Residual risk, Cross-checks
- **`merge-receipt.md`**: `Base SHA | e59c44e64f40b475a3541c6de3e9176ebf5d0cdf`, `Head SHA | 613b326d24191fd0ceaba5a500ef5b59857bd68d`
- **`release-receipt.md`**: `Remote tag | v0.7.78`, `Peel match | true`
- **`scripts/regen_manifest_index_shas.py`**: `--check` exit 0 over 76 manifests; `--dry-run` exit 0; `--check --dry-run` exit 2
- **`scripts/tests/test_regen_manifest_index_shas.py`**: `Ran 13 tests … OK`
- **`scripts/smoke_test_ccs.sh`**: 6/6 checks pass

## Falsification evidence

Six mutations, each reverted and `cmp`-verified byte-identical against
`/home/rubentxu/.jcode/scratch/regen.m9-76-good.py` before the next, and each
observed to fail for the reason the guard exists:

| # | Reverted | Observed result |
|---|---|---|
| 1 | stale-row print quiet-gated again | naming unit test FAILED; smoke FAILED with `regen: --check did not name the drifted row` (found organically on the first smoke run) |
| 2 | self-row comparison removed | two unit tests FAILED; the real tree reported a manifest's own path as stale |
| 3 | `--check` failure branch disabled | two unit tests FAILED; smoke FAILED |
| 4 / 4b | dangling-row guard removed / row blanked and counted | `FileNotFoundError` / preservation test FAILED with `1 != 0` |
| 5 | fixpoint loop reduced to one pass | interlinked-manifest test FAILED |
| 6 | write path emits a wrong hash | smoke FAILED with `regen: rewrite exit=2 (expected 0)` |

## Tangential modifications

- Six historical archive manifests (`m9-67`, `m9-68`, `m9-72`, `m9-73`, `m9-74`,
  `m9-75`) had 1–3 Artifact-index rows regenerated because
  `scripts/smoke_test_ccs.sh`, `maintenance/vault-drift-sweep.md` and `AGENTS.md`
  changed in this cycle. This is CC#4's normal behaviour (the rows are hashes of
  shared files); the regeneration was performed by the new tool, and the self rows
  were left at their committed values.
- `terms/index.md`: the `FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO` row was removed
  from the active deferred list (it had been appended to the m9-72 section) and
  added to the resolved list.
- The archive-manifest count grows from 75 to 76 with this cycle; the smoke check
  asserts exit codes and row names, never counts, so it scales automatically.

## Cross-checks

- CC#1..CC#56: pass (48 python + 7 bash, counts unchanged — CC#4 already covers
  SHA consistency, so no new check was added and the smoke suite still asserts the
  same counts while running 6 checks instead of 5).
- CC#4: `--check` clean over 76 manifests after the cycle's own regeneration, which
  was performed by the new tool rather than by hand; the rows of `m9-67`, `m9-68`,
  `m9-72`, `m9-73`, `m9-74` and `m9-75` were regenerated for the three edited files
  and the self rows were left at their committed values.
- CC#24 / CC#31: this manifest carries the `Base SHA` and `Head SHA` rows.
- CC#36 / CC#55 / CC#39: `verify-report.md` carries `Path` at the head and a
  `## Files Inventory` section; `verify-findings.json` carries `lens_summary` after
  `subject`; this manifest and `release-report.md` carry `## Cross-checks`.
- CC#33: `## Subject` is present in `change-entry.md`, `verify-report.md` and
  `verify-findings.json`.
- CC#12: `main_sha == head_sha == remote_tag_peel` for `v0.7.78` (filled in the
  post-release commit, because the tag points at the artifacts commit).
- T0: `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets
  -- -D warnings` clean.
- T1: 921 passed (workspace lib minus `chronos-sandbox`/`chronos-native`/
  `chronos-e2e`) plus 101 passed for `chronos-native --lib --test-threads=1`.
- T4-canary: `e2e_connectivity` 1/1 and `store_open_failure` 2/2 with
  `CHRONOS_MCP_PATH` exported. No Rust file changed in this cycle, so the canary is
  a build-and-serve sanity check, not a regression gate.
- Falsification: six mutations, each restored byte-identically and re-run green;
  the smoke check was observed to fail for the right reason in three of them.
- No `docs/propuestas/` file touched; no test deleted or weakened.

## Follow-ups (deferred)

- **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** (low): with the opt-in
  set, the degraded mode is logged but never surfaced in a tool response.
- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low): make the ptrace tests
  serial structurally so the default `cargo test` cannot hang for 17 minutes.
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low): four hand-rolled
  copies of the read-path table-error policy that `chronos-store::table_error`
  already names.
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, mitigated): the
  operational churn is now one command plus `--check`; the design shape (manifests
  listing shared mutable files) is unchanged.
- **Sandbox warm-up ordering** and **5+19 not-merged branches triage**: preserved;
  the latter still needs human review.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-76-cc4-regen-tool-in-repo/archive-manifest.md` | `0000000000000000000000000000000000000000000000000000000000000000` |
| source (regen tool) | `scripts/regen_manifest_index_shas.py` | `2e9031fed75cfd2faa583e4a13c0370f4655a8fae7e441dd13a48d092cba84a3` |
| test (regen unit tests) | `scripts/tests/test_regen_manifest_index_shas.py` | `57168dbb1942ac0cd58f5edd42edc2dd407abe1a6cd23b2dcb355f654e8a4672` |
| test (cc smoke suite) | `scripts/smoke_test_ccs.sh` | `86def49d7e23b4e521687ddcb53a384aac18ec3f91f0e2845396f90d7ac3e7d3` |
| docs (agents manual) | `AGENTS.md` | `83d09aa421c0c0c06a8dc28d4a5ba27dbb66772f7a1310103f3c133969fe8924` |
| docs (vault drift sweep) | `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` | `0c57c1dacf0f30bd9ac4adf201dcdf9d60ee43ae858dd50276189d140eadb302` |
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-76-cc4-regen-tool-in-repo/apply-checkpoint.json` | `232613564d78f9c0839f732c4b1647cf12f4582602c4073580cbf0cc6e20df0c` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-76-cc4-regen-tool-in-repo/verify-report.md` | `c10f7d6c66d92dd01420a5dc64846ae717dd874c7e2334b138a9578d987adfcf` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-76-cc4-regen-tool-in-repo/verify-findings.json` | `e3f3a9b1dd726d8794a6aa7eb11aa0bc91c86d06e1a0b623ef9ee6b72943b28a` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-76-cc4-regen-tool-in-repo/release-report.md` | `4d269ba1b0bff938b0ac7b11706a5d871b1ee553c60ac5747399eaa438f97c93` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-76-cc4-regen-tool-in-repo/release-receipt.md` | `a0f2475bfc990115cbabf9109f26c06ffcd3b05ade3ef55ee9c53e9c24816182` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-76-cc4-regen-tool-in-repo/merge-receipt.md` | `0ccc5642788a57be9e0457ca7dac763211872229f76f679311b9b483a7488051` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-76-cc4-regen-tool-in-repo/change-entry.md` | `c40623efda64da667293be5d19262f6d627d7c8a54e2e8b5d527bd82b2c778f1` |
| vault index (cycles) | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | `7d10834aa7c01e5f5e4442718c4fe7a78246fba3a5815e6eaffa2f8d023cf895` |
| vault index (terms) | `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | `876d4b1072ea1bfe6ee23744a5f43cd23ef94686669415df893718499cdb5170` |
