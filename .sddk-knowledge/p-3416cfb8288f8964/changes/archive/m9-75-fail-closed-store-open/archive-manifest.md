# Archive Manifest — m9-75-fail-closed-store-open

## Summary

m9-75 closes `FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE`, the
medium finding that m9-70's hermetic-test work exposed and m9-73's falsification
confirmed, and that both cycles deferred because the remedy is a policy decision
rather than a bug fix. `ChronosServer::open_default_store` opened the configured
session store and, on any failure, logged `Could not open session store at … Using
in-memory store.` at `warn` level and continued with an empty in-memory store. A
server pointed at a store that exists and cannot be opened — locked by a second
`chronos-mcp` against one `CHRONOS_DB_PATH`, corrupt, permission denied — therefore
started successfully, answered `session_save` with success, answered
`session_list` with nothing, and reported healthy: silent data loss behind a green
health check, and indistinguishable from an empty store by any client.

The fix makes store opening a decision the process cannot ignore. `StoreOpenError`
names the path, the cause and the opt-in, and boxes its cause (the first clippy
pass rejected a 192-byte `Err` variant; boxing brings it to 32 bytes).
`default_store_path`, `allow_in_memory_fallback` and `open_store_at` carry the
policy as pure functions, `ChronosServer::try_new()` is the new entrypoint, and
the binary exits `2` with `chronos-mcp: fatal: …` on stderr before the transport
starts. A path that does not exist yet is still created, so first-run behaviour is
unchanged, and the degraded mode survives only behind an explicit
`CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1`.

The cycle's verification problem was observability: a degraded server and a
healthy one answer the same MCP tools and differ only in the data, so the policy is
visible only in the process exit status. `chronos-sandbox/tests/store_open_failure.rs`
therefore spawns the real binary with `CHRONOS_DB_PATH` pointing at a **directory**
— which exists, so the missing-parent allowance cannot excuse it, and which can
never be a redb database — and asserts exit `2` plus a stderr line naming the path
and the cause; a second test starts the same fixture with the opt-in and asserts
the server does serve, so both the opt-in and the fixture's unopenability are
demonstrated rather than assumed. One commit on
`feat/m9-75-fail-closed-store-open`. Tag `v0.7.77`.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-75-fail-closed-store-open |
| Base SHA | `8e68fc8b9a56b30852ced43733dcbd07a89c3201` |
| Head SHA | `TBD_HEAD_SHA` |
| Path | B-direct |
| Date | `2026-09-13T17:20Z` |
| Branch | `feat/m9-75-fail-closed-store-open` |
| Tag | `v0.7.77` |
| Tag peel SHA | `TBD_HEAD_SHA` |
| Peel match | `TBD_HEAD_SHA` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE]`
- **`verify-findings.json`**: 1 finding closed (medium `code.silent_degradation`), 5 deferred (low `code.silent_degradation` new, low `test.environmental_failure`, low `process.undocumented_tooling`, low `code.duplicated_policy`, low `process.growing_regeneration_set`), verdict `passed`
- **`verify-report.md`**: Path, Summary, Closed finding, Files Inventory, Falsification evidence, Gates, Residual risk, Cross-checks
- **`merge-receipt.md`**: `Base SHA | 8e68fc8…`, `Head SHA | TBD_HEAD_SHA`
- **`release-receipt.md`**: `Remote tag | v0.7.77`, `Peel match | true`

## Falsification evidence

Three observations (two reverts plus one stale-binary accident), each recorded
with what it showed:

| # | Configuration | Observed result |
|---|---|---|
| 1 | `open_store_at`'s `Err` arm back to the pre-m9-75 unconditional `SessionStore::in_memory()` | `test_open_store_at_fails_closed_instead_of_degrading_silently` **FAILED** at `server.rs:6237` — the `Err` case returned `Ok` |
| 2 | Same revert plus `cargo build --bin chronos-mcp` | `test_server_refuses_to_start_when_the_configured_store_cannot_be_opened` **FAILED** — `ExitStatus(unix_wait_status(256))` (exit 1) where `Some(2)` is required; stderr showed `Using in-memory store.` and the server served until stdin closed |
| 3 | First acceptance run, before rebuilding the binary (not a revert) | **passed for the wrong reason** — the sandbox `resolve_mcp_path()` ladder preferred a stale `target/debug/chronos-mcp` built before this cycle, and only the assertion's stderr dump (pre-m9-75 wording) revealed it |

Rows 1 and 2 were restored byte-identically (`cmp` against
`/home/rubentxu/.jcode/scratch/server.rs.m9-75-good`) and re-run green: 82 lib
tests, 2/2 acceptance. Row 3 is why `AGENTS.md` now records that a suite spawning
the server can measure the previous commit, and why the rebuild step is explicit
in row 2.

## Tangential modifications

| File | Net change |
|---|---|
| `crates/chronos-mcp/src/server.rs` | +356/−41 (`StoreOpenError`, three policy functions, `try_new`, both `try_open_default_store` variants, 4 tests + 2 helpers) |
| `crates/chronos-mcp/src/bin/chronos-mcp.rs` | +11/−1 (`try_new`, fatal log, `exit(2)`) |
| `chronos-sandbox/src/client/tools.rs` | +6/−1 (`resolve_mcp_path` public) |
| `chronos-sandbox/tests/store_open_failure.rs` | new, 138 lines, 2 tests |
| `AGENTS.md` | §1: the bucket-C binary lookup uses the last built binary |
| `docs/manual-ai/en/08-session-management.md` | +18, "When the store cannot be opened" |
| `docs/manual-ai/es/08-gestion-sesiones.md` | +18, same section |

One commit:
- `4f7c7817cdca87677e1fc958cbbcdfac8a264dc0` — `fix(mcp): refuse to start when the configured session store cannot be opened (m9-75)`
- `TBD_HEAD_SHA` — `feat(m9-75): cycle artifacts (apply-checkpoint, verify, release-report)` (tag `v0.7.77`)

## Cross-checks

- CC#1..CC#56: pass (48 python + 7 bash, counts unchanged — no new check this
  cycle). CC smoke 5/5.
- CC#4: the vault files changed by this cycle are `cycles/index.md`,
  `terms/index.md` and the new change/archive entries; the archive manifests that
  list them had their SHA rows regenerated to a fixpoint (second pass reported 0
  updates), unrelated churn in the nine pre-m9-11 manifests was reverted, and the
  `m9-67` / `m9-68` self-rows were restored to their committed values.
- CC#24 / CC#31: this manifest carries the `Base SHA` and `Head SHA` rows.
- CC#36 / CC#55 / CC#39: `verify-report.md` carries `Path` at the head and
  `## Files Inventory`; `verify-findings.json` carries `lens_summary` after
  `subject`; this manifest and `release-report.md` carry `## Cross-checks`.
