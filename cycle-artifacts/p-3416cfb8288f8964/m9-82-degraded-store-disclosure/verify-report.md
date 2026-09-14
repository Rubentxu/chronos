# Verify Report — m9-82-degraded-store-disclosure

## Subject

| Field | Value |
|---|---|
| Cycle | `m9-82-degraded-store-disclosure` |
| Path | A-min |
| Head SHA | `4e805174fb872e9f4fa0d1ef9d379ba454091994` |
| Base SHA | `a0f72c2a7fe36eaeb9c772505dfe563f85f42773` |
| Tag (when released) | `v0.7.84` (planned; pending release phase) |
| Merge SHA | pending release phase |

> **Cycle**: `p-3416cfb8288f8964/m9-82-degraded-store-disclosure`
> **Path**: A-min
> **Head SHA**: `4e80517`
> **Base SHA**: `a0f72c2a7fe36eaeb9c772505dfe563f85f42773`
> **Date**: 2026-09-14

## Summary

m9-82 closes FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE by
adding three layers of disclosure:

1. **Source layer (`chronos-store`)**: a `StoreKind` enum
   (`Persistent | InMemory`) on `SessionStore`, set by every constructor,
   exposed through a new `pub fn is_persistent(&self) -> bool`.
2. **Server layer (`chronos-mcp`)**: a `degraded: bool` field on
   `ChronosServer`, computed once in `from_store` from the store's
   `is_persistent()` result, exposed through a new
   `pub fn is_degraded(&self) -> bool`.
3. **Wire layer (MCP tool envelopes)**: a new `session_envelope(degraded,
   value)` helper that injects a top-level `"degraded": <bool>` key into
   the JSON object. The five session-persistence tools
   (`save_session`, `list_sessions`, `load_session`, `delete_session`,
   `drop_session`) all wrap their success envelopes through the helper,
   so the flag reaches MCP callers without changing any existing field.

Spec correction: the proposal and earlier spec draft named the tools
with underscores-as-separators (`session_save`/`session_list`/
`session_load`). The actual MCP tool names registered in `server.rs`
are `save_session`, `list_sessions`, `load_session`, `delete_session`,
`drop_session`. Spec REQ-M9-82-03 and the proposal scope/acceptance
bullets were amended mid-cycle with a note documenting the correction
and extending the contract to all five tools symmetrically.

## Verdict: PASS

All five spec REQs (`REQ-M9-82-01` through `REQ-M9-82-05`) PASS. T0,
T2, focused T3, and T4-smoke all green. No regressions in the
`chronos-store`, `chronos-mcp`, or `chronos-services` test suites.

## Files Inventory (CC#55)

| Path | Change | Notes |
|---|---|---|
| `crates/chronos-store/src/storage.rs` | modified | Adds `StoreKind` enum, `kind` field on `SessionStore`, `is_persistent()` accessor; sets `kind` in all three constructors (`open`, `try_open`, `in_memory`) and the inline `SessionStore { ... }` literal in tests. 3 new unit tests cover each constructor. |
| `crates/chronos-mcp/src/server.rs` | modified | Adds `degraded: bool` field on `ChronosServer`, sets it in `from_store`, exposes `pub fn is_degraded()`. Adds `session_envelope(degraded, value)` helper. Wraps 5 tool envelopes (save_session, list_sessions, load_session, delete_session, drop_session both branches). Adds 5 unit tests covering helper shape, defensive wrap, list_sessions envelope, save_session envelope, and the test-server accessor. |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/spec.md` | modified | Mid-impl correction: REQ-M9-82-03 expanded to the actual tool names; spec note added. |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-82-degraded-store-disclosure/proposal.md` | modified | Mid-impl correction: scope and acceptance bullets updated to the actual tool names. |
| `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | modified (pending T3) | m9-82 OPEN row appended at T0; will flip to CLOSED at T3. |
| `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | modified (pending T3) | Last updated bump; FIND-M9-75 closure recorded at T3. |

## Title and summary (CC#30A-D)

