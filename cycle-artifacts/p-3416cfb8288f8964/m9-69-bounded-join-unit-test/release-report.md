# Release Report — m9-69-bounded-join-unit-test

## Path

B-direct

## Subject

Makes the HIGH-5 bounded-join contract in `NativeProbeBackend::stop_probe` unit-testable. The 10s timeout was hardcoded inline, so the timeout branch — the entire reason the guard exists — had no test (exercising it cost 10s). Extracted `pub(crate) fn bounded_join_with_timeout(handle, timeout) -> BoundedJoinResult`; `stop_probe` calls it with `Duration::from_secs(10)` and maps the three results onto its identical log lines. Two new tests prove both branches in 0.10s.

## Files changed

| Group | Count | Change |
|---|---|---|
| `crates/chronos-native/src/probe_backend.rs` | 1 | +105/−13: `BoundedJoinResult` enum, `bounded_join_with_timeout` helper, `stop_probe` call site rewritten, 2 tests |
| `cycle-artifacts/…/m9-69-bounded-join-unit-test/` | 6 | new: apply-checkpoint, verify-report, verify-findings, release-report, merge-receipt, release-receipt |

Total: 1 source file + 6 new artifacts, +105/−13 in the logic commit.

## Cross-checks

| ID | Description | Status |
|---|---|---|
| T0 | `cargo fmt --all -- --check` | pass |
| T0 | `cargo clippy -p chronos-native --lib -- -D warnings` | pass (0 warnings) |
| T1 | `cargo test -p chronos-native --lib` (serial) | pass (101 passed; 0 failed; 13.19s) |
| T1 | `cargo test --workspace --lib --no-fail-fast` (serial) | pass except 1 pre-existing unrelated failure (1003 passed; 1 failed) |
| Self-test | `cargo test -p chronos-native --lib bounded_join` | pass (2 passed; 0.10s) |
| CC#12 | main_sha == head_sha == remote_tag_peel | set in post-release commit |
| CC#1..CC#55 | vault drift sweep | pass (no vault CC added or changed) |

## Self-tests performed

```
$ cargo test -p chronos-native --lib bounded_join
running 2 tests
test probe_backend::tests::bounded_join_returns_joined_when_thread_exits_quickly ... ok
test probe_backend::tests::bounded_join_returns_timeout_when_thread_overruns ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 0.10s
```

The timeout test is the load-bearing one: it spawns a 10s sleeper, calls the helper with a 100ms budget, asserts `Timeout`, and asserts the call returned in < 250ms. That return-time assertion is what encodes the HIGH-5 guarantee — a future refactor that reintroduced a blocking join would exceed the budget and fail the test rather than hang the suite.

## Pre-existing failure observed during the gate

`cargo test --workspace --lib` also runs `chronos-mcp`, where `server::tests::test_list_sessions_after_save` fails deterministically:

- Reproduces on `main` at `5b6c337` (isolated run).
- Cause: `ChronosServer::new()` opens `$HOME/.local/share/chronos/sessions.redb`; `list_sessions` bincode-deserializes every record and hard-fails on a stale-schema record.
- Proof of environment coupling: `CHRONOS_DB_PATH=/tmp/t.redb cargo test -p chronos-mcp --lib test_list_sessions_after_save` → 1 passed.
- Tracked as `FIND-M9-69-MCP-STORE-ISOLATION` (deferred; recorded in `verify-findings.json` and `apply-checkpoint.json`).

This is not a regression from m9-69 (which touches only `chronos-native`).

## History

m9-62 introduced the HIGH-5 guard and explicitly deferred the unit test ("would take 60s to simulate a wedged probe thread"). m9-67 and m9-68 release-reports restated the deferral. m9-69 closes it by parameterizing the timeout, which turns a 10s runtime blocker into a 0.10s test. The production behavior is unchanged: `stop_probe` still waits up to `Duration::from_secs(10)` and still emits the same three log lines.

## Future work

- `FIND-M9-69-MCP-STORE-ISOLATION` — isolate `chronos-mcp` server tests from the user store; make `SessionStore::list_sessions` tolerant of (or versioned against) stale `SessionMetadata` records. Recommended next cycle.
- Remaining from earlier cycles: 5+19 not-merged branch triage (human review); sandbox test warm-up ordering.
