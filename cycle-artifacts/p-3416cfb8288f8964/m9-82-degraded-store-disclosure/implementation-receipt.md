# Implementation Receipt: m9-82

> **Cycle**: `p-3416cfb8288f8964/m9-82-degraded-store-disclosure`
> **Path**: A-min
> **Date**: 2026-09-14
> **Head SHA**: `4e805174fb872e9f4fa0d1ef9d379ba454091994`
> **Base SHA**: `a0f72c2a7fe36eaeb9c772505dfe563f85f42773`

## Summary

Closes FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE by adding
disclosure at three layers:

- **`chronos-store`**: `StoreKind` enum, `kind` field on `SessionStore`,
  `pub fn is_persistent(&self) -> bool`.
- **`chronos-mcp`**: `degraded: bool` field on `ChronosServer`, set in
  `from_store`, exposed via `pub fn is_degraded(&self) -> bool`.
- **Wire layer**: `session_envelope(degraded, value)` helper injects a
  top-level `"degraded": <bool>` into the JSON envelope of the five
  session-persistence tools (`save_session`, `list_sessions`,
  `load_session`, `delete_session`, `drop_session`).

The change is wire-shape-additive: every existing field is preserved
unchanged; only one new field is added per envelope.

## Commits

| SHA | Title |
|---|---|
| `5687346` | m9-82: vault (exploration-report + proposal + spec + tasks) |
| `77cd0ab` | feat(store): expose SessionStore kind so MCP can disclose degraded store |
| `477ee83` | feat(mcp): surface degraded store mode in session-persistence tool envelopes |
| `4e80517` | style: cargo fmt pass on m9-82 tool envelope wrapper and helper |

## Source diff (high-level)

### `crates/chronos-store/src/storage.rs`

- New `pub(crate) enum StoreKind { Persistent, InMemory }`.
- New `kind: StoreKind` field on `SessionStore`.
- New `pub fn is_persistent(&self) -> bool` accessor.
- All three constructors set `kind`:
  - `open(p)` → `kind: StoreKind::Persistent` (line 105)
  - `try_open(p)` → `kind: StoreKind::Persistent` (line 127, also 149 for the inner retry path)
  - `in_memory()` → `kind: StoreKind::InMemory` (line 192)
- The inline `SessionStore { ... }` literal in `storage::tests` also
  gets `kind: StoreKind::Persistent` (line 725).
- 3 new unit tests:
  - `test_session_store_is_persistent_after_in_memory` — asserts false.
  - `test_session_store_is_persistent_after_open` — asserts true.
  - `test_session_store_is_persistent_after_try_open` — asserts true.

### `crates/chronos-mcp/src/server.rs`

- New `degraded: bool` field on `ChronosServer`.
- New `pub fn is_degraded(&self) -> bool` accessor.
- `from_store` computes `let degraded = !store.is_persistent();` once
  and stores it in the new field.
- New `fn session_envelope(degraded: bool, value: serde_json::Value) -> serde_json::Value`:
  - Object branch: inserts `"degraded"` key, preserves every other key.
  - Non-object branch: wraps in `{"result": ..., "degraded": ...}` so
    the wire contract stays a JSON object.
- 5 tool envelopes wrapped through the helper:
  - `save_session` (line 2879) — single success arm.
  - `list_sessions` (line 2932) — single success arm.
  - `load_session` (line 2976) — single success arm.
  - `delete_session` (line 3018) — single success arm.
  - `drop_session` (lines 3061 and 3071) — both success arms
    (`existed` / `not_found`).
- 5 new unit tests:
  - `test_server_is_degraded_true_for_in_memory_test_store` — asserts
    the accessor agrees with the in-memory store under cfg(test).
  - `test_session_envelope_injects_degraded_at_top_level` — verifies
    the helper preserves every existing key.
  - `test_session_envelope_wraps_non_object_defensively` — verifies
    the helper's fallback for non-object inputs.
  - `test_list_sessions_envelope_includes_degraded_true` — round-trips
    the list_sessions content through `serde_json::to_value` and
    asserts `degraded: true`.
  - `test_save_session_envelope_includes_degraded` — same shape, on
    save_session; also asserts the `session_id` and `status` fields
    are unchanged.

## Spec correction (mid-impl)

The proposal and earlier spec draft named the session-persistence
tools with underscores-as-separators (`session_save`/`session_list`/
`session_load`). The actual MCP tool names registered in `server.rs`
are `save_session`, `list_sessions`, `load_session`, `delete_session`,
`drop_session`. Spec REQ-M9-82-03 and the proposal scope/acceptance
bullets were amended mid-cycle with a note documenting the correction
and extending the contract to all five tools symmetrically.

## Acceptance against spec

| REQ | Status | Evidence |
|---|---|---|
| REQ-M9-82-01 (`SessionStore` exposes its kind) | PASS | `grep -n 'pub fn is_persistent' crates/chronos-store/src/storage.rs` returns 1 match (line 79). `cargo test -p chronos-store --lib storage::tests::test_session_store_is_persistent` passes (3 tests). |
| REQ-M9-82-02 (`ChronosServer` carries a `degraded` flag) | PASS | `grep -n 'pub fn is_degraded' crates/chronos-mcp/src/server.rs` returns 1 match (line 1479). `let degraded = !store.is_persistent();` in `from_store` at line 1454. |
| REQ-M9-82-03 (tool responses include `degraded` at top level) | PASS | All 5 tool envelopes wrap through `session_envelope(self.degraded, output)`. Round-trip tests `test_list_sessions_envelope_includes_degraded_true` and `test_save_session_envelope_includes_degraded` decode the JSON content and assert the field. |
| REQ-M9-82-04 (wire-shape additivity) | PASS | `cargo test -p chronos-mcp --lib --no-fail-fast` is 87/0 (was 82; +5 new tests). `cargo test -p chronos-store --lib --no-fail-fast` is 77/0 (was 74; +3 new tests). `cargo test -p chronos-services --lib --no-fail-fast` is 264/0 (unchanged). |
| REQ-M9-82-05 (lint/clippy clean) | PASS | `cargo fmt --all -- --check` exits 0 (after `cargo fmt --all` applied). `cargo clippy --workspace --all-targets -- -D warnings` exits 0. |

## T4-smoke (chronos-sandbox subset)

| Suite | Result |
|---|---|
| `e2e_connectivity` | 1 / 0 / 0 (5.6s) |
| `session_persistence` | 4 / 0 / 0 (28s) |
| `session_lifecycle` | 8 / 0 / 0 (60s) |
| **Total** | **13 / 0 / 0** |

CHRONOS_MCP_PATH was set explicitly to the rebuilt binary path so
the stale-binary symptom flagged in AGENTS.md §1 cannot mask
regressions.

## Carry-forward

- **Closed**: `FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE`.
- **New**: none.

## Out-of-scope items (intentionally untouched)

- Surfacing `degraded` through `chronos-cli` (it does not use the MCP
  wire shape today).
- Adding a dedicated "session info" tool; the cycle piggy-backs on
  existing tool responses (per the proposal's "out of scope" list).
- Changing the binary's exit-status policy (already correct per m9-75).
- New tests for the chronos-sandbox integration suite; the change is
  observable through the JSON body shape, and the sandbox suite's
  smoke tests already cover the start/drain/stop and basic persistence
  paths (T4-smoke 13/0).
