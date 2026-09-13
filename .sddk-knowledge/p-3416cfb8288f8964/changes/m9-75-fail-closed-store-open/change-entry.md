# Change: m9-75 fail-closed store open

## Summary

Closes `FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE`, the
medium finding m9-70's hermetic-test work exposed and m9-73's falsification
confirmed — and that both cycles deferred, because the remedy is a policy
decision rather than a bug fix.

`ChronosServer::open_default_store` opened the configured session store and, on
**any** failure, logged `Could not open session store at … Using in-memory store.`
at `warn` level and continued with an empty in-memory store. A server pointed at a
store that exists and cannot be opened — locked by a second `chronos-mcp` against
one `CHRONOS_DB_PATH`, corrupt, permission denied — therefore started
successfully, answered `session_save` with success, answered `session_list` with
nothing, and reported healthy. Silent data loss behind a green health check, and
indistinguishable from an empty store by any client.

The fix makes store opening a decision the process cannot ignore:

- `StoreOpenError { path, cause }` names the path, the cause and the opt-in in its
  `Display`, exposes `path()` / `cause()` and returns the store error from
  `Error::source()`. The cause is boxed because the first clippy pass rejected a
  192-byte `Err` variant (`clippy::result_large_err`); boxing brings it to 32 bytes.
- Three pure functions carry the policy, so it is testable without mutating the
  process environment: `default_store_path(db_path, home)` prefers
  `$CHRONOS_DB_PATH` and falls back to `$HOME/.local/share/chronos/sessions.redb`,
  with empty values falling through in both cases; `allow_in_memory_fallback(raw)`
  accepts only `1` / `true` / `yes` after trimming and lowercasing, so a typo
  cannot re-enable silent degradation; `open_store_at(path, allow)` returns
  `Result<SessionStore, StoreOpenError>`, creates missing parent directories, and
  degrades only when the caller opted in.
- `ChronosServer::try_new()` is the new entrypoint, `new()` panics with the same
  message, and the binary uses `try_new()` and exits `2` with
  `chronos-mcp: fatal: <message>` on stderr **before** the transport starts. A path
  that does not exist yet is still created, so first-run behaviour is unchanged.

The interesting part of the cycle is the verification problem, not the fix. The
policy is invisible through the MCP tool surface: a degraded server and a healthy
one answer the same tools and differ only in the data, which is precisely why the
finding survived two cycles. It *is* visible in the process exit status, so
`chronos-sandbox/tests/store_open_failure.rs` spawns the real binary with
`CHRONOS_DB_PATH` pointing at a **directory** — a path that exists, so the
missing-parent allowance cannot excuse it, and that can never be a redb database —
and asserts exit code `2` plus a stderr line naming the path and the cause and no
`initialize` response. A second test starts the same fixture with
`CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1` and asserts the server **does** serve, which
shows both that the opt-in is the difference and that the fixture is genuinely
unopenable rather than merely disliked.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-75-fail-closed-store-open` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `8e68fc8b9a56b30852ced43733dcbd07a89c3201` |
| Head SHA | `ac33be59c8c177b7afef79d4440d8841dd7003d1` |
| Tag | `v0.7.77` |

## Subject

- base_sha: `8e68fc8b9a56b30852ced43733dcbd07a89c3201`
- head_sha: `ac33be59c8c177b7afef79d4440d8841dd7003d1`
- source commits: `4f7c781` (code + tests + docs), `ac33be5` (artifacts)
- cycle: m9-75
- branch: `feat/m9-75-fail-closed-store-open`
- date: `2026-09-13T17:20Z`
- tag: `v0.7.77`
- findings_closed: 1 (`FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE` medium)
- findings_introduced (deferred): 1 new (`FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE` low), plus 4 inherited low rows re-listed
- new tests: 6 (4 in `server.rs`, 2 in the new sandbox suite)

## Files changed

- (modified) `crates/chronos-mcp/src/server.rs` — `StoreOpenError`, `default_store_path`, `allow_in_memory_fallback`, `open_store_at`, `try_new`, `new` panicking with the message, `try_open_default_store` in both `cfg(test)` and production variants, four tests plus two helpers
- (modified) `crates/chronos-mcp/src/bin/chronos-mcp.rs` — `try_new()`, `tracing::error!`, `eprintln!("chronos-mcp: fatal: {}", e)`, `std::process::exit(2)` before the transport
- (modified) `chronos-sandbox/src/client/tools.rs` — `resolve_mcp_path()` is public so the new suite spawns the same binary as every other suite
- (new) `chronos-sandbox/tests/store_open_failure.rs` — 138 lines, 2 tests, drives the real binary
- (modified) `AGENTS.md` — §1 records that the bucket-C binary lookup uses the last built binary, so a stale `target/debug/chronos-mcp` silently tests the previous commit
- (modified) `docs/manual-ai/en/08-session-management.md` — "When the store cannot be opened": fail closed with exit `2`, missing path still initialised, `CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1`
- (modified) `docs/manual-ai/es/08-gestion-sesiones.md` — the same section in Spanish
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-75-fail-closed-store-open/*`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-75-fail-closed-store-open/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-75-fail-closed-store-open/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-75 row, Total cycles 74→75)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (the m9-73 fallback row terminated as closed; one new deferred row)

