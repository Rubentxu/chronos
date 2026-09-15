# Verify Report — m9-73-sandbox-client-store-isolation

| Campo | Valor |
|---|---|
| Cycle | `m9-73-sandbox-client-store-isolation` |
| Path | B-direct |
| Base SHA | `2df028874bb74a618e126828dd84923a67030a60` |
| Head SHA (pre-artifacts) | `d4919eec670e133f55758ba6a37301aee92f9570` |
| Diff digest | `sha256:2c6d51e79e7b5e347273284e85377ec6bec4f1ea7fe4c93f4c1baf911591db7a` |
| Verified at | `2026-09-13T15:16Z` |
| Working tree | clean at verification time; artifact writes follow |
| Verdict | **passed** |

## Summary

Verdict: **passed**. One finding closed (`FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT`), four deferred: `FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT` (high), `FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE` (medium), `FIND-M9-73-SESSION-EDGE-CASES-HEAVY-SAVE-NEVER-COMPLETES` (medium), `FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO` (low).

Falsification did more than confirm the fix: it **rejected the first version of the cycle's own test suite**, which passed with the guard removed because it asserted what the client recorded instead of what the server opened. The suite now observes the filesystem (the recorded store file must exist and be non-empty) and the rebuild of those assertions is what exposed `FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE`, a silent data-loss path one layer below the harness.

## Subject

- base_sha: `2df028874bb74a618e126828dd84923a67030a60` (`main` at cycle start)
- head_sha: `d4919eec670e133f55758ba6a37301aee92f9570` (pre-artifacts source commit)
- diff_digest: `sha256:2c6d51e79e7b5e347273284e85377ec6bec4f1ea7fe4c93f4c1baf911591db7a`
- working_tree_clean: true (at verification time)
- verified_at: `2026-09-13T15:16Z`
- cwd: `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos`
- route: B-direct (skill → apply → light-verify → release → archive)

## Closed finding

### FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT (medium, `test.shared_database_state`)

m9-72 recorded one writer of `CHRONOS_DB_PATH`. Recon found three:

| # | Writer | Effect |
|---|---|---|
| 1 | `McpTestClient::start()` → `start_path()` → `factory::start()`, spawning with the caller's environment | every sandbox client resolved `$HOME/.local/share/chronos/sessions.redb` — one 86 MB store shared by all 35 suites |
| 2 | `session_edge_cases::test_load_session_persists_across_client_instances` (`remove_var`, `set_var` ×2, `remove_var`) | mutates the environment of the whole test binary, so tests on other threads inherit it |
| 3 | `counterexample_tools::ce12` (`set_var`, `remove_var`) under the comment "so the CLI replay can find the same store" | the comment was false: `replay_bundle` passes `--db <path>` explicitly, so the mutation protected nothing |

Fix: `start_path` allocates a private store (`allocate_store_dir()`: PID + process-local `AtomicU64` + nanosecond timestamp), passes it through `start_with_env` via `db_env()`, and removes the directory in `impl Drop` after taking the process handle (so the child is reaped before its store is deleted). `start_with_db_path` stays the explicit opt-in for sharing, records `db_dir: None` so the caller's path is never deleted, and no longer builds an `std::env::vars()` base. `start()` and `start_with_db_path` now share one binary-resolution ladder (`resolve_mcp_path()`). `spawn_with_env` removes `CHRONOS_DB_PATH` from the inherited environment before applying `extra_env`, so the child's store is only ever what the caller supplied. Both global-mutation sites are gone, and CC#56 (new, python) fails the drift sweep if either pattern returns anywhere under `chronos-sandbox/`.

## Files Inventory

| File | Change |
|---|---|
| `chronos-sandbox/src/client/tools.rs` | +118/−39: `db_dir`, `db_path`, `resolve_mcp_path`, `allocate_store_dir`, `db_env`, `db_path()`, `impl Drop`; `start_path` private store; `start_with_db_path` explicit |
| `chronos-sandbox/src/client/process.rs` | +8/−4: `spawn_with_env` removes the inherited `CHRONOS_DB_PATH` |
| `chronos-sandbox/tests/client_store_isolation.rs` | new, 212 lines, 3 tests + 1 helper |
| `chronos-sandbox/tests/session_edge_cases.rs` | +5/−14: SE1 shares one store via `start_with_db_path`; no environment mutation |
| `chronos-sandbox/tests/counterexample_tools.rs` | −4: ce12's redundant `set_var`/`remove_var` |
| `scripts/smoke_test_ccs.sh` | expected CC counts 47 → 48 python (2 hunks: the grep and the comment) |
| `.sddk-knowledge/…/maintenance/vault-drift-sweep.md` | +51: CC#56, with the three-writer history and the resolution |

## Falsification evidence

