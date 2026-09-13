# Release Report — m9-75-fail-closed-store-open

## Cycle

| Campo | Valor |
|---|---|
| Cycle | `m9-75-fail-closed-store-open` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `8e68fc8b9a56b30852ced43733dcbd07a89c3201` |
| Code SHA | `4f7c7817cdca87677e1fc958cbbcdfac8a264dc0` |
| Branch | `feat/m9-75-fail-closed-store-open` |
| Tag | `v0.7.77` |
| Findings closed | 1 (`FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE`, medium) |
| Findings introduced | 1 deferred (`FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE`, low) |

## What shipped

The session store is now a decision the server cannot ignore. `open_store_at`
returns `Result<SessionStore, StoreOpenError>`; the `Err` branch is fail-closed
unless the caller opted in through `CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1` (strict:
`1`/`true`/`yes`, trimmed, case-insensitive). `ChronosServer::try_new()` is the
new entrypoint, `new()` panics with the error message, and `chronos-mcp` uses
`try_new()` and exits `2` with `chronos-mcp: fatal: <message>` on stderr **before**
starting the transport. A path that does not exist yet is still created, so
first-run behaviour is unchanged.

`StoreOpenError` is `Debug + Display + Error`, names the path, the cause and the
opt-in, and boxes its cause: the first clippy pass failed with
`clippy::result_large_err` on a 192-byte `Err` variant, and boxing brings it to 32
bytes. That is the whole of the production diff apart from the binary and the
`resolve_mcp_path()` visibility change.

## Commits

| SHA | Subject |
|---|---|
| `4f7c7817cdca87677e1fc958cbbcdfac8a264dc0` | `fix(mcp): refuse to start when the configured session store cannot be opened (m9-75)` |
| `TBD_ARTIFACTS_SHA` | `feat(m9-75): cycle artifacts (apply-checkpoint, verify, release-report)` (tag `v0.7.77`) |

## Diff summary

| File | Lines |
|---|---|
| `crates/chronos-mcp/src/server.rs` | +356/−41 (error type, three policy functions, `try_new`, both `try_open_default_store` variants, 4 tests + 2 helpers) |
| `crates/chronos-mcp/src/bin/chronos-mcp.rs` | +11/−1 (`try_new`, fatal log, `exit(2)`) |
| `chronos-sandbox/src/client/tools.rs` | +6/−1 (`resolve_mcp_path` public) |
| `chronos-sandbox/tests/store_open_failure.rs` | new, 138 lines, 2 tests |
| `AGENTS.md` | +7 (stale-binary hazard in the bucket-C binary lookup) |
| `docs/manual-ai/en/08-session-management.md` | +18 ("When the store cannot be opened") |
| `docs/manual-ai/es/08-gestion-sesiones.md` | +18 (same section, Spanish) |

## Verification

Verify report: `cycle-artifacts/p-3416cfb8288f8964/m9-75-fail-closed-store-open/verify-report.md`.

- T0: `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
- T2: `cargo test -p chronos-mcp --lib` 82 passed (78 before, +4); `cargo test -p chronos-store --lib` 74 passed.
- T3: workspace lib minus `chronos-sandbox`/`chronos-native`/`chronos-e2e` all green; `cargo test -p chronos-native --lib -- --test-threads=1` 101 passed; `cargo test -p chronos-mcp --tests` all green.
- T4-smoke: `store_open_failure` 2/2, `client_store_isolation` 3/3, `e2e_connectivity` 1/1, `session_persistence` 4/4, `--test-threads=1` with `CHRONOS_MCP_PATH` exported.

## Falsification evidence

| # | Reverted | Observed result |
|---|---|---|
| 1 | `open_store_at`'s `Err` branch back to the unconditional `SessionStore::in_memory()` | `test_open_store_at_fails_closed_instead_of_degrading_silently` FAILED at `server.rs:6237` (the `Err` case returned `Ok`) |
| 2 | Same revert + `cargo build --bin chronos-mcp` | `test_server_refuses_to_start_when_the_configured_store_cannot_be_opened` FAILED: `ExitStatus(unix_wait_status(256))` (exit 1) instead of `Some(2)`, stderr showing `Using in-memory store.` and the server serving until stdin closed |

Both reverts restored byte-identically (`cmp` against
`/home/rubentxu/.jcode/scratch/server.rs.m9-75-good`) and re-run green
(82 lib tests, 2/2 acceptance).

A third observation is the reason `AGENTS.md` gained a paragraph: the **first**
acceptance run was green against a stale `target/debug/chronos-mcp` built before
this cycle, because `resolve_mcp_path()` prefers the built binary over
`CHRONOS_MCP_PATH` in the test process. The assertion's stderr dump showed the
pre-m9-75 warning, which is how it was caught. Rebuilding and re-running produced
the real result, and the same hazard is why the falsification rebuild step is
explicit in rows 2 above.

## Cross-checks

- CC#1..CC#56: pass (48 python + 7 bash; no new check this cycle). CC smoke 5/5.
- CC#12: `main_sha == head_sha == remote_tag_peel` for tag `v0.7.77`.
- CC#4: archive-manifest artifact indexes regenerated to a fixpoint (second pass 0 updates); unrelated churn in the nine pre-m9-11 manifests reverted; `m9-67`/`m9-68` self-rows restored to their committed values.
- CC#36 / CC#55 / CC#39: `verify-report.md` carries `Path` at the head and `## Files Inventory`; `verify-findings.json` carries `lens_summary` after `subject`; this report and `archive-manifest.md` carry `## Cross-checks`.
- CC#24 / CC#31: this report and the archive manifest carry the `Base SHA` / `Head SHA` rows.
- Annotation: tag created with `GIT_COMMITTER_NAME`/`GIT_COMMITTER_EMAIL` set (no `--local-user`); `release-receipt.md` lands in the post-release commit.

## Follow-ups (deferred)

- **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** (low, new): the opt-in
  degraded mode is logged but never mentioned in a tool response, so an opted-in
  client still cannot tell from a payload that nothing is persisted.
- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low, unchanged).
- **FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO** (low, fourth confirmation).
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low, unchanged).
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, unchanged).
- **Sandbox warm-up ordering** and **5+19 not-merged branches triage**: preserved.