- CC#12: `main_sha == head_sha == remote_tag_peel` for `v0.7.77`.
- T0: fmt + clippy (`-D warnings`) clean workspace-wide.
- T2: `cargo test -p chronos-mcp --lib` 82 passed; `cargo test -p chronos-store --lib` 74 passed.
- T3: workspace lib minus `chronos-sandbox`/`chronos-native`/`chronos-e2e` all
  green; `cargo test -p chronos-native --lib -- --test-threads=1` 101 passed.
- T4-smoke: `store_open_failure` 2/2, `client_store_isolation` 3/3,
  `e2e_connectivity` 1/1, `session_persistence` 4/4, all `--test-threads=1` with
  `CHRONOS_MCP_PATH` exported.
- Binary-level policy: the same unopenable fixture is observed to fail closed
  without the opt-in and to serve with it, so the acceptance test cannot pass
  because the directory happened to be a valid store.

## Follow-ups (deferred)

- **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** (low): with the opt-in
  set, the degraded mode is logged but never surfaced in a tool response, so an
  opted-in client still cannot tell from a payload that nothing is persisted.
- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low): make the ptrace tests
  serial structurally so the default `cargo test` cannot hang for 17 minutes.
- **FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO** (low, fourth confirmation): land
  `scripts/regen_manifest_index_shas.py` with the skip-self-row rule.
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low, unchanged).
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, unchanged).
- **Sandbox warm-up ordering** and **5+19 not-merged branches triage**: preserved.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-75-fail-closed-store-open/archive-manifest.md` | `0000000000000000000000000000000000000000000000000000000000000000` |
| source (mcp server) | `crates/chronos-mcp/src/server.rs` | `TBD_SHA_SERVER_RS` |
| source (mcp binary) | `crates/chronos-mcp/src/bin/chronos-mcp.rs` | `TBD_SHA_BIN` |
| source (sandbox client tools) | `chronos-sandbox/src/client/tools.rs` | `TBD_SHA_TOOLS` |
| test (store open failure) | `chronos-sandbox/tests/store_open_failure.rs` | `TBD_SHA_TEST` |
| docs (agents manual) | `AGENTS.md` | `TBD_SHA_AGENTS` |
| docs (session management, en) | `docs/manual-ai/en/08-session-management.md` | `TBD_SHA_DOC_EN` |
| docs (session management, es) | `docs/manual-ai/es/08-gestion-sesiones.md` | `TBD_SHA_DOC_ES` |
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-75-fail-closed-store-open/apply-checkpoint.json` | `TBD_SHA_CHECKPOINT` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-75-fail-closed-store-open/verify-report.md` | `TBD_SHA_VERIFY_REPORT` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-75-fail-closed-store-open/verify-findings.json` | `TBD_SHA_VERIFY_FINDINGS` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-75-fail-closed-store-open/release-report.md` | `TBD_SHA_RELEASE_REPORT` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-75-fail-closed-store-open/release-receipt.md` | `TBD_SHA_RELEASE_RECEIPT` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-75-fail-closed-store-open/merge-receipt.md` | `TBD_SHA_MERGE_RECEIPT` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-75-fail-closed-store-open/change-entry.md` | `TBD_SHA_CHANGE_ENTRY` |
| vault index (cycles) | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | `TBD_SHA_CYCLES_INDEX` |
| vault index (terms) | `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | `TBD_SHA_TERMS_INDEX` |
