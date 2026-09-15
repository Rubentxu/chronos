# Verify Report — m9-75-fail-closed-store-open

| Campo | Valor |
|---|---|
| Cycle | `m9-75-fail-closed-store-open` |
| Path | B-direct |
| Base SHA | `8e68fc8b9a56b30852ced43733dcbd07a89c3201` |
| Head SHA (pre-artifacts) | `4f7c7817cdca87677e1fc958cbbcdfac8a264dc0` |
| Diff digest | `sha256:0b6ec6784781dacfcd62003d60e43cfbafb6804cadb33e8f85b7b00fdc0dab2d` |
| Branch | `feat/m9-75-fail-closed-store-open` |
| Verified at | `2026-09-13T17:15Z` |
| Working tree | clean at verification time; artifact writes follow |
| Verdict | **passed** |

## Summary

Closes `FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE`, the
medium finding m9-70 and m9-73 both left open: `ChronosServer::open_default_store`
opened the configured store and, on **any** failure, logged
`Could not open session store at ... Using in-memory store.` at `warn` level and
continued with an empty in-memory store. A server pointed at a store that exists
but cannot be opened (locked by a second process, corrupt, permission denied)
therefore started successfully, answered `session_save` with success, answered
`session_list` with nothing, and reported healthy. That is silent data loss with
a green health check.

The fix makes opening the store a decision the process cannot ignore:

- `pub struct StoreOpenError { path, cause }` names the path, the cause and the
  opt-in in its `Display`, and its `Error::source()` is the store error.
- Three pure functions carry the policy: `default_store_path(db_path, home)`
  (`$CHRONOS_DB_PATH`, else `$HOME/.local/share/chronos/sessions.redb`, with empty
  values falling through), `allow_in_memory_fallback(raw)` (strict: `1`/`true`/`yes`,
  trimmed, case-insensitive) and `open_store_at(path, allow_in_memory_fallback)` which
  returns `Result<SessionStore, StoreOpenError>` and only degrades when the caller
  opted in.
- `ChronosServer::try_new() -> Result<Self, StoreOpenError>` is the new
  entrypoint; `ChronosServer::new()` panics with the same message; the binary uses
  `try_new()` and, on `Err`, logs the error, prints
  `chronos-mcp: fatal: <message>` to stderr and `exit(2)` **before** the transport
  starts.

The interesting part is the test that fails when the fix is reverted, and why it
had to be written at the binary level. The policy is not observable through the
MCP tool surface: a degraded server and a healthy server answer the same tools,
just with different data. It *is* observable in the process's exit status, which
is why `chronos-sandbox/tests/store_open_failure.rs` spawns the real binary with
`CHRONOS_DB_PATH` pointing at a **directory** (a path that exists, so the
"missing parent" allowance cannot excuse it, and that can never be a redb
database) and asserts exit code `2`, a stderr line naming the path and the cause,
and no `initialize` response. The second test starts the same fixture with
`CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1` and asserts the server **does** serve, so the
opt-in is shown to be the difference and the fixture is shown to be genuinely
unopenable rather than merely disliked.

## Subject

- base_sha: `8e68fc8b9a56b30852ced43733dcbd07a89c3201` (`main` at cycle start)
- head_sha: `4f7c7817cdca87677e1fc958cbbcdfac8a264dc0` (pre-artifacts source commit: code + tests + docs)
- diff_digest: `sha256:0b6ec6784781dacfcd62003d60e43cfbafb6804cadb33e8f85b7b00fdc0dab2d`
- working_tree_clean: true (at verification time)
- verified_at: `2026-09-13T17:15Z`
- cwd: `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos`
- branch: `feat/m9-75-fail-closed-store-open`
- tag: `v0.7.77`
- findings_closed: 1 (`FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE`, medium)
- findings_introduced (deferred): 1 new low plus 4 inherited low rows

## Closed finding

- **FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE** (medium,
  `code.silent_degradation`). Gap: `open_default_store` replaced any open failure
  with `SessionStore::in_memory()` behind a `tracing::warn`, so a locked or corrupt
  store yielded successful saves and empty listings. Fix: the error type, the pure
  policy functions, `try_new()`, the `exit(2)` path in the binary, four unit tests
  and the binary-level acceptance suite above. A path that does not exist still
  gets its parent directories created, so first-run behaviour is unchanged.

## Files Inventory

| File | Change |
|---|---|
| `crates/chronos-mcp/src/server.rs` | `StoreOpenError`, `default_store_path`, `allow_in_memory_fallback`, `open_store_at`, `try_new`, `new` panics with the message, `try_open_default_store` in both `cfg(test)` variants, 4 tests + 2 helpers |
| `crates/chronos-mcp/src/bin/chronos-mcp.rs` | `try_new()`, fatal log, `eprintln!`, `exit(2)` before the transport |
| `chronos-sandbox/src/client/tools.rs` | `resolve_mcp_path()` made public so the acceptance suite spawns the same binary the other suites do |
| `chronos-sandbox/tests/store_open_failure.rs` | new, 138 lines, 2 tests, drives the real binary |
| `AGENTS.md` | notes that the sandbox binary lookup uses the last built binary, so a stale `target/debug/chronos-mcp` silently tests the previous commit |
| `docs/manual-ai/en/08-session-management.md` | "When the store cannot be opened" section: fail closed with exit `2`, missing path still initialised, `CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1` |
| `docs/manual-ai/es/08-gestion-sesiones.md` | same section in Spanish |

