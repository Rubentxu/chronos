# Release Report — m9-72-read-path-table-error-classification

## Path

B-direct

## Subject

Closes `FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE` (deferred by m9-71). Four
read paths in `chronos-store` answered *every* `open_table` failure on a read
transaction with a benign value: `cas::get` → `Ok(None)`, `cas::contains` →
`Ok(false)`, `load_session` SESSION_META → `SessionNotFound`, `load_session`
SESSION_EVENTS → `SessionNotFound`. Only redb's `TableDoesNotExist` is benign;
`TableTypeMismatch` (a store written by a different binary) and any future
variant are faults.

Recon corrected the deferral twice: the real count is **four** sites (m9-71
recorded two and did not mention `cas.rs`), and the collapse has an observable
cost, because `load_session` consumes `cas::get` in a loop — a fault answered
with `Ok(None)` silently removed events from the loaded session and reported
success on a truncated session.

New `table_error` module owns the distinction (`Absent` vs `Fault`) plus the two
answer shapes the callers need. The sibling module `counterexample_storage.rs`
already matched `TableDoesNotExist` explicitly, so this puts the crate on the
policy three of its modules already followed.

## Files changed

| Group | Count | Change |
|---|---|---|
| `crates/chronos-store/src/table_error.rs` | 1 | new: classifier + 4 unit tests (~140 lines) |
| `crates/chronos-store/src/lib.rs` | 1 | `mod table_error;` registered |
| `crates/chronos-store/src/cas.rs` | 1 | 2 sites + 1 behavioural test asserting both |
| `crates/chronos-store/src/storage.rs` | 1 | 2 sites + 2 behavioural tests (META, EVENTS) |
| `AGENTS.md` | 1 | T3 excludes `chronos-e2e` (hangs on ptrace, bucket D); §6.5 records the hang |
| `cycle-artifacts/…/m9-72-read-path-table-error-classification/` | 6 | new: apply-checkpoint, verify-report, verify-findings, release-report, merge-receipt, release-receipt |

Total: 4 source files (1 new), +103/−4, 7 new tests (4 unit + 3 behavioural).

## Cross-checks

| ID | Description | Status |
|---|---|---|
| T0 | `cargo fmt --all -- --check` | pass |
| T0 | `cargo clippy --workspace --all-targets -- -D warnings` | pass (0 warnings) |
| T2 | `cargo test -p chronos-store -p chronos-services -p chronos-mcp --tests --no-fail-fast` | pass (69 store lib; 263 services lib; 77 + 49 mcp; 0 failed) |
| T3 | `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e --exclude chronos-native --no-fail-fast` | pass |
| T3 | `cargo test -p chronos-native --lib -- --test-threads=1` | pass (parallel run hangs, documented pre-existing) |
| T4-smoke | `cargo test -p chronos-sandbox --test session_persistence --test counterexample_tools --test e2e_connectivity -- --test-threads=1` | flaky, characterised (see below) |
| CC#1..CC#55 | `./scripts/check_vault_drift.sh` | pass (47 python CCs clean, 7 bash CCs clean) |
| CC smoke | `./scripts/smoke_test_ccs.sh` | pass (5 run, 0 failures) |
| CC#12 | `main_sha == head_sha == remote_tag_peel` | pass |

## Self-tests performed

| Check | Command | Observation |
|---|---|---|
| Fault is genuine | incompatible `TableDefinition` on the same table name | redb returns `TableTypeMismatch` on the read-path `open_table` |
| `cas::get` guard is real | revert `Err(_) => Ok(None)` | `test_get_propagates_storage_fault_instead_of_reporting_missing_content` FAILS |
| `cas::contains` guard is real | revert `Err(_) => Ok(false)` | same test FAILS on the `contains` assertion |
| META guard is real | revert to `Err(_) => SessionNotFound` | `test_load_session_propagates_storage_fault_instead_of_session_not_found` FAILS |
| EVENTS guard is real | revert to `Err(_) => SessionNotFound` | `test_load_session_propagates_event_table_storage_fault` FAILS |
| No wildcard reads remain | `grep -rn "Err(_) =>" crates/chronos-store/src/*.rs` | 1 remaining, `counterexample_storage.rs:968`, bincode `deserialize` best-effort row skip with its comment intact |
| Tool surface intact | sandbox `session_persistence` | 4 tests, 3 pass / 1 flake (below) |
| Server reachable | sandbox `e2e_connectivity` | 1 passed |
| Store paths | sandbox `counterexample_tools` | 12 passed |
| T3 usable again | re-run the corrected T3 command after the AGENTS.md edit | terminates instead of hanging past 10 min on `test_ptrace_capture` |

