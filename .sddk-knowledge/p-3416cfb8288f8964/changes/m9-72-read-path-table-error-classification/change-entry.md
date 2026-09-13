# Change: m9-72 read path table error classification

## Summary

Closes `FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE`, which m9-71 recorded as a two-site cosmetic inconsistency inside `SessionStore::load_session`. Recon found four sites and an observable consequence. `ContentStore::get` answered every read-path `open_table` failure with `Ok(None)` and `contains` with `Ok(false)`, and both `load_session` reads answered every one with `SessionNotFound`. Only redb's `TableError::TableDoesNotExist` is benign — a database that exists but was never written — and every other variant, including `TableTypeMismatch` on a store written by a different binary and any variant redb adds behind its `#[non_exhaustive]` enum, is a storage fault. The consequence is what turned this from a note into a cycle: `load_session` consumes `cas::get` in a loop, so a fault answered with `Ok(None)` silently removed events from the loaded session and reported success on a truncated one. A new `table_error` module now owns the distinction and all four sites route through it. The sibling module `counterexample_storage.rs` already matched `TableDoesNotExist` explicitly, so the crate is now on the policy three of its four modules already followed.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-72-read-path-table-error-classification` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `09e21107ca0707c0cd40d631eddcdb39cb3669ba` |
| Head SHA | `f3500a9071d528fd6256c5f65ef00c0ca300b903` |
| Tag | `v0.7.74` |

## Subject

- base_sha: `09e21107ca0707c0cd40d631eddcdb39cb3669ba`
- head_sha: `f3500a9071d528fd6256c5f65ef00c0ca300b903`
- cycle: m9-72
- branch: `feat/m9-72-read-path-table-error-classification`
- date: 2026-09-13
- tag: `v0.7.74`
- findings_closed: 1 (`FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE`)
- findings_introduced (deferred): 2 (`FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT` medium, `FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION` low)
- new tests: 7 (4 classifier unit + 3 behavioural)

## Files changed