Six independent observations. Every revert was applied, observed to fail, and restored; the two restored `src/` files are byte-identical to their pre-falsification copies (verified by `grep -c FALSIFICATION` = 0 and by re-running the suite green).

| # | Configuration | Observed result |
|---|---|---|
| 1 | First version of the suite (no on-disk assertion), `start_path` guard removed | 3/3 **PASSED** — vacuous. Asserting `a.db_path() != b.db_path()` tests the allocator, not the wiring, and "B cannot see A's session" was satisfied by `open_default_store` falling back to an in-memory store. This is the observation that forced the rewrite. |
| 2 | Rewritten suite, `start_path` guard removed | **FAILED 2/3**: `test_each_client_gets_its_own_store_path` — "server A never opened the store the client recorded … the server fell back to an in-memory store instead"; `test_saved_session_is_invisible_to_another_client` — "client A's server never opened … so 'invisible to B' proves nothing". `test_explicit_db_path_still_shares_one_store` still passed, correctly: it does not use `start_path`. |
| 3 | Ambient decoy (`CHRONOS_DB_PATH` exported to a scratch path) with the fix in place | 3/3 passed, the check printed "ambient `CHRONOS_DB_PATH` … was ignored", and the decoy file was **never created** |
| 4 | Ambient decoy with **both** guards removed (`start_path` guard and the `env_remove`) | **FAILED**, first assertion to fire: "a sandbox client opened the ambient `CHRONOS_DB_PATH` /home/rubentxu/.jcode/scratch/decoy-store/decoy.redb instead of a private store", and `decoy.redb` existed on disk afterwards. Test 2 additionally failed with the in-memory-fallback message. |
| 5 | CC#56 against the pre-fix tree (`git show HEAD:` of both touched test files) | 6 hits — `counterexample_tools.rs:599,688`, `session_edge_cases.rs:41,44,94,176` — i.e. exactly the removed writers. Against the current tree: empty output. |
| 6 | Interleaved A/B against `main`, alternating builds, `session_persistence` alone, 3 rounds | fix: 50 s / 50 s / 53 s, **3/3 pass** · `main`: 56 s / 53 s / **60 s FAILED** — `session_persistence.rs:128`, `save_session failed: TimeoutError("method=tools/call", 30s)`, the signature m9-72 recorded. |
| 7 | Interleaved A/B against `main`, alternating builds, `session_edge_cases`, 3 rounds | cycle branch 3/2/2 failures, `main` 2/2/2 — the same two tests fail on both sides, so the cycle's own file edit did not cause them (the section above has the detail). |
| 8 | SE1 run with `--ignored --exact`, twice | 1 passed / 0 failed both times (18.3 s), which is why its `#[ignore]` was removed. |

Observation 6 tests the flake claim rather than asserting it: the failure landed on the unmodified build in the same alternating sequence, and the failing run is the slowest `main` run (60 s vs 53–56 s), consistent with elapsed time behind the fixed 30 s client timeout. The fix side was green in all three rounds and consistently ~5 s faster, which is what removing an 86 MB shared write from the path looks like.

## Gates

| Gate | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check` | pass |
| T0 | `cargo clippy --workspace --all-targets -- -D warnings` | pass, 0 warnings |
| T2 | `cargo test -p chronos-sandbox --lib` | 3 passed |
| T4-smoke | `client_store_isolation` | 3 passed (36.15 s) |
| T4-smoke | `session_persistence` | 4 passed (56.56 s) |
| T4-smoke | `session_edge_cases` | **4 passed, 2 failed** (109.44 s) — both failures pre-existing, attributed below; SE1 now runs (it was `#[ignore]`d) and passes |
| T4-smoke | `counterexample_tools` | 12 passed (66.79 s) |
| T4-smoke | `e2e_connectivity` | 1 passed (5.64 s) |
| Drift | `./scripts/check_vault_drift.sh` | PASS (48 python CCs, 7 bash CCs) |
| CC smoke | `./scripts/smoke_test_ccs.sh` | 5 tests / 0 failures |

T3 was not re-run for this cycle: no crate outside `chronos-sandbox` changed (`git diff --name-only`), and the sandbox crate is excluded from T3 by construction. `chronos-sandbox` has no `--lib` tests affected by the change beyond the 3 that pass; its signal lives in the T4 bucket, which was exercised with the documented `--test-threads=1`.

## Pre-existing failure found while running the required subset (not caused by this cycle)

`session_edge_cases` is red in this environment, and the cycle modified that file, so the failure had to be attributed rather than waved through. Two tests fail, always the same two, always with the same signature: `test_compare_sessions_crash_vs_normal` (`save_session (crash) failed: TimeoutError("method=tools/call", 30s)`) and `test_performance_regression_audit_different_workloads`.

