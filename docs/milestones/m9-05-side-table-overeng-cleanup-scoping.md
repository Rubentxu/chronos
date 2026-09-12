# Scoping — m9-05-side-table-overeng-cleanup

## Goal

Close the four m9-04 debt findings targeted at `apply`:

1. `overeng-001-v3-chunk-decode-dup` — v3 chunk decode ladder duplicated in `load_counterexample_bundle_events` and `count_counterexample_bundle_events`.
2. `overeng-002-v3-range-scan-verify-dup` — v3 range-scan + identity-verify ladder duplicated in `collect_bundle_chunks_range` and `save_bundle_record_and_events`.
3. `overeng-003-events-count-none-branch` — `collect_bundle_chunks(events_count: Option<u64>)` and `get_bundle_events_count` Option wrapper; no caller exercises None post-remediation.
4. `cc-003-wrong-direction-visibility` — `storage.rs::db()` widened `pub(crate)` → `pub`; `COUNTEREXAMPLE_BUNDLES` and `COUNTEREXAMPLE_BUNDLE_EVENTS` widened to `pub const` to support the new CLI integration test.

## Scope

| Concern | Decision |
|---|---|
| Scope boundary | `crates/chronos-store/src/counterexample_storage.rs` + `crates/chronos-store/src/storage.rs`. Touch `crates/chronos-cli/tests/replay_integration.rs` only as needed (visibility narrows must not break tests). |
| Public wire changes | None. No new MCP tools, no schema_version bump, no on-disk format change. |
| Architectural fork | None. File-local refactor. |
| Path | **B-direct** (prescribed remediations from debt-verify envelopes; ~50 LoC reducible). |
| Tiers required | T0 + T1 + T2 (per-crate integration of chronos-store + chronos-cli; chronos-services round-trip already covered by m9-04 tests). |
| Sandbox smoke | Not warranted. No MCP/probe plumbing touched. |

## Remediations

### R1 — Extract `decode_chunk_payload(bytes: &[u8]) -> Option<Vec<TraceEvent>>`

**Resolves:** `overeng-001-v3-chunk-decode-dup`

- New private free function in `counterexample_storage.rs` next to `decode_chunk_value`.
- Signature: `fn decode_chunk_payload(bytes: &[u8]) -> Option<Vec<TraceEvent>>`.
- Body: try `decode_chunk_value` (v3) → take events; on `None`, try `bincode::deserialize::<Vec<TraceEvent>>` (v2 legacy).
- Replace both inline ladders in `load_counterexample_bundle_events` (around lines 663-673) and `count_counterexample_bundle_events` (around lines 700-710).
- LoC reduction: ~10 LoC across two call sites.

### R2 — Extract `collect_v3_keys_for_bundle(tx, bundle_id) -> Result<Vec<Vec<u8>>, StoreError>`

**Resolves:** `overeng-002-v3-range-scan-verify-dup`

- New private free function in `counterexample_storage.rs`.
- Signature: `fn collect_v3_keys_for_bundle(tx: &redb::ReadTransaction, bundle_id: &str) -> Result<Vec<Vec<u8>>, StoreError>`.
- Body: open `COUNTEREXAMPLE_BUNDLE_EVENTS` table, build the `[prefix||0, prefix||u32::MAX)` range, iterate, decode each entry via `decode_chunk_key` + `decode_chunk_value`, verify `value_bundle_id == bundle_id`, and collect the raw key bytes. This is the **same logic** as `collect_bundle_chunks_range` minus the `(chunk_idx, value)` pair accumulation.
- Replace the inline block in `save_bundle_record_and_events` (around lines 551-571) with a single call.
- LoC reduction: ~22 LoC in `save_bundle_record_and_events`.

### R3 — Drop `Option<u64>` wrapper from `collect_bundle_chunks`

**Resolves:** `overeng-003-events-count-none-branch`

- Change `collect_bundle_chunks(events_count: Option<u64>)` to `collect_bundle_chunks(events_count: u64)`.
- Inline the guard: `if events_count == 0 { return Ok(Vec::new()); }` after the v3 range scan, **only when the v3 scan returns empty** (same D7 semantics).
- Change `get_bundle_events_count` to return `u64` directly (not `Option<u64>`):
  - If bundle record exists: return `summary.events_count`.
  - If table does not exist: return 0 (caller treats 0 as "no record, skip fallback" — same effect as the current None branch; no caller actually relies on the None semantics post-m9-04 remediation).
  - If bundle record not found for `bundle_id`: return 0.
