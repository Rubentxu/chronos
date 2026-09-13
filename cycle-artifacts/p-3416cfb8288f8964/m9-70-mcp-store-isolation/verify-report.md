# Verify Report — m9-70

**Cycle**: m9-70-mcp-store-isolation
**Path**: B-direct

## Summary

Two-file B-direct cycle closing `FIND-M9-69-MCP-STORE-ISOLATION`, the deferral recorded by m9-69. The defect had two faces: `ChronosServer::new()` opened the developer's real `$HOME/.local/share/chronos/sessions.redb` even under `cfg(test)`, and `SessionStore::list_sessions` hard-failed on the first record it could not deserialize. Together they made `chronos-mcp`'s `server::tests::test_list_sessions_after_save` fail deterministically on any machine with a populated store while passing on a clean one.

Three fixes landed, each with a test that was observed to fail before the fix:

1. **`list_sessions` is best-effort.** A record whose bytes do not deserialize into `SessionMetadata` (bincode is not self-describing, so an older binary's field layout is unreadable) is skipped with a `tracing::warn!` instead of failing the whole call with `Serialization`.
2. **Absent table means empty.** Reading a database that has no tables yet returns `Ok(vec![])` / `Ok(false)` instead of `TableDoesNotExist`. This was also a real fresh-install bug, independent of tests: on a brand-new `sessions.redb`, `session_list` answered with an error instead of `[]`.
3. **Tests are hermetic.** `ChronosServer::new()` was split into `from_store(store)` plus two `open_default_store()` variants: production keeps the `$CHRONOS_DB_PATH` / `$HOME` logic unchanged, and `cfg(test)` returns a fresh in-memory store. All 36 `ChronosServer::new()` sites in the unit suite are now isolated from the user's database. A test that genuinely wants a file store still opts in via `CHRONOS_DB_PATH`.

## Subject

| Base | Head (logic) | Diff digest | CWD | Verified at |
|---|---|---|---|---|
| `682817b` | `071b820` | `sha256:5897a0c7f8f33ebeb5c0667f5f1e309b1a89bb5ff76dba70ea9e22a4d5a93a24` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-13T12:12:00Z |

## Files Inventory

2 files changed across the logic commit (183 insertions, 27 deletions, 4 new tests):

| File | Change |
|---|---|
| `crates/chronos-store/src/storage.rs` | `list_sessions` skips undeserializable records (+warn, includes the key in the message); `list_sessions` and `session_exists` treat `redb::TableError::TableDoesNotExist` as the empty answer; 3 new tests (`…skips_unreadable_record`, `…virgin_store_is_empty`, `…virgin_store_is_false`). |
| `crates/chronos-mcp/src/server.rs` | `new()` split into `new()` → `from_store(store)`; two `open_default_store()` impls behind `#[cfg(not(test))]` / `#[cfg(test)]`; the `std::path::PathBuf` import became fully-qualified paths (the import was only used by the production branch, so it became unused in `lib test` builds); 1 new test (`test_default_test_server_does_not_read_the_developer_store`). |

Total: 2 files, +183/−27, 4 new tests. No production behavior change: the production store-opening logic is byte-identical, only relocated behind `#[cfg(not(test))]`.

## Drift Evidence (pre-cycle baseline)

```
$ git checkout main && cargo test -p chronos-mcp --lib test_list_sessions_after_save
test server::tests::test_list_sessions_after_save ... FAILED
  assertion `left != right` failed
    left: Some(true)   right: Some(true)
test result: FAILED. 0 passed; 1 failed; 76 filtered out

$ CHRONOS_DB_PATH=/tmp/t.redb cargo test -p chronos-mcp --lib test_list_sessions_after_save
test result: ok. 1 passed        # <- the failure is environment coupling, not logic

# The suite had no test asserting that a fresh server starts empty, and no test
# feeding list_sessions an unreadable record.
```

## Drift Evidence (post-cycle state)

```
$ cargo test -p chronos-store --lib
test result: ok. 62 passed; 0 failed

$ cargo test -p chronos-mcp --lib
test server::tests::test_list_sessions_after_save ... ok
test server::tests::test_default_test_server_does_not_read_the_developer_store ... ok
test result: ok. 77 passed; 0 failed
```

## Falsification of the new guards

Each fix was reverted in turn and the corresponding test observed to fail. Without this step the guards would be decorative: the first version of the hermeticity test passed **before** the fix (it was written to compare two servers, and `redb`'s exclusive lock made the second server fall back to in-memory rather than share the store — a green test for the wrong reason). It was rewritten to assert the observable bug instead.

| Guard reverted | Observed result |
|---|---|
| `list_sessions` best-effort skip | `test_session_store_list_sessions_skips_unreadable_record` FAILED |
| absent-table handling (list path) | `test_list_sessions_on_virgin_store_is_empty` FAILED |
| absent-table handling (exists path) | `test_session_exists_on_virgin_store_is_false` FAILED |
| `cfg(test)` hermetic store (pre-fix behavior simulated) | FAILED: `fresh test server must start with an empty store, found 21 session(s): the test store is reading the developer's $HOME database` |

## Gates

| Gate | Status | Evidence |
|---|---|---|
| T0: `cargo fmt --all -- --check` | PASS | clean |
| T0: `cargo clippy -p chronos-store -p chronos-mcp --all-targets -- -D warnings` | PASS | 0 warnings |
| T2: `cargo test -p chronos-store -p chronos-mcp --tests` | PASS | store 62 + mcp lib 77 + 49 across 6 mcp integration binaries |
| T2 (consumer): `cargo test -p chronos-services --lib` | PASS | 263 passed; 0 failed |
| T4-smoke: `cargo test -p chronos-sandbox --test e2e_connectivity --test session_persistence --test session_lifecycle -- --test-threads=1` | PASS | required: the change is visible through the MCP tool surface (`session_list` on a fresh store) |
| CC drift sweep: `./scripts/check_vault_drift.sh` | PASS | 47 python CCs all clean, 7 bash CCs all clean |
| CC smoke: `./scripts/smoke_test_ccs.sh` | PASS | 5 tests run, 0 failures |
| CC#12 | set in post-release commit | `main_sha == head_sha == remote_tag_peel` |

## Cross-checks

- `test_session_store_list_sessions_skips_unreadable_record` injects `b"\xff\xff\xff\xff\xff\xff"` under key `b"corrupt"` via `store.db().begin_write()` + `open_table(SESSION_META)` alongside one good session, and asserts exactly the good one is returned.
- `test_default_test_server_does_not_read_the_developer_store` asserts (1) a fresh server lists zero sessions and (2) saving to server `a` is invisible to server `b`. Assertion (1) is the load-bearing one; it is skipped when `CHRONOS_DB_PATH` is set, since that is the documented opt-in.
- `session_exists` has no callers outside `chronos-store` itself (verified by workspace grep), so relaxing its absent-table case cannot change a downstream contract.
- `chronos-services::sessions::list_sessions` still carries a substring-match workaround for the missing table; with the store fix that branch is unreachable, recorded as `FIND-M9-70-SERVICES-TABLE-STRING-MATCH` (deferred, low) rather than deleted in-cycle.

## Pre-existing observations (not caused by this cycle)

- Parallel `cargo test -p chronos-native --lib` hangs (AGENTS.md §6.5 `ptrace_tracer` flake). Reproduces on `main`; serial passes 101/101. `chronos-native` is untouched by this cycle.

## Notes

- **Why `cfg(test)` rather than a temp dir per test**: an in-memory store is strictly hermetic, needs no fixture teardown, and cannot leak the developer's data into a test or a test's data into the developer's DB. `CHRONOS_DB_PATH` remains the escape hatch for tests that need durability.
- **Why `#[cfg]` on the function rather than on the call site**: `new()` stays a single expression (`Self::from_store(Self::open_default_store())`), so production and test share the field-initialisation path and cannot drift.
- **Why the absent-table fix is in scope**: it was surfaced by this cycle's own test, it is a genuine fresh-install defect in the same read path, and it makes the store's contract self-consistent ("no sessions" is not an error). Leaving it would have required weakening the new test to `is_err() || is_empty()`.
- **No new CC**: local robustness plus test isolation. No new structural drift class, consistent with the "no new CC unless structural value" convention; CC#48/CC#54 counts are unchanged (47 python / 7 bash), so the smoke test's expected counts need no update.

## History

m9-62 introduced the HIGH-5 bounded-join guard; m9-69 closed that deferral. While running m9-69's T1 gate, `chronos-mcp`'s `test_list_sessions_after_save` was found to fail deterministically on any machine with a populated `$HOME` store; m9-69 recorded `FIND-M9-69-MCP-STORE-ISOLATION` as deferred because both fixes have behavior implications. m9-70 closes it, and in doing so promotes the store's read path to a consistent contract: unreadable records are skipped, a missing table is empty, and tests never touch the developer's database.
