# Archive Manifest — m9-73-sandbox-client-store-isolation

## Summary

m9-73 closes `FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT`, and recon found the deferral understated it by a factor of three: `CHRONOS_DB_PATH` had three independent writers in the sandbox harness, not one. `McpTestClient::start()` spawned the server with the caller's environment, so every client in all 35 suites resolved the developer's 86 MB `$HOME` store; `session_edge_cases` published its own path through `std::env::set_var`, which mutates the environment of the whole test binary and leaks into tests on other threads; and `ce12` did the same under a comment its own code contradicts (`replay_bundle` passes `--db <path>` explicitly). Every client now allocates a private store, `start_with_db_path` is the explicit opt-in for sharing, the spawn helper no longer inherits an ambient path at all, and CC#56 fails the drift sweep if process-global environment mutation returns to `chronos-sandbox`. Falsification rejected the first version of the cycle's own test suite — it asserted what the client recorded rather than what the server opened, and `open_default_store`'s silent in-memory fallback made "B cannot see A's session" true for the wrong reason — which is how the fallback became `FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE`, and how characterising the surviving `session_edge_cases` failures became `FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT` (one fsync per event on the save path; a 35k-event session takes ~3 minutes). Two commits on `feat/m9-73-sandbox-client-store-isolation`. Tag `v0.7.75`.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-73-sandbox-client-store-isolation |
| Base SHA | `2df028874bb74a618e126828dd84923a67030a60` |
| Head SHA | `456247521285fc4899ed134e370340e689ca38ca` |
| Path | B-direct |
| Date | `2026-09-13T15:20Z` |
| Branch | `feat/m9-73-sandbox-client-store-isolation` |
| Tag | `v0.7.75` |
| Tag peel SHA | `456247521285fc4899ed134e370340e689ca38ca` |
| Peel match | `456247521285fc4899ed134e370340e689ca38ca` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT]`
- **`verify-findings.json`**: 1 finding closed (medium `test.shared_database_state`), 4 deferred (high `perf.transaction_per_item`, medium `code.silent_degradation`, medium `test.environmental_failure`, low `process.undocumented_tooling`), verdict `passed`
- **`verify-report.md`**: Summary, Subject, Closed finding, Files inventory, Falsification evidence, Pre-existing failure, Gates, Residual risk, Cross-checks
- **`merge-receipt.md`**: `Base SHA | 2df0288…`, `Head SHA | 456247521285fc4899ed134e370340e689ca38ca`
- **`release-receipt.md`**: `Remote tag | v0.7.75`, `Peel match | true`

## Falsification evidence

Eight observations, each revert applied, observed, and restored:

| # | Configuration | Observed result |
|---|---|---|
| 1 | First draft of the suite, `start_path` guard removed | 3/3 **passed** — vacuous: it compared the paths the clients *recorded* (the allocator's bookkeeping) and "B cannot see A's session" was satisfied by the in-memory fallback |
| 2 | Rewritten suite, `start_path` guard removed | **FAILED 2/3** — "server A never opened the store the client recorded … fell back to an in-memory store"; the explicit-path test still passed, correctly |
| 3 | `CHRONOS_DB_PATH` exported to a scratch decoy, fix in place | 3/3 passed, decoy file never created |
| 4 | Decoy + both guards removed (`start_path` and the `env_remove`) | **FAILED** on the ambient check; `decoy.redb` existed on disk afterwards |
| 5 | CC#56 against the pre-fix tree | 6 hits: `counterexample_tools.rs:599,688`, `session_edge_cases.rs:41,44,94,176`; empty against the current tree |
| 6 | Interleaved A/B, `session_persistence`, alternating builds, 3 rounds | cycle branch 50/50/53 s **3/3 pass** · `main` 56/53/**60 s FAILED** at `session_persistence.rs:128`, `TimeoutError("method=tools/call", 30s)` |
| 7 | Interleaved A/B, `session_edge_cases`, alternating builds, 3 rounds | cycle branch 3/2/2 failures · `main` 2/2/2 — the same two tests on both sides |
| 8 | SE1 with `--ignored --exact`, twice | 1 passed / 0 failed both times (18.3 s) |

Observations 7 and 8 are the ones that kept the cycle honest: the suite this cycle edits is red, but it is red on `main` too, and the single test whose reason for being ignored was removed by this cycle passes when run.

## Pre-existing failure and its mechanism

`session_edge_cases::test_compare_sessions_crash_vs_normal` and `::test_performance_regression_audit_different_workloads` fail on every observed run in this environment, on `main` as well, with `save_session ... TimeoutError("method=tools/call", 30s)`. Raising the client timeout to 180 s still fails after 186 s, so the save genuinely does not complete. The test prints `Crash session stopped: 34905 events`, and `ContentStore::put` opens a redb write transaction per event with redb's default immediate durability (`set_durability` appears nowhere in `chronos-store`), so the cost is linear at roughly 4.8 ms per event — ≈168 s for 35k events, and a few seconds for the ~2,500-event saves `session_persistence` uses. Recorded as `FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT` (high) with a proposed `put_many` fix, and both failing tests are pinned in `AGENTS.md` §6.5.

## Tangential modifications

| File | Net change |
|---|---|
| `chronos-sandbox/src/client/tools.rs` | +118/−39 (private store, `resolve_mcp_path`, `db_path()`, `impl Drop`) |
| `chronos-sandbox/src/client/process.rs` | `spawn_with_env` removes the inherited `CHRONOS_DB_PATH` |
| `chronos-sandbox/tests/client_store_isolation.rs` | new, 212 lines, 3 tests |
| `chronos-sandbox/tests/session_edge_cases.rs` | SE1 shares an explicit store; un-ignored |
| `chronos-sandbox/tests/counterexample_tools.rs` | ce12 drops `set_var`/`remove_var` |
| `AGENTS.md` | §6.5 gains the two pre-existing `session_edge_cases` failures |
| `scripts/smoke_test_ccs.sh` | expected CC counts 47 → 48 python |
| `maintenance/vault-drift-sweep.md` | CC#56 documented |
| `m9-67` / `m9-68` archive manifests | two SHA rows regenerated |

Two commits:
- `d4919eec670e133f55758ba6a37301aee92f9570` — `fix(sandbox): give every MCP test client a private session store (m9-73)`
- `456247521285fc4899ed134e370340e689ca38ca` — `feat(m9-73): cycle artifacts (apply-checkpoint, verify, release-report)` (tag `v0.7.75`)

## Cross-checks

- CC#1..CC#56: pass (no drift). Python CC count 47 → 48; bash unchanged at 7. `scripts/smoke_test_ccs.sh` updated for CC#48/CC#54 and passing 5/5.
- CC#4: vault files changed by this cycle are `cycles/index.md`, `terms/index.md`, `scripts/smoke_test_ccs.sh` and `maintenance/vault-drift-sweep.md`; the archive manifests that list them had their SHA rows regenerated to a fixpoint (second pass reported 0 updates). Nine older manifests whose only stale row is the self-referential `archive-manifest (this file)` row were deliberately left alone.
- CC#24 / CC#31: `verify-report.md` carries `## Cross-checks`; this manifest carries `Base SHA` and `Head SHA`.
- CC#12: `main_sha == head_sha == remote_tag_peel`.
- T0: fmt + clippy (`-D warnings`) clean workspace-wide.
- T2: `cargo test -p chronos-sandbox --lib` → 3 passed.
- T4-smoke: `client_store_isolation` 3/3, `session_persistence` 4/4, `counterexample_tools` 12/12, `e2e_connectivity` 1/1, `session_edge_cases` 4/2 (both failures pre-existing on `main`), all with `--test-threads=1`.

