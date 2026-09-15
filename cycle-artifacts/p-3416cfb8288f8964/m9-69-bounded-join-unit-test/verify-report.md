# Verify Report — m9-69

**Cycle**: m9-69-bounded-join-unit-test
**Path**: B-direct

## Summary

Two-commit B-direct cycle that makes the HIGH-5 bounded-join contract in `stop_probe` unit-testable. The timeout was hardcoded at 10s inside `NativeProbeBackend::stop_probe`, so the timeout branch (the whole point of the HIGH-5 guard introduced in m9-62) had no test: exercising it would have cost 10 seconds per run. The pattern is extracted into a `pub(crate) fn bounded_join_with_timeout(handle, timeout) -> BoundedJoinResult` helper, called from `stop_probe` with `Duration::from_secs(10)`. Two unit tests now prove the contract in **0.10 s total**: one asserts a fast thread yields `Joined`, the other asserts a 10s-sleeping thread yields `Timeout` in ~100ms (asserting return within 250ms, i.e. proving `stop_probe` cannot block past its budget).

This closes the deferral recorded in m9-62's verify-report and repeated in m9-67/m9-68 release-reports ("Bounded join unit test: deferred from m9-62 (10s runtime blocker, may need a different test strategy)").

## Subject

| Base | Head (logic) | Dirty diff digest | CWD | Verified at |
|---|---|---|---|---|
| `5b6c337` | `dba5f0a` | `sha256:8c9737514768e979ee448982af968c78e0c006347669df0637831946af45a325` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-13T12:05:00Z |

## Files Inventory

1 file changed across the logic commit (105 insertions, 13 deletions):

| File | Change |
|---|---|
| `crates/chronos-native/src/probe_backend.rs` | Adds `BoundedJoinResult` enum and `pub(crate) bounded_join_with_timeout` helper (+64 lines including doc); `stop_probe` now calls the helper instead of the inline channel/`recv_timeout` block (−13 lines of inline code); 2 new `#[test]` functions (happy path + timeout path). No production behavior change: `stop_probe` still waits up to 10s. |

Total: 1 file, +105/−13, 2 new tests.

## Drift Evidence (pre-cycle baseline)

```
grep -c recv_timeout crates/chronos-native/src/probe_backend.rs  →  1 (inline, timeout hardcoded)
grep -c bounded_join crates/chronos-native/src/probe_backend.rs  →  0 (no helper, no test)
# The HIGH-5 timeout branch was unreachable by any test: to hit it you must
# wait the full 10s, which no test does.
```

## Drift Evidence (post-cycle state)

```
$ cargo test -p chronos-native --lib bounded_join
running 2 tests
test probe_backend::tests::bounded_join_returns_joined_when_thread_exits_quickly ... ok
test probe_backend::tests::bounded_join_returns_timeout_when_thread_overruns ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 0.10s
```

The timeout test spawns a thread that sleeps 10s, calls the helper with a 100ms budget, and asserts `Timeout` within 250ms. The 10s sleeper is detached (the waiter thread is abandoned), which is the documented production behavior on timeout — so the test asserts the contract without paying the 10s cost.

## Gates

| Gate | Status | Evidence |
|---|---|---|
| T0: cargo fmt --all -- --check | PASS | clean after `cargo fmt --all` |
| T0: cargo clippy -p chronos-native --lib -- -D warnings | PASS | Finished `dev` profile, 0 warnings |
| T1: chronos-native --lib (serial) | PASS | 101 passed; 0 failed; finished in 13.19s |
| T1: workspace --lib --no-fail-fast (serial) | PASS (1 pre-existing unrelated failure) | 1003 passed; 1 failed — see "Pre-existing failure" below |
| T4-smoke | not applicable | no probe/mcp plumbing changed; the extracted helper is a local refactor of existing logic |

## Cross-checks

- `bounded_join_returns_joined_when_thread_exits_quickly` — fast thread → `Joined`, returns < 500ms.
- `bounded_join_returns_timeout_when_thread_overruns` — 10s thread + 100ms budget → `Timeout`, returns < 250ms (proves boundedness).
- `stop_probe` still calls the helper with `Duration::from_secs(10)`; the `BoundedJoinResult` match preserves the exact log lines (`info!("Probe thread exited cleanly…")`, `warn!("…did not exit within 10s…")`, `warn!("…panicked during shutdown…")`).
- MS-RACE-FIX / HIGH-5 traceability preserved in both the `stop_probe` call site comment and the new helper doc.

## Pre-existing failure (not caused by this cycle)

`cargo test --workspace --lib` reports one failure in **`chronos-mcp`**:

```
test server::tests::test_list_sessions_after_save ... FAILED
thread '…' panicked at crates/chronos-mcp/src/server.rs:5892:9:
assertion `left != right` failed
  left: Some(true)
 right: Some(true)
```

Findings:

1. **Pre-existing**: reproduces identically on `main` at `5b6c337` (verified by checkout + isolated run).
2. **Environment-coupled, not a logic regression**: `ChronosServer::new()` opens `$HOME/.local/share/chronos/sessions.redb` when `CHRONOS_DB_PATH` is unset. On a developer machine with a populated store, `store.list_sessions()` bincode-deserializes every `SessionMetadata`; a stale-schema record makes it return `StoreError::Serialization`, which `SessionsService::list_sessions` propagates as `ListFailed` (only "does not exist" is tolerated). Proof: with `CHRONOS_DB_PATH=/tmp/m9-69-test.redb cargo test -p chronos-mcp --lib test_list_sessions_after_save` → **1 passed**.
3. **Two distinct defects** (out of scope for m9-69, tracked as a follow-up): (a) the `server.rs` tests use the real user store instead of an isolated/temp store — a test-isolation bug that pollutes `$HOME` on every run; (b) `list_sessions` hard-fails on any undeserializable record rather than tolerating/versioning stale schema.

Also observed: parallel `cargo test -p chronos-native --lib` hangs (documented in AGENTS.md §6.5 as the `ptrace_tracer` flake); it reproduces on `main` and passes 101/101 serially in 13s. This cycle's tests are unaffected.

## Notes

- **Why a free function, not a method**: the helper needs no `self`; making it `pub(crate)` at module scope lets the test call it directly with an arbitrary timeout. A method on `NativeProbeBackend` would drag in the whole server construction path for no benefit.
- **Why `BoundedJoinResult` instead of `Result`**: the three real states are `Joined` / `Timeout` / `Panicked`; they map 1:1 to the three log lines `stop_probe` already emitted. A `Result` would force a synthetic error type for the non-error `Timeout` case.
- **Why the "return within 250ms" assertion matters**: it is the assertion that encodes the HIGH-5 guarantee. If a future refactor reintroduced a blocking join, this test would fail by exceeding the budget rather than hanging the suite.
- **No new CC**: the change is a local refactor plus a test. It closes a known deferral; it does not introduce a new drift class that needs a vault cross-check (consistent with m9-65's "no new CC unless structural value").

## History

m9-62 introduced the HIGH-5 bounded-join guard in `stop_probe` to prevent a wedged probe thread from deadlocking the MCP response path. Its verify-report deferred the unit test because the hardcoded 10s timeout made the timeout branch cost 10s to exercise. m9-67 and m9-68 release-reports repeated the deferral. m9-69 closes it by extracting the pattern behind a parameterized timeout, which makes the branch testable in 0.1s. The deferral is now closed with no production behavior change.

## Findings

None — clean state. (m10-legacy-migration)