## T4-smoke: flake characterised, not tolerated silently

The required subset is unstable in this environment, and the instability is a
harness defect, not this diff. `session_persistence::test_save_session_multiple_times`
fails with `save_session failed: TimeoutError("method=tools/call", 30s)` at
`session_persistence.rs:128` (first save) or `:133` (second save) in a subset of
runs of the three-binary command, and also in back-to-back runs of
`session_persistence` alone. In the same runs `counterexample_tools::ce12` logs
`Database error: Database already open. Cannot acquire lock.` for its private
scratch db.

Mechanism, after two hypotheses were tested and one dropped:
`McpTestClient::start()` → `start_path()` → `factory::start()` spawns the server
with the caller's environment, so every sandbox client resolves
`default_db_path()` = `$HOME/.local/share/chronos/sessions.redb` — one 86 MB store
shared by all 35 suites — and `session_save` serializes ~2500 compressed events
into it behind a fixed 30 s client timeout. The failing runs are the slow ones
(76-80 s of suite wall time against 55-64 s for passes) and the failing
assertion moves between the two saves, so the trigger is elapsed time, not data.

The dropped hypothesis is recorded so it is not re-derived: a leaked
`chronos-mcp` holding the redb lock. `pgrep -fc chronos-mcp` seemed to show
survivors after each run, but that pattern matches the runner script's own
command line; with a specific pattern
(`pgrep -fc 'cargo-targets/debug/chronos-mcp'`) the count is 0 before and after
every run, so nothing leaks and no lock is held.

Attribution was measured, not assumed. `main` and the cycle branch were built
alternately and run against the identical command (4 runs), then
`session_persistence` alone was run three times back-to-back per build with the
same binary within each group:

| Group | Run 1 | Run 2 | Run 3 |
|---|---|---|---|
| three-binary A/B | cycle branch **fail** (101) | `main` pass, cycle pass, `main` pass | — |
| cycle branch, one binary | pass (60 s) | **fail** (76 s, line 133) | pass (80 s) |
| `main`, one binary | pass (59 s) | pass (64 s) | **fail** (80 s, line 133) |

The last cell is the exoneration: the identical failure reproduces on `main`,
whose build contains none of this cycle's code, and the same cycle-branch binary
both passed and failed. Recorded as
`FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT` (medium).

## Pre-existing observations

- `counterexample_storage.rs` keeps four inline copies of the policy `table_error`
  now names. Correct, duplicated; deferred as
  `FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION` (low).
- Sandbox warm-up-ordering flake (`test_session_start_via_v2_then_session_stop_via_v2`):
  not reproducible this cycle either.
- `FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION` is unchanged and now has
  one more manifest in its set (m9-71's), so each cycle's vault-index edit must
  regenerate three prior manifests instead of two.

## History

| Timestamp | Event |
|---|---|
| 2026-09-13T14:50Z | cycle opened on `feat/m9-72-read-path-table-error-classification` off `09e2110` |
| 2026-09-13T14:55Z | `table_error` module added; 4 sites routed through it; 7 tests added |
| 2026-09-13T15:00Z | 4/4 reverts falsified each guard, then restored |
| 2026-09-13T15:01Z | T0 / T2 green; T4-smoke flake characterised with interleaved A/B |
| 2026-09-13T15:04Z | logic commit `845be8d` |
| 2026-09-13T15:10Z | artifacts commit; tag `v0.7.74` |

## Future work

- `FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT` (medium): give each
  sandbox client its own temporary `CHRONOS_DB_PATH`; `start_with_db_path` exists
  and is only used by `ce12`.
- `FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION` (low): fold the four
  inline matches in `counterexample_storage.rs` onto `table_error`.
- Triage of the 5 local + 19 remote unmerged branches (human review).