| Observation | Result |
|---|---|
| Interleaved A/B, alternating builds, whole suite, 3 rounds each, `--test-threads=1` | cycle branch: 3, 2, 2 failures · `main`: 2, 2, 2 failures — the same two tests every round on both sides (line numbers differ by exactly the eight lines this cycle removed from the top of the file) |
| Timeout raised to 180 s (temporary edit to `rpc.rs`, reverted) | still FAILED, after 186 s, with `TimeoutError("method=tools/call", 180s)` — so the save genuinely does not complete; this is not a calibration issue |
| Failure cause | the test prints `Crash session stopped: 34905 events`; `ContentStore::put` opens one redb write transaction (with redb's default immediate-durability commit) **per event**, so the save is linear at roughly 4.8 ms per event (34,905 × 4.8 ms ≈ 168 s, against >186 s observed). Recorded as `FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT` (high) with a proposed fix |
| Bare `#[ignore]` audit | `session_edge_cases` had one ignored test, SE1, ignored for parallel interference that was caused by the process-global `CHRONOS_DB_PATH` mutation this cycle removed. With the store passed explicitly it passes 2/2 (18.3 s each), so it is now un-ignored and contributes a passing test. |

Both rows are recorded in `AGENTS.md` §6.5 so future cycles do not chase them as regressions. The remaining gate evidence for the cycle is unaffected: the failure is present on `main`, on the same tests, with the same signature.

## Residual risk

- **The deferred fallback is live.** `open_default_store` still degrades to an in-memory store, so any *other* path that points a server at a locked store silently loses data. This cycle removes the sandbox's ability to reach that state and makes the isolation suite fail if it does (`FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE`).
- **`Drop` cleanup is best-effort by design.** The store directory is removed on drop; if a test process is killed mid-run the temp directory survives. Directories are unique per client, so a survivor cannot collide with a later run, and a leaked `chronos-mcp` is reaped by the same `Drop` through the process handle.
- **`test_explicit_db_path_still_shares_one_store` deliberately exercises the shared path**, i.e. the configuration whose timeout this cycle removed from the default. It passed in every observed run (34–44 s for the suite), and the shared path there is a small, dedicated file rather than the 86 MB developer store, so it is not the same timing regime.

## Cross-checks

- CC#1..CC#56: pass, no drift. Python CC count 47 → 48 (`scripts/smoke_test_ccs.sh` updated in the artifacts commit); bash count unchanged at 7.
- CC#48 + CC#54: the smoke test asserts the PASS message reports the expected counts; updated to 48 python, and the smoke run reports `48 python CCs all clean, 7 bash CCs all clean`.
- CC#4 (archive-manifest index SHAs): this cycle edits `cycles/index.md`, `terms/index.md`, `scripts/smoke_test_ccs.sh` and `maintenance/vault-drift-sweep.md`, so the content-addressed rows in **every** prior manifest that lists them go stale. Regenerated to a fixpoint; the second run reported 0 rows updated. Manifests that do not list those paths were left untouched: a whole-tree regeneration rewrites the self-referential `archive-manifest (this file)` row of nine older manifests (m9-01, m9-03..m9-10), which is unrelated churn, and was reverted. `m9-67` and `m9-68` keep only the rows this cycle's edits actually changed. Recorded as `FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO`.
- CC#24 / CC#31: this report has its `## Cross-checks` section and the archive manifest carries `Base SHA` and `Head SHA`.
- CC#12: `main_sha == head_sha == remote_tag_peel` verified after the release.
- CC#56 (new): the check itself was validated against the pre-fix tree (6 hits) and the current tree (empty).

## Deferred findings

- **FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE** (medium, `code.silent_degradation`): `chronos-mcp::open_default_store` falls back to `SessionStore::in_memory()` when the configured store cannot be opened (locked, corrupt, missing parent), logging only a `tracing::warn`. A server pointed at a locked store therefore starts successfully, reports success on `session_save`, and answers `session_list` with an empty list: silent data loss with a green health check. Found by falsification of this cycle's own test. Not fixed here: the fallback is a deliberate resilience choice, the remedy is a policy decision that belongs to `chronos-mcp` (fail closed on a store that exists but cannot be opened; keep the fallback for a missing parent; or surface degraded mode in a tool response).
- **FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO** (low, `process.undocumented_tooling`): the CC#4 regeneration is a scratch script re-derived every cycle, with two known footguns (self-referential rows; the naive recompute-everything rule that produced m9-02's defect). Remedy: land `scripts/regen_manifest_index_shas.py` with the skip-self-row rule and document it beside CC#4.
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low): unchanged.
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low): unchanged; the affected set grew again this cycle.
- **Sandbox warm-up ordering** and **5+19 not-merged branches**: unchanged.

## Findings

None — clean state. (m10-legacy-migration)
