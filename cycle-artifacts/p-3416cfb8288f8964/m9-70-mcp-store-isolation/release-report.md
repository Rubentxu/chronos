# Release Report — m9-70-mcp-store-isolation

## Path

B-direct

## Subject

Closes `FIND-M9-69-MCP-STORE-ISOLATION` (deferred by m9-69). `ChronosServer::new()` opened the developer's real `$HOME/.local/share/chronos/sessions.redb` even under `cfg(test)`, and `SessionStore::list_sessions` hard-failed on the first record it could not deserialize, so `chronos-mcp`'s `server::tests::test_list_sessions_after_save` failed deterministically on a populated machine and passed on a clean one. Three fixes: best-effort listing (skip unreadable records), absent-table-is-empty reads (also a real fresh-install bug), and a hermetic `cfg(test)` store so the unit suite never touches the user's database. Production store-opening logic is byte-identical, only relocated behind `#[cfg(not(test))]`.

## Files changed

| Group | Count | Change |
|---|---|---|
| `crates/chronos-store/src/storage.rs` | 1 | +83/−10: `list_sessions` skips undeserializable records (+warn with key); `list_sessions` + `session_exists` treat `TableDoesNotExist` as the empty answer; 3 tests |
| `crates/chronos-mcp/src/server.rs` | 1 | +100/−17: `new()` → `from_store(Self::open_default_store())`; `#[cfg(not(test))]` / `#[cfg(test)]` store impls; `PathBuf` import replaced by fully-qualified paths; 1 test |
| `cycle-artifacts/…/m9-70-mcp-store-isolation/` | 6 | new: apply-checkpoint, verify-report, verify-findings, release-report, merge-receipt, release-receipt |

Total: 2 source files + 6 artifacts, +183/−27, 4 new tests.

## Cross-checks

| ID | Description | Status |
|---|---|---|
| T0 | `cargo fmt --all -- --check` | pass |
| T0 | `cargo clippy -p chronos-store -p chronos-mcp --all-targets -- -D warnings` | pass (0 warnings) |
| T2 | `cargo test -p chronos-store -p chronos-mcp --tests` | pass (store 62; mcp lib 77; 49 across 6 mcp integration binaries) |
| T2 | `cargo test -p chronos-services --lib` (consumer of `list_sessions`) | pass (263 passed; 0 failed) |
| T4-smoke | `cargo test -p chronos-sandbox --test e2e_connectivity --test session_persistence --test session_lifecycle -- --test-threads=1` | pass (13 passed; 0 failed; 118s incl. build) |
| CC#1..CC#55 | `./scripts/check_vault_drift.sh` | pass (47 python CCs all clean, 7 bash CCs all clean) |
| CC#48 / CC#54 | `./scripts/smoke_test_ccs.sh` | pass (5 tests run, 0 failures) |
| CC#12 | main_sha == head_sha == remote_tag_peel | set in post-release commit |

## Self-tests performed

Falsification-first: each fix was reverted in turn and the corresponding test was observed to fail. The first draft of the hermeticity test passed *before* the fix, because it compared two servers and `redb`'s exclusive lock made the second server fall back to in-memory rather than share the store; it was rewritten to assert the observable bug and then failed pre-fix with `found 21 session(s)`.

```
$ cargo test -p chronos-store --lib
test storage::tests::test_session_store_list_sessions_skips_unreadable_record ... ok
test storage::tests::test_list_sessions_on_virgin_store_is_empty ... ok
test storage::tests::test_session_exists_on_virgin_store_is_false ... ok
test result: ok. 62 passed; 0 failed; 0 ignored; 0 measured; finished in 0.46s

$ cargo test -p chronos-mcp --lib
test server::tests::test_list_sessions_after_save ... ok          # failed on main
test server::tests::test_default_test_server_does_not_read_the_developer_store ... ok
test result: ok. 77 passed; 0 failed; 0 ignored; 0 measured; finished in 0.84s

$ CHRONOS_MCP_PATH=… cargo test -p chronos-sandbox \
    --test e2e_connectivity --test session_persistence --test session_lifecycle -- --test-threads=1
e2e_connectivity:     1 passed; 0 failed; finished in 5.55s
session_persistence:  8 passed; 0 failed; finished in 56.99s
session_lifecycle:    4 passed; 0 failed; finished in 54.90s
```

T4-smoke was run because the change is observable through the MCP tool surface (`session_list` against a fresh store), even though no probe plumbing was touched.

## Pre-existing observations

Parallel `cargo test -p chronos-native --lib` still hangs (AGENTS.md §6.5 `ptrace_tracer` flake; serial passes 101/101). `chronos-native` is untouched by this cycle.

## History

m9-62 introduced the HIGH-5 bounded-join guard in `stop_probe`; m9-69 closed that unit-test deferral. While running m9-69's T1 gate, `chronos-mcp`'s `test_list_sessions_after_save` was found to fail deterministically on a machine with a populated `$HOME` store, and m9-69 recorded `FIND-M9-69-MCP-STORE-ISOLATION` as deferred because both candidate fixes have behavior implications. m9-70 closes it: unreadable records no longer brick listing, a missing table means "no sessions", and the test suite no longer reads or writes the developer's database. A third, lower-severity defect (`FIND-M9-70-VIRGIN-STORE-READ-ERROR`, `session_list` erroring on a brand-new store) was surfaced by the new hermetic test and closed in the same cycle.

## Future work

- `FIND-M9-70-SERVICES-TABLE-STRING-MATCH` (deferred, low): `chronos-services::sessions::list_sessions` still substring-matches the error text for a missing table; now unreachable, decide delete vs typed match in a future cycle.
- Preserved from earlier cycles: 5+19 not-merged branch triage (human review); sandbox test warm-up ordering (`test_session_start_via_v2_then_session_stop_via_v2` fails alone, passes with the full file).
