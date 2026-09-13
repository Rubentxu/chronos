# Change: m9-70 mcp store isolation

## Summary

Closes `FIND-M9-69-MCP-STORE-ISOLATION` (deferred by m9-69). `ChronosServer::new()` opened the developer's real `$HOME/.local/share/chronos/sessions.redb` even under `cfg(test)`, and `SessionStore::list_sessions` hard-failed on the first record it could not deserialize; together they made `chronos-mcp`'s `server::tests::test_list_sessions_after_save` fail deterministically on any machine with a populated store. Three fixes landed, each independently falsified: best-effort listing (skip unreadable records with a warning), absent-table-is-empty reads (also a real fresh-install bug where `session_list` errored on a brand-new database), and a hermetic `cfg(test)` store so the unit suite never reads or writes the developer's database. Production store-opening logic is byte-identical, only relocated behind `#[cfg(not(test))]`.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-70-mcp-store-isolation` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `682817b520010dfa15512fb461a9eeb679009708` |
| Head SHA | `54e859ffe43c09cc2217c8943233f27016bbe76b` |
| Tag | `v0.7.72` |

## Subject

- base_sha: `682817b520010dfa15512fb461a9eeb679009708`
- head_sha: `54e859ffe43c09cc2217c8943233f27016bbe76b`
- cycle: m9-70
- branch: `feat/m9-70-mcp-store-isolation`
- date: 2026-09-13
- tag: `v0.7.72`
- findings_closed: 2 (FIND-M9-69-MCP-STORE-ISOLATION, FIND-M9-70-VIRGIN-STORE-READ-ERROR)
- findings_introduced (deferred): 1 (FIND-M9-70-SERVICES-TABLE-STRING-MATCH)
- new tests: 4

## Files changed

- (modified) `crates/chronos-store/src/storage.rs` — `list_sessions` skips undeserializable records with `tracing::warn!` (+key); `list_sessions` + `session_exists` treat `redb::TableError::TableDoesNotExist` as the empty answer; 3 new tests (+83/−10 total)
- (modified) `crates/chronos-mcp/src/server.rs` — `new()` → `from_store(Self::open_default_store())`; `#[cfg(not(test))]` / `#[cfg(test)]` store impls; `std::path::PathBuf` import replaced by fully-qualified paths (unused in `lib test` builds); 1 new test (+100/−17 total)
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-70-mcp-store-isolation/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-70-mcp-store-isolation/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-70-mcp-store-isolation/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-70-mcp-store-isolation/release-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-70-mcp-store-isolation/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-70-mcp-store-isolation/release-receipt.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-70-mcp-store-isolation/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-70-mcp-store-isolation/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-70 row added, Total cycles 69→70)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (FIND-M9-69-MCP-STORE-ISOLATION terminated; FIND-M9-70-SERVICES-TABLE-STRING-MATCH deferred; Last archive bumped)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-02-events-side-table/archive-manifest.md` (cycles/index.md + terms/index.md SHAs regenerated)

## Cross-checks

CC#1..CC#55: pass (no drift; no new CC — 47 python + 7 bash unchanged, so the smoke test's expected counts need no update). T0 fmt + clippy (`-D warnings`) pass. T2: chronos-store 62 passed, chronos-mcp 77 lib + 49 integration, chronos-services 263. T4-smoke: `e2e_connectivity` 1, `session_persistence` 8, `session_lifecycle` 4 — 13 passed / 0 failed (required because the change is observable through the `session_list` tool surface). CC smoke 5/5 pass.

## Falsification evidence

Each guard was reverted and its test observed to fail: best-effort skip → `test_session_store_list_sessions_skips_unreadable_record` FAILED; absent-table list path → `test_list_sessions_on_virgin_store_is_empty` FAILED; absent-table exists path → `test_session_exists_on_virgin_store_is_false` FAILED; pre-fix `cfg(test)` store simulated → `test_default_test_server_does_not_read_the_developer_store` FAILED with `fresh test server must start with an empty store, found 21 session(s)`. The first draft of the hermeticity test passed pre-fix for the wrong reason (two-server comparison is masked by redb's exclusive lock forcing an in-memory fallback) and was rewritten to assert the observable bug.

## Follow-ups (deferred)

- **FIND-M9-70-SERVICES-TABLE-STRING-MATCH** (low): `chronos-services::sessions::list_sessions` still substring-matches the error text for a missing table; unreachable after the chronos-store fix. Decide delete vs typed match.
- **5+19 not-merged branches triage** (preserved from m9-65): human review needed.
- **Sandbox test warm-up ordering**: `test_session_start_via_v2_then_session_stop_via_v2` fails alone, passes with the full file.