## Follow-ups (deferred)

- **FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT** (high): `ContentStore::put` commits one write transaction per event and `save_session` loops over it; batch with `put_many` into a single transaction. This is the fix that makes large sessions saveable and would turn the red `session_edge_cases` tests green.
- **FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE** (medium): `chronos-mcp::open_default_store` silently degrades to an in-memory store when the configured store cannot be opened, so a locked store yields successful saves and empty listings.
- **FIND-M9-73-SESSION-EDGE-CASES-HEAVY-SAVE-NEVER-COMPLETES** (medium): the two `session_edge_cases` tests that are red on this environment and on `main`; the observable symptom of the finding above.
- **FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO** (low): land the manifest-SHA regeneration as a repo script with the skip-self-row rule.
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low): unchanged.
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low): unchanged; the affected set grows by one manifest per cycle.
- **Sandbox warm-up ordering** and **5+19 not-merged branches triage**: preserved.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-73-sandbox-client-store-isolation/archive-manifest.md` | `0000000000000000000000000000000000000000000000000000000000000000` |
| source (sandbox client tools) | `chronos-sandbox/src/client/tools.rs` | `054ad67e3b4fd871de0725b515b53a1d7f4ec06bc059eae1edbc7f222315fb97` |
| source (sandbox process spawn) | `chronos-sandbox/src/client/process.rs` | `8306aa18d6b688e30c7531c3ee00365e936191a0e20df9ba2c0317b4975f236c` |
| test (store isolation) | `chronos-sandbox/tests/client_store_isolation.rs` | `a48c78a31a5a1517418e560a2982b2ec39ea435f78b627cda79d38133ce56495` |
| test (session edge cases) | `chronos-sandbox/tests/session_edge_cases.rs` | `79d913f406259d2ffaa2c2537aab2a9c0c8a4e7c90628e7c737b0043e374bae0` |
| test (counterexample tools) | `chronos-sandbox/tests/counterexample_tools.rs` | `debf760f268a40297bbb85923ad0ebb7d54d2342dbf1596395d37231a1f6a4b9` |
| docs (agents manual) | `AGENTS.md` | `83d09aa421c0c0c06a8dc28d4a5ba27dbb66772f7a1310103f3c133969fe8924` |
| script (cc smoke test) | `scripts/smoke_test_ccs.sh` | `86def49d7e23b4e521687ddcb53a384aac18ec3f91f0e2845396f90d7ac3e7d3` |
| vault maintenance (drift sweep) | `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` | `0c57c1dacf0f30bd9ac4adf201dcdf9d60ee43ae858dd50276189d140eadb302` |
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-73-sandbox-client-store-isolation/apply-checkpoint.json` | `35408c5d6ab2a0a9250228401b91f4f0b484cd7874ccb3f8b538bac2b5587931` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-73-sandbox-client-store-isolation/verify-report.md` | `d12ad15ceb8c5673312b86206d0d5eb46a45a3b4d8d4348763bded29ae95e633` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-73-sandbox-client-store-isolation/verify-findings.json` | `fb47642a7696dbd20cb3c7de5203f1aa496e16f42e872718c41ce8f5dc7dd6b1` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-73-sandbox-client-store-isolation/release-report.md` | `f70cb66b2110e7dfb1d8642a6af63262b64879f54fa70a2d43b202369e53e79a` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-73-sandbox-client-store-isolation/release-receipt.md` | `5872b2538d4b743dfa1c3b5c20894c340172d274b8a30fb657b463c5e311e3a1` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-73-sandbox-client-store-isolation/merge-receipt.md` | `9f2ee96b248a5800c1cb96ed16a13dc2e44908d26d2dcf83b07c07644f382647` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-73-sandbox-client-store-isolation/change-entry.md` | `0669b9d2740f447337a475e8cfd9c16bb041ad608ffc732c0e9a53e99fa2e31e` |
| vault index (cycles) | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | `2ccfbb229fe92af1f647440596a5bc4ee76a55571949ede31bd3c56b25bad5df` |
| vault index (terms) | `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | `befab10ccae378abd04549f0e1316c38cc93157509e62a1e5d5c1edc71c9ccdc` |