- (new) `crates/chronos-store/src/table_error.rs` — `classify_read_table_error`, `TableOpenFailure::{Absent, Fault}`, `or_not_found`, `session_not_found`, 4 unit tests
- (modified) `crates/chronos-store/src/lib.rs` — registers `mod table_error;`
- (modified) `crates/chronos-store/src/cas.rs` — `get` and `contains` classify instead of matching `Err(_)`; behavioural test asserting both
- (modified) `crates/chronos-store/src/storage.rs` — both `load_session` reads classify; two behavioural tests (META, EVENTS)
- (modified) `AGENTS.md` — T3 command excludes `chronos-e2e` (it hangs; the crate is bucket D, opt-in) and §6.5 records the hang
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/release-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/release-receipt.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-72-read-path-table-error-classification/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-72-read-path-table-error-classification/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-72 row, Total cycles 71→72)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (finding terminated; two deferred; Last archive bumped)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-02-events-side-table/archive-manifest.md` (vault-index SHAs regenerated)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-70-mcp-store-isolation/archive-manifest.md` (same regeneration)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-71-services-list-store-contract/archive-manifest.md` (same regeneration; the affected set now has three members)

## Cross-checks

CC#1..CC#55: pass (no drift; no new CC — 47 python + 7 bash unchanged, so the smoke test's expected counts need no update). T0 fmt + clippy (`-D warnings`) pass workspace-wide. T1/T3: workspace `--lib --tests` passes excluding `chronos-sandbox` and `chronos-e2e` (the latter hangs on ptrace, now excluded from the documented T3 command) with `chronos-native` run serially (`--test-threads=1`, its documented parallel hang). T2: chronos-store **69** (62 → 69), chronos-services 263, chronos-mcp 77 lib + 49 integration. T4-smoke: `session_persistence` + `counterexample_tools` + `e2e_connectivity` with `--test-threads=1` — the subset is flaky for reasons outside this diff, characterised below. CC smoke 5/5 pass. CC#12: `main_sha == head_sha == remote_tag_peel == f3500a9071d528fd6256c5f65ef00c0ca300b903`.

## Falsification evidence

The fault is genuine in every test, not mocked: the table is created with an incompatible `TableDefinition` signature, so redb itself fails the later `open_table` with `TableTypeMismatch` — the same failure a store written by a different binary produces. `TableDoesNotExist` cannot be manufactured for a table that was created, which is exactly why the old `Err(_)` arms were invisible to the suite. Each of the four sites was reverted **one at a time** and the corresponding test observed to fail, then restored:

| Site reverted | Test that fails |
|---|---|
| `cas::get` (`Err(_) => Ok(None)`) | `test_get_propagates_storage_fault_instead_of_reporting_missing_content` |
| `cas::contains` (`Err(_) => Ok(false)`) | same test, on the `contains` assertion |
| `load_session` SESSION_META (`Err(_) => SessionNotFound`) | `test_load_session_propagates_storage_fault_instead_of_session_not_found` |
| `load_session` SESSION_EVENTS (`Err(_) => SessionNotFound`) | `test_load_session_propagates_event_table_storage_fault` |

4/4 confirmed. The EVENTS test writes the metadata row with the correct signature and injects only the event table, because the second `open_table` is otherwise unreachable.

## T4-smoke flake: characterised, not waved away

`session_persistence::test_save_session_multiple_times` fails with `save_session failed: TimeoutError("method=tools/call", 30s)` — a client-side RPC timeout, not a store error — at line 128 (first save) or line 133 (second save), in a subset of runs of the three-binary command and also in back-to-back runs of `session_persistence` alone. `counterexample_tools::ce12` logs `Database error: Database already open. Cannot acquire lock.` for its own scratch db in the same runs. Mechanism: `McpTestClient::start()` inherits the caller's environment, so every sandbox client resolves `default_db_path()` = `$HOME/.local/share/chronos/sessions.redb`, one 86 MB store shared by all 35 suites, and `session_save` serializes ~2500 compressed events into it behind a fixed 30 s client timeout. Failing runs are the slow ones (76-80 s of wall time against 55-64 s) and the failing assertion moves between the first and second save, so the trigger is elapsed time, not data.

Because a flaky green run proves nothing, the attribution was tested rather than argued. The harness and the failing test are untouched by this cycle (`git diff --name-only` lists only `crates/chronos-store`), the failing call is `session_save` — a write path — while the diff changes four read-path classifications, and the same failure was reproduced on `main`. Three experiments, with builds alternated so the shared store grows symmetrically:

| Experiment | Observation |
|---|---|
| Three-binary command, build alternating | cycle branch **101** · `main` 0 · cycle branch 0 · `main` 0 |
| `session_persistence` alone, cycle-branch binary, 3 back-to-back runs | 0 (60 s) · **101** (76 s, line 133) · 0 (80 s) |
| `session_persistence` alone, `main` binary, 3 back-to-back runs | 0 (59 s) · 0 (64 s) · **101** (80 s, line 133) |

The `main` failure is the exoneration: `main`'s build contains none of this cycle's code, and the cycle-branch binary produced both a pass and a failure, so the outcome does not track the build. The failing assertion moves between line 128 (first save) and 133 (second save) and failing runs are the slow ones (76-80 s vs 55-64 s), so the trigger is elapsed time behind a fixed 30 s client timeout, not data. One hypothesis was tested and dropped: "a leaked `chronos-mcp` holds the redb lock". `pgrep -fc chronos-mcp` appeared to show survivors, but that pattern matched the wrapper script's own command line; with a specific pattern the count is 0 before and after every run. No process leaks.

## Follow-ups (deferred)

- **FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT** (medium): give each sandbox client a unique temporary `CHRONOS_DB_PATH`; `start_with_db_path` already exists and is used only by `ce12`.
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low): `counterexample_storage.rs` keeps four hand-rolled copies of the policy `table_error` now names.
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low): unchanged, and the affected set grew to three manifests this cycle.
- **Sandbox warm-up ordering** (preserved): `test_session_start_via_v2_then_session_stop_via_v2` still not reproducible.
- **5+19 not-merged branches triage** (preserved from m9-65): human review needed.