- Drop the `Some/None` plumbing; the `m9_04_replay_v2_bundle_uses_legacy_path` test already injects a bundle record with `events_count = 1`, so the Some branch is exercised.
- LoC reduction: ~12 LoC across the function signatures + guard branches.

### R4 — Narrow `db()` visibility and table constants back to `pub(crate)`

**Resolves:** `cc-003-wrong-direction-visibility`

- Revert `storage.rs::db()` to `pub(crate)`.
- Revert `COUNTEREXAMPLE_BUNDLES` and `COUNTEREXAMPLE_BUNDLE_EVENTS` to module-private consts (drop the `pub`).
- Introduce narrow, typed **test chokepoints** in `chronos-store` (behind `#[cfg(any(test, feature = "test-support"))]` or a `#[doc(hidden)] pub` with explicit naming):
  - `pub fn write_v2_chunk_for_test(db: &redb::Database, bundle_id: &str, chunk_idx: u32, events: &[TraceEvent]) -> Result<(), StoreError>` — single-call v2 chunk injection that the cli integration test needs.
  - `pub fn count_v3_chunks_for_test(db: &redb::Database, bundle_id: &str) -> Result<usize, StoreError>` — single-call v3 chunk count that the cli integration test needs.
  - These two functions replace the raw `db().begin_write()` + `open_table(COUNTEREXAMPLE_BUNDLE_EVENTS)` + `insert` dance in `replay_integration.rs` (lines 156-165) and the raw `db().begin_read()` + `collect_bundle_chunks_range` call (lines 217-220).
- The `replay_integration.rs` test file is updated to use the new chokepoints instead of raw table access.
- The doc comment on `db()` is corrected to match the actual visibility ("crate-internal access").

## Out of scope

- `cc-001-god-module` (counterexample_storage.rs at 2556 lines, 5 concerns) — backlog. Splitting keys/records/persistence into separate modules requires a design pass and a backward-compat shim; not B-direct.
- `cc-004-implicit-io-toctou` (read-then-write window in `save_bundle_record_and_events`) — backlog. Pre-existing pattern from m9-02 R7. A workspace-internal mutex would require a concurrency design pass.
- `cc-002-env-coupling-test` — terminated (no-action).
- m9-02 R1-R8, m9-01 R1-R4, m8-06 R4, m8-04 R-hypothesis-fallback — untouched.

## Tests

The m9-04 tests already pin the production semantics; this cycle only changes internal implementation:

- T1 unit tests: `cargo test -p chronos-store --lib` — must remain green (56/56 m9-04 tests).
- T2 per-crate integration:
  - `cargo test -p chronos-store --tests` (matches m9-04 verification).
  - `cargo test -p chronos-cli --tests` — `m9_04_replay_uses_v3_layout` and `m9_04_replay_v2_bundle_uses_legacy_path` rewritten to use new test chokepoints.
- Services round-trip (`m9_04_save_load_roundtrip_through_services`) — exercises the public surface that does not change; should remain green.

## Risk

| Risk | Mitigation |
|---|---|
| R3 signature change ripples through call sites | Both production callers (`load_counterexample_bundle_events`, `count_counterexample_bundle_events`) and the cli integration test paths go through `collect_bundle_chunks`. After R3 the test injects a bundle record with `events_count = 1`, so the `events_count > 0` branch is exercised. |
| R4 visibility regression in cli test | New test chokepoints must provide equivalent test surface. Integration test rewrites preserve the same observable behavior (`events_in_bundle == 1` for v2 fallback, `events_in_bundle == 5` for v3). |
| Refactor breaks m9-04 spec scenarios | The 9 spec scenarios in m9-04's verify-report must remain pinned by the same tests, possibly rewritten to use new chokepoints. |

## Acceptance

- T0 lint clean: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`.
- T1 unit tests green: `cargo test -p chronos-store --lib`.
- T2 per-crate integration green: `cargo test -p chronos-store -p chronos-cli --tests`.
- Diff stats: ≤2 changed crates (`chronos-store`, `chronos-cli/test`); net LoC reduction ~50 lines (R1 + R2 + R3 estimates).
- 4 findings closed in `terms/index.md`: overeng-001, overeng-002, overeng-003, cc-003.

## Out of band

- No spec/design doc needed (B-direct skips `sddk-spec` and `sddk-design`).
- No explore needed (prescribed remediations from debt envelopes).
- No swarm spawn for apply (bounded, file-local refactor; the orchestrator executes directly).