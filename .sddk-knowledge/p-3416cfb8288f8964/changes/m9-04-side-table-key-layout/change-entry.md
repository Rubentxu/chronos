# Change: m9-04 side table key layout

## Summary

Drift closure cycle for this milestone.


## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-04-side-table-key-layout` |
| Path | `A-min` |
| Status | CLOSED |
| Base SHA | `eb2cccd6fdfcd4b00bf980453bc40c606fa69885` |
| Head SHA | `d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc` |
| Tag | `v0.7.2` (annotated, peel matches published SHA) |

## Commits

| SHA | Subject |
|---|---|
| `083f5ba` | docs(m9-04): scoping — side-table key layout (range-scan friendly, v3 schema) |
| `6ec8757` | docs(m9-04): spec — side-table key layout (range-scan friendly) |
| `140d53a` | docs(m9-04): design — side-table v3 key layout (range-scan friendly) |
| `7f86a1c` | feat(m9-04): side-table key layout v3 (blake3 prefix, 20-byte fixed keys) |
| `379759e` | m9-04 side-table-key-layout: fix verify failures, add CLI replay integration tests |
| `d6b3b8c` | verify(m9-04): PASS — remediation resolved all 6 prior findings |

## Scope

A-min schema-migration cycle targeting **FIND-M9-02-DV-PERF-01** (the last open debt finding from m9-02). Introduces a third schema version of the side-table key layout so single-bundle reads no longer require a full-table scan.

### Changes

- **v3 key encoding** (20-byte fixed): `[16-byte blake3(bundle_id) prefix][4-byte big-endian chunk_idx]`.
- **v3 value encoding**: `[bundle_id length-prefixed UTF-8][bincode(events)]` so each chunk carries its own `bundle_id` for hash-collision containment.
- **v2 fallback**: pre-existing variable-width key layout remains readable via `collect_bundle_chunks_legacy`; legacy bundles that are never resaved continue to use the v2 path (R4 pin).
- **D7 guard** (`events_count: Option<u64>` on `collect_bundle_chunks`): `Some(0)` short-circuits the v2 fallback to prevent resurrection of phantom chunks for bundles explicitly marked empty.
- **Save atomicity**: `save_bundle_record_and_events` removes both v3 and v2 prior chunks in the same write transaction so a resave from v2 → v3 leaves no orphan v2 entries.
- **Future-version rejection**: `CURRENT_BUNDLE_SCHEMA_VERSION = 3`; loads of `schema_version > 3` return `StoreError::UnsupportedBundleSchemaVersion` (m9-01 R2 hardening).
- **CLI `project_report` regression fix**: `replay.rs:243` now reads `bundle.summary.events_count` instead of `bundle.events.len()` (which has been empty since the m9-02 side-table split). Pinned by `m9_04_replay_uses_v3_layout` asserting `report.events_in_bundle == 5`.

### Changed paths

- `crates/chronos-cli/Cargo.toml` — new `dev-dependencies` on `chronos-store` (test-only).
- `crates/chronos-cli/src/lib.rs` — module decl for `replay`.
- `crates/chronos-cli/src/replay.rs` — `project_report` reads `summary.events_count`.
- `crates/chronos-cli/tests/replay_integration.rs` — 2 new integration tests (228 LoC, end-to-end through the public CLI).
- `crates/chronos-services/src/counterexample.rs` — services round-trip test (94 LoC).
- `crates/chronos-store/src/counterexample_storage.rs` — v3 key/value encoding + dual-format reader (1105 LoC added).
- `crates/chronos-store/src/storage.rs` — `SessionStore::db()` visibility widened (`pub(crate)` → `pub`) so external integration tests can drive the chokepoint; doc comment says "minimum visibility" but actual modifier is `pub`. Tracked for m9-05.
- `docs/milestones/m9-04-side-table-key-layout-{scoping,spec,design}.md` — cycle docs.

## Findings resolved

| ID | Cluster | Título | Closed by |
|---|---|---|---|
| FIND-M9-02-DV-PERF-01 | perf | Side-table key layout forces full-scan for single-bundle reads | m9-04-side-table-key-layout (`v0.7.2`) |

## Findings introduced (m9-04 debt-verify)

| ID | Cluster | Severity | Target | Título |
|---|---|---|---|---|
| overeng-001-v3-chunk-decode-dup | overeng | MEDIUM | apply | v3 chunk decode ladder duplicated in `load_counterexample_bundle_events` and `count_counterexample_bundle_events` |
| overeng-002-v3-range-scan-verify-dup | overeng | LOW | apply | v3 range-scan + identity-verify ladder duplicated in `collect_bundle_chunks_range` and `save_bundle_record_and_events` |
| overeng-003-events-count-none-branch | overeng | LOW | apply | `collect_bundle_chunks(events_count: Option<u64>)` and `get_bundle_events_count` Option wrapper; no caller exercises None post-remediation |
| cc-001-god-module | coupling | MEDIUM | backlog | `counterexample_storage.rs` at 2,556 lines; 5 distinct concerns (keys/records/persistence/schema-version/v2-legacy) |
| cc-002-env-coupling-test | coupling | LOW | none | `replay_integration::temp_db_path` reads `std::env::temp_dir()` (test-only) |
| cc-003-wrong-direction-visibility | coupling | MEDIUM | apply | `storage.rs::db()` widened `pub(crate)` → `pub`; `COUNTEREXAMPLE_BUNDLES` and `COUNTEREXAMPLE_BUNDLE_EVENTS` widened to `pub const`. Driven by the new CLI integration test. |
| cc-004-implicit-io-toctou | coupling | LOW | backlog | `save_bundle_record_and_events` opens read-then-write; pre-existing TOCTOU pattern |

Verdict: **PASS** · 1/1 m9-02 finding closed; 4 m9-04 `apply`-target findings scheduled for `m9-05-side-table-overeng-cleanup`; 2 m9-04 backlog findings recorded.

## Artefactos

| Kind | Path |
|---|---|
| Apply checkpoint | n/a (B-direct verify-round; no checkpoint emitted pre-merge) |
| Merge receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/merge-receipt.md` |
| Release receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/release-receipt.md` |
| Release report | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/release-report.md` |
| Verify report | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/verify-report.md` |
| Verify findings | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/verify-findings.json` |
| Archive manifest | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-04-side-table-key-layout/archive-manifest.md` |
| Scoping | `docs/milestones/m9-04-side-table-key-layout-scoping.md` |
| Spec | `docs/milestones/m9-04-side-table-key-layout-spec.md` |
| Design | `docs/milestones/m9-04-side-table-key-layout-design.md` |