# Change: m9-73 sandbox client store isolation

## Summary

Closes `FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT`, the deferral m9-72 recorded when its own required T4-smoke subset turned out to be flaky. The finding described one writer of `CHRONOS_DB_PATH`; recon found **three**, and only the first was benign-looking. `McpTestClient::start()` spawned the server with the caller's environment, so every sandbox client resolved the developer's `$HOME/.local/share/chronos/sessions.redb` — one 86 MB database shared by all 35 suites. `session_edge_cases` set the variable globally around two `start()` calls, which mutates the environment of the whole test binary and therefore leaks into every other test on every other thread. `counterexample_tools::ce12` set it under a comment claiming the CLI replay needed it, which is false: `replay_bundle` passes `--db <path>` explicitly.

The client now allocates a private store per instance and passes it through `start_with_env`; `start_with_db_path` remains the explicit opt-in for two clients that must share one store; the spawn helper no longer inherits an ambient `CHRONOS_DB_PATH` at all, so a developer who has it exported cannot silently redirect a sandbox server at their real database; and both global-mutation sites are gone. A new cross-check (CC#56) forbids `std::env::set_var` / `remove_var` anywhere in `chronos-sandbox`, which is what keeps the class from returning.

The interesting part of this cycle is that **falsification rejected the first version of its own test**. Removing the guard left all three tests green, because they asserted what the *client recorded* rather than what the *server opened* — and because `open_default_store` silently falls back to an in-memory store, a client that fails to open a real file still answers `session_list` with an empty list, which satisfied "client B must not see client A's session". The tests now require the recorded path to exist on disk and be non-empty. That fallback is `FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE` (medium, deferred: it is a `chronos-mcp` policy decision, not a harness one).

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-73-sandbox-client-store-isolation` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `2df028874bb74a618e126828dd84923a67030a60` |
| Head SHA | `456247521285fc4899ed134e370340e689ca38ca` |
| Tag | `v0.7.75` |

## Subject

- base_sha: `2df028874bb74a618e126828dd84923a67030a60`
- head_sha: `456247521285fc4899ed134e370340e689ca38ca`
- cycle: m9-73
- branch: `feat/m9-73-sandbox-client-store-isolation`
- date: 2026-09-13
- tag: `v0.7.75`
- findings_closed: 1 (`FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT`)
- findings_introduced (deferred): 4 (`FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT` high, `FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE` medium, `FIND-M9-73-SESSION-EDGE-CASES-HEAVY-SAVE-NEVER-COMPLETES` medium, `FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO` low)
- new tests: 3 (store isolation suite) + CC#56

## Files changed

- (modified) `chronos-sandbox/src/client/tools.rs` — `db_dir`/`db_path` fields, `resolve_mcp_path()`, `allocate_store_dir()`, `db_env()`, `db_path()`, `impl Drop for McpTestClient`; `start_path` gives every client a private store, `start_with_db_path` keeps the explicit shared path
- (modified) `chronos-sandbox/src/client/process.rs` — `spawn_with_env` removes `CHRONOS_DB_PATH` from the inherited environment before applying `extra_env`
- (new) `chronos-sandbox/tests/client_store_isolation.rs` — 3 tests: two clients differ and both store files exist, a saved session is invisible to a second client, an explicit shared path still shares; plus the ambient-decoy check
- (modified) `chronos-sandbox/tests/session_edge_cases.rs` — SE1 shares one store via `start_with_db_path` instead of mutating the process environment, and is **un-ignored**: the interference its `#[ignore]` guarded against was exactly that mutation, and it passes 2/2 when run (18.3 s each)
- (modified) `chronos-sandbox/tests/counterexample_tools.rs` — ce12 drops its redundant `set_var`/`remove_var` (the server is already started with `start_with_db_path`)
- (modified) `AGENTS.md` — §6.5 gains the two pre-existing `session_edge_cases` failures with their measured attribution
- (modified) `scripts/smoke_test_ccs.sh` — expected CC counts 47 → 48 python (CC#56)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` — CC#56 documented, with the three-writer history and the resolution
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-67-cc-smoke-test/archive-manifest.md` — `smoke_test_ccs.sh` SHA regenerated
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-68-verify-report-files-inventory-backfill/archive-manifest.md` — `smoke_test_ccs.sh` and `vault-drift-sweep.md` SHAs regenerated
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-73-sandbox-client-store-isolation/*`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-73-sandbox-client-store-isolation/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-73-sandbox-client-store-isolation/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-73 row, Total cycles 72→73)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (finding terminated; two deferred; Last archive bumped)

## Cross-checks

CC#1..CC#56: pass (no drift; 48 python + 7 bash, up one python CC from CC#56). T0 fmt + clippy (`-D warnings`) pass workspace-wide. T2: `-p chronos-sandbox --lib` 3/3, plus the changed suites in T4. T4-smoke: `client_store_isolation` (new) 3/3, `session_persistence` 4/4, `session_edge_cases` 4 passed / 2 failed (both failures pre-existing, attributed below), `counterexample_tools` 12/12, `e2e_connectivity` 1/1, all with `--test-threads=1`. Two interleaved A/Bs against `main` with alternating builds: one for `session_persistence` (fix 3/3 green, `main` reproduces the historical timeout) and one for `session_edge_cases` (both builds fail the same two tests, so it is not this cycle). CC#4: the vault-index SHA rows required by this cycle's edits were regenerated to a fixpoint; unrelated churn in nine older manifests was reverted (see the deferred finding). CC#12: `main_sha == head_sha == remote_tag_peel`.

## Falsification evidence

Four independent reverts, each observed to fail and then restored:

| Reverted | Observed result |
|---|---|
| `start_path` drops `CHRONOS_DB_PATH` from the child env (`HashMap::new()`) | `test_each_client_gets_its_own_store_path` FAILED — "server A never opened the store the client recorded"; `test_saved_session_is_invisible_to_another_client` FAILED — "client A's server never opened … so 'invisible to B' proves nothing"; `test_explicit_db_path_still_shares_one_store` still passed, correctly, because it does not use `start_path` |
| First version of the suite, guard removed (no on-disk assertion) | 3/3 **passed** — the test was vacuous, which is what forced the on-disk assertions; recorded as the reason `FIND-M9-73-…-FALLBACK` exists |
| Ambient decoy: the suite re-run with `CHRONOS_DB_PATH` exported to a scratch path | With the guard removed the decoy file appears and the check fires; with the fix in place the decoy is never created |
| CC#56, run against the pre-fix tree (`git show HEAD:` for both touched test files) | 6 hits, exactly the removed writers: `counterexample_tools.rs:599,688` and `session_edge_cases.rs:41,44,94,176`. Against the current tree: empty output |

## Pre-existing failure in the required subset (attributed, not waved through)

`session_edge_cases` fails two tests in this environment on **both** builds: interleaved A/B with alternating builds gave cycle branch 3/2/2 failures and `main` 2/2/2, always `test_compare_sessions_crash_vs_normal` and `test_performance_regression_audit_different_workloads`, always `save_session ... TimeoutError("method=tools/call", 30s)`. Raising the client timeout to 180 s still fails (186 s), so it is not calibration. The test prints `Crash session stopped: 34905 events`, and `ContentStore::put` commits one write transaction per event, which makes the save linear at roughly 4.8 ms per event (≈168 s at this volume). This is a pre-existing product defect, recorded as `FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT` (high) with a proposed fix, and both failing tests are pinned in `AGENTS.md` §6.5.

The same audit found one ignored test in that suite, SE1 — ignored because of parallel interference caused by exactly the process-global mutation this cycle removed. It passes 2/2 when run, so it is now un-ignored.

## Follow-ups (deferred)

- **FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE** (medium): `chronos-mcp::open_default_store` silently degrades to an in-memory store when the configured store cannot be opened, so a locked store yields successful saves and empty listings instead of an error.
- **FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT** (high, pre-existing, mechanism identified): `ContentStore::put` opens a redb write transaction — with an fsync, since redb's default durability is immediate and the crate never calls `set_durability` — per event, and `save_session` calls it in a loop. A 35k-event crash session therefore needs about three minutes to save (linear at ~4.8 ms/event). Proposed fix: `put_many` batching a whole session into one transaction.
- **FIND-M9-73-SESSION-EDGE-CASES-HEAVY-SAVE-NEVER-COMPLETES** (medium, pre-existing): the two `session_edge_cases` tests that fail on this environment and on `main`; they are the observable symptom of the finding above.
- **FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO** (low): the CC#4 manifest-SHA regeneration lives in agent scratch rather than in `scripts/`, and the naive whole-tree version churns the self-referential row of nine older manifests.
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low): four hand-rolled copies of the `table_error` policy in `counterexample_storage.rs`.
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low): the affected set grows by one manifest per cycle.
- **Sandbox warm-up ordering** (preserved): `test_session_start_via_v2_then_session_stop_via_v2` still not reproducible.
- **5+19 not-merged branches triage** (preserved from m9-65): human review needed.