- **Title (CC#30A)**: `m9-82 MCP tools disclose degraded (in-memory) store mode`. Matches the cycle ID and intent.
- **Verdict (CC#30B)**: PASS.
- **Cycle (CC#30C)**: `m9-82-degraded-store-disclosure`. Matches the cycle record and the apply-checkpoint.json `cycle_id`.
- **Summary (CC#30D)**: Adds a `StoreKind` enum + `is_persistent()` on `SessionStore`; adds `is_degraded()` on `ChronosServer`; wires a top-level `degraded: <bool>` into the JSON envelopes of save_session, list_sessions, load_session, delete_session, drop_session via a `session_envelope()` helper. T0 + clippy clean; 5 new tests in chronos-mcp (87/0) and 3 new tests in chronos-store (77/0); downstream chronos-services 264/0; T4-smoke 13/0 (e2e_connectivity 1 + session_persistence 4 + session_lifecycle 8). FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE closes.

## Path (CC#32)

`A-min` (1-3 crates, no architectural fork). Tier required: `T2`. Tiers run: `T0` + `T2` + focused `T3` + `T4-smoke`.

## Lens summary (CC#36)

- **Behaviour lens**: PASS. `chronos-store` 74→77 (+3, all new is_persistent tests). `chronos-mcp` 82→87 (+5, all new degraded-envelope tests). `chronos-services` 264/264 unchanged. Sandbox smoke 13/0.
- **Code-quality lens**: PASS. Net +221 / -18 across the two source files. The helper is small and self-contained; the per-tool wrapper is a 2-line `&session_envelope(...)` substitution. Spec correction committed in the same T2 commit because the change is a clarification of intent, not a behaviour change.
- **Architectural lens**: PASS. The change crosses two crates but preserves the layering: `chronos-store` exposes the source-of-truth; `chronos-mcp` consumes it via a single accessor and exposes a wire-level flag. No new module, no new dependency edge, no schema change, no behaviour change for the healthy (persistent) path. The `degraded: false` shape for healthy runs is identical to a no-op for clients that ignore unknown fields.

## Tier results

| Tier | Command | Result | Wall time |
|---|---|---|---|
| T0 | `cargo fmt --all -- --check` (after `cargo fmt --all` applied) | PASS | 1.1s |
| T0 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS | 11.0s |
| T2 | `cargo test -p chronos-store --lib --no-fail-fast` | 77 / 0 / 0 | 1.0s |
| T2 | `cargo test -p chronos-mcp --lib --no-fail-fast` | 87 / 0 / 0 | 1.0s |
| T2 | `cargo test -p chronos-services --lib --no-fail-fast` | 264 / 0 / 0 | 0.5s |
| T3 (focused) | `cargo test -p chronos-store -p chronos-mcp -p chronos-services --tests --no-fail-fast` | 477 / 0 / 0 across 10 test binaries | 28s |
| T4-smoke | `cargo build --bin chronos-mcp && CHRONOS_MCP_PATH=/var/home/rubentxu/cargo-targets/debug/chronos-mcp cargo test -p chronos-sandbox --test e2e_connectivity --test session_persistence --test session_lifecycle --no-fail-fast -- --test-threads=1` | 13 / 0 / 0 (e2e_connectivity 1 + session_persistence 4 + session_lifecycle 8) | 95s |

## Cross-checks (CCs touched)

- **CC#9** (Head SHA 40-char): PASS — head `4e805174fb872e9f4fa0d1ef9d379ba454091994` is full 40 chars; base `a0f72c2a7fe36eaeb9c772505dfe563f85f42773` is full 40 chars.
- **CC#28** (base_sha/head_sha fields): PASS — both fields populated in apply-checkpoint.json; cycle record matches.
- **CC#32** (Path field): PASS — `A-min` recorded in apply-checkpoint.json and verify-report.
- **CC#34** (verify-report Subject, Summary, Cross-checks, Files Inventory): PASS — sections present.
- **CC#42** (peel format): N/A this cycle (peel relevant only at release tag).
- **CC#49** (Base SHA 40-char): PASS — `a0f72c2a7fe36eaeb9c772505dfe563f85f42773` is full 40 chars.
- **CC#51** (cycle-artifacts folder): PASS — folder `cycle-artifacts/p-3416cfb8288f8964/m9-82-degraded-store-disclosure/` exists.
- **CC#55** (Files Inventory): PASS — table above.

## Findings

| ID | Severity | Description | Mitigation | Status |
|---|---|---|---|---|
| F1 | info | SessionStore::is_persistent() correctly reports `false` for `in_memory()`, `true` for `open()` and `try_open()`. Verified by 3 unit tests. | Verified. | CLOSED |
| F2 | info | ChronosServer::is_degraded() correctly reports `true` for a test server (built from in_memory under cfg(test)). Verified by 1 unit test. | Verified. | CLOSED |
| F3 | info | session_envelope helper preserves the existing object shape and only adds the `degraded` key. Defensive wrap covers non-object inputs. Verified by 2 unit tests. | Verified. | CLOSED |
| F4 | info | Real-tool envelope assertions: list_sessions and save_session both carry `degraded: true` at the top level when the server is in-memory, and existing fields are preserved unchanged. Verified by 2 round-trip tests. | Verified. | CLOSED |
| F5 | info | T0 + workspace clippy clean. cargo fmt applied. | Verified. | CLOSED |
| F6 | info | T4-smoke (chronos-sandbox subset) green: e2e_connectivity 1/0, session_persistence 4/0, session_lifecycle 8/0. CHRONOS_MCP_PATH set explicitly so the stale-binary symptom flagged in AGENTS.md §1 cannot mask regressions. | Verified. | CLOSED |
| F7 | info | Spec correction: the proposal and earlier spec draft named the session-persistence tools with underscores-as-separators; the actual tool names in server.rs are save_session, list_sessions, load_session, delete_session, drop_session. Spec REQ-M9-82-03 and the proposal scope/acceptance bullets were amended mid-cycle with a note documenting the correction. | Resolved. | CLOSED |

See `verify-findings.json` for full JSON form.

## Recommendations

- Merge cycle branch into `main` with `--no-ff`.
- Tag `v0.7.84` on the merge commit.
- Delete cycle branch after release.
- Append "Findings closed in m9-82" entry to `terms/index.md` listing FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE.