## Falsification evidence

Two independent reverts of the fail-closed branch, each observed to fail and then
restored byte-identically (`cmp` against
`/home/rubentxu/.jcode/scratch/server.rs.m9-75-good`):

| # | Reverted | Observed result |
|---|---|---|
| 1 | `open_store_at`'s `Err` branch back to the pre-m9-75 unconditional `SessionStore::in_memory()` | `crates/chronos-mcp/src/server.rs::tests::test_open_store_at_fails_closed_instead_of_degrading_silently` **FAILED** at `server.rs:6237` (the `Err` case returned `Ok`) |
| 2 | Same revert, rebuilt binary, acceptance suite | `test_server_refuses_to_start_when_the_configured_store_cannot_be_opened` **FAILED**: `got ExitStatus(unix_wait_status(256))` (exit 1) instead of `Some(2)`, with stderr showing `Using in-memory store.` and the server serving before the harness closed stdin |

Observation 2 is the load-bearing one: it shows the acceptance test measures the
process's real behaviour and not a helper's bookkeeping. Observation 1 shows the
unit tests do, too. After both restores: `cargo test -p chronos-mcp --lib` →
82 passed, and the acceptance suite → 2 passed.

A third observation is not a revert: the first run of the acceptance suite was
**green for the wrong reason** — it spawned a stale
`target/debug/chronos-mcp` built before this cycle and the failure text of the
assertion showed the pre-m9-75 `Using in-memory store.` warning, because the
sandbox `resolve_mcp_path()` ladder prefers the built binary over
`CHRONOS_MCP_PATH` from the test process. The binary was rebuilt, the run repeated,
and `AGENTS.md` now records the hazard (a suite that spawns the server can measure
the previous commit).

## Gates

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check` | clean |
| T0 | `cargo clippy --workspace --all-targets -- -D warnings` | clean (a first pass flagged `clippy::result_large_err` on `StoreOpenError`; the cause is boxed) |
| T2 | `cargo test -p chronos-mcp --lib` | 82 passed (was 78; +4 unit tests) |
| T2 | `cargo test -p chronos-store --lib` | 74 passed |
| T3 | `cargo test --workspace --lib --exclude chronos-sandbox --exclude chronos-native --exclude chronos-e2e --no-fail-fast` | all green |
| T3 | `cargo test -p chronos-native --lib -- --test-threads=1` | 101 passed |
| T3 | `cargo test -p chronos-mcp --tests --no-fail-fast` | all green (82 + 7 + 13 + 10 + 8 + 11) |
| T4-smoke | `store_open_failure` 2/2, `client_store_isolation` 3/3, `e2e_connectivity` 1/1, `session_persistence` 4/4, `--test-threads=1`, `CHRONOS_MCP_PATH` exported | all green |

T4-smoke was mandatory here, not optional: this cycle changes how the server
starts, which is exactly the plumbing the sandbox buckets exercise.

## Residual risk

- **`ChronosServer::new()` panics.** Three library paths call it (`new()` is the
  ergonomic constructor). That is deliberate - a library called from a test or a
  tool should not silently degrade either - but it means a caller that used to get
  an in-memory server now gets a panic. The panic message is the `Display` of the
  error, so it names the path and the opt-in. Callers that want to handle it use
  `try_new()`; the binary does.
- **`exit(2)` in the binary, not an error return.** The transport is not started
  yet, so there is nothing to unwind; a non-zero exit is the only signal an MCP
  client can observe, and the sandbox suite pins it.
- **`CHRONOS_ALLOW_IN_MEMORY_FALLBACK` is strict on purpose.** `yes`/`1`/`true`
  only, trimmed and case-insensitive; `0`, `false` and any other value keep the
  fail-closed default, so a typo cannot re-enable silent degradation.
- **The MCP tool surface does not report the degraded mode.** With the opt-in set
  the server behaves as before and no tool mentions it, so a client still cannot
  tell from a response. Closing that would mean changing the response envelope of
  the session tools; the finding was about the *silent* part, which is fixed by
  refusing to start at all.

## Cross-checks

- CC#1..CC#56: pass (48 python + 7 bash, counts unchanged - no new check this
  cycle). CC smoke 5/5.
- CC#12: `main_sha == head_sha == remote_tag_peel` for tag `v0.7.77`.
- CC#36 / CC#55 / CC#39: this report carries `Path` near the top of the document
  (the cycle's path and SHAs at the head) and the `## Files Inventory` section
  above; `verify-findings.json` carries `lens_summary` right after `subject`;
  `archive-manifest.md` and `release-report.md` carry `## Cross-checks`.
- CC#4: the archive-manifest artifact indexes were regenerated to a fixpoint
  (second pass reported 0 updates); unrelated churn in the pre-m9-11 manifests was
  reverted, and the `m9-67` / `m9-68` self-rows were restored to their committed
  values.
- T0/T2/T3/T4 as tabulated above, on the commit named in `Subject`.
- Binary-level acceptance: the same fixture (a directory as the store path) is
  observed to fail closed without the opt-in and to serve with it, so the test
  cannot pass because the fixture happens to be a valid store.

## Findings

None — clean state. (m10-legacy-migration)