## Cross-checks

CC#1..CC#56: pass (48 python + 7 bash, counts unchanged — no new check this
cycle). T0 fmt + clippy (`-D warnings`) clean workspace-wide. T2: `-p chronos-mcp
--lib` 82 passed (was 78), `-p chronos-store --lib` 74 passed. T3: workspace minus
`chronos-sandbox` minus `chronos-e2e` green, plus `-p chronos-native --lib
--test-threads=1` 101 passed and `-p chronos-mcp --tests` green. T4-smoke:
`store_open_failure` 2/2, `client_store_isolation` 3/3, `e2e_connectivity` 1/1,
`session_persistence` 4/4, all serial with `CHRONOS_MCP_PATH` exported. CC#4: the
regeneration loop reached a fixpoint (second pass 0 updates) and unrelated churn in
the nine pre-m9-11 manifests was reverted. CC#12: `main_sha == head_sha ==
remote_tag_peel` for `v0.7.77`.

## Falsification evidence

| Reverted | Observed result |
|---|---|
| `open_store_at`'s `Err` branch back to the unconditional `SessionStore::in_memory()` | `test_open_store_at_fails_closed_instead_of_degrading_silently` FAILED at `server.rs:6237` — the `Err` case returned `Ok` |
| Same revert, rebuilt binary, acceptance suite | `test_server_refuses_to_start_when_the_configured_store_cannot_be_opened` FAILED — `ExitStatus(unix_wait_status(256))` (exit 1) instead of `Some(2)`, stderr showing `Using in-memory store.` with the server serving until stdin closed |

Both restored byte-identically (`cmp` against
`/home/rubentxu/.jcode/scratch/server.rs.m9-75-good`) and re-run green.

A third observation is not a revert and is the reason `AGENTS.md` gained a
paragraph: the **first** acceptance run was green against a stale
`target/debug/chronos-mcp` built before this cycle, because the sandbox
`resolve_mcp_path()` ladder prefers the built binary over `CHRONOS_MCP_PATH` in the
test process. Only the assertion's stderr dump — which showed the pre-m9-75
`Using in-memory store.` wording — revealed it. Rebuilding and re-running produced
the real result.

## Follow-ups (deferred)

- **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** (low, new): with the
  opt-in set, the degraded mode is logged but never surfaced in a tool response.
- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low, unchanged).
- **FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO** (low, fourth confirmation).
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low, unchanged).
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, unchanged).
- **Sandbox warm-up ordering** (preserved): `test_session_start_via_v2_then_session_stop_via_v2`
  still not reproducible.
- **5+19 not-merged branches triage** (preserved from m9-65): human review needed.
