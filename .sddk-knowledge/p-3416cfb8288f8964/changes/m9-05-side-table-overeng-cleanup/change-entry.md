# Change: m9-05 side table overeng cleanup

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-05-side-table-overeng-cleanup` |
| Path | `B-direct` |
| Status | CLOSED |
| Base SHA | `c9e541750e39458b2b28c5d19ad0ca11d0e73d5b` |
| Head SHA | `07d01d5869ff6e3ffed29315b761e6e476f3b70d` |
| Tag | `v0.7.3` (annotated, peel matches published SHA) |

## Commits

| SHA | Subject |
|---|---|
| `232e2d2` | docs(m9-05): scoping — side-table overeng cleanup (4 apply-target findings) |
| `fa0ca90` | fix(m9-05): close 4 apply-target debt findings — extract decode_chunk_payload, collect_v3_keys_for_bundle; drop events_count Option wrapper; narrow pub visibility with chokepoints |
| `07d01d5` | docs(m9-05): light-verify evidence (R1-R4 PASS, 4/4 findings closed) |

## Scope

B-direct debt-cleanup closing the four `apply`-target findings introduced by `m9-04`:

1. **overeng-001** — extracted `decode_chunk_payload(bytes: &[u8]) -> Option<Vec<TraceEvent>>` to centralize the v3-first / v2-fallback decode ladder; both `load_counterexample_bundle_events` and `count_counterexample_bundle_events` now call the helper.
2. **overeng-002** — extracted `collect_v3_keys_for_bundle(tx, bundle_id) -> Result<Vec<Vec<u8>>, StoreError>` to centralize the v3 range-scan + identity-verify ladder; `save_bundle_record_and_events` (D6 cleanup block) now reuses the helper.
3. **overeng-003** — dropped the `Option<u64>` wrapper from `collect_bundle_chunks` and `get_bundle_events_count`; `events_count: u64` is now plumbed directly with the D7 short-circuit inlined as `if events_count == 0 { return Ok(Vec::new()); }`. One pre-existing direct-injection lib test (`m9_02_load_events_concatenates_chunks_in_order`) was updated to inject a bundle record with `events_count = 3` so the v2 fallback path is exercised through the public API.
4. **cc-003** — narrowed `storage.rs::db()` back to `pub(crate)` and reverted `COUNTEREXAMPLE_BUNDLES` + `COUNTEREXAMPLE_BUNDLE_EVENTS` to module-private consts. Introduced three `#[doc(hidden)] pub` chokepoints on `SessionStore`: `insert_v2_chunk_for_test`, `count_v3_chunks_for_test`, `insert_bundle_record_for_test`. The cli integration test (`replay_integration.rs`) was rewritten to use these chokepoints instead of raw database/table access.

### Changed paths

- `crates/chronos-cli/tests/replay_integration.rs` — uses chokepoints.
- `crates/chronos-store/src/counterexample_storage.rs` — extracts helpers + drops Option + adds chokepoints.
- `crates/chronos-store/src/storage.rs` — narrows `db()` visibility.

## Findings resolved

| ID | Cluster | Título | Closed by |
|---|---|---|---|
| overeng-001-v3-chunk-decode-dup | overeng | v3 chunk decode ladder duplicated in `load_counterexample_bundle_events` and `count_counterexample_bundle_events` | m9-05-side-table-overeng-cleanup (`v0.7.3`) |
| overeng-002-v3-range-scan-verify-dup | overeng | v3 range-scan + identity-verify ladder duplicated in `collect_bundle_chunks_range` and `save_bundle_record_and_events` | m9-05-side-table-overeng-cleanup (`v0.7.3`) |
| overeng-003-events-count-none-branch | overeng | `collect_bundle_chunks(events_count: Option<u64>)` and `get_bundle_events_count` Option wrapper; no caller exercises None post-remediation | m9-05-side-table-overeng-cleanup (`v0.7.3`) |
| cc-003-wrong-direction-visibility | coupling | `storage.rs::db()` widened `pub(crate)` → `pub`; `COUNTEREXAMPLE_BUNDLES` and `COUNTEREXAMPLE_BUNDLE_EVENTS` widened to `pub const` | m9-05-side-table-overeng-cleanup (`v0.7.3`) |

Verdict: **PASS** (4/4 apply-target findings closed) · 2 m9-04 backlog findings (cc-001 god-module, cc-004 TOCTOU) remain in m9+ backlog.

## Artefactos

| Kind | Path |
|---|---|
| Scoping | `docs/milestones/m9-05-side-table-overeng-cleanup-scoping.md` |
| Merge receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/merge-receipt.md` |
| Release receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/release-receipt.md` |
| Release report | `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/release-report.md` |
| Verify report | `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/verify-report.md` |
| Archive manifest | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-05-side-table-overeng-cleanup/archive-manifest.md` |