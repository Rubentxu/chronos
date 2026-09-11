# Delta for ChronosBundleStorage — Side-table key layout v3 (range-scan friendly)

**Cycle**: `p-3416cfb8288f8964/m9-04-side-table-key-layout`
**Path**: A-min | **Base**: `eb2cccd` (m9-03 close) | **Date**: 2026-09-11
**Precedence**: `docs/milestones/m9-04-side-table-key-layout-scoping.md` §3 D1–D8 + R1–R7.

---

## ADDED Requirements

### Requirement: Side-table keys use a fixed-width blake3 prefix (v3 layout)

`encode_chunk_key` MUST produce `[blake3(bundle_id)[..16] || chunk_index.to_be_bytes()]`, exactly 20 bytes per key. `decode_chunk_key` MUST reject any key whose length is not 20 (`None`). Fixed width is what makes redb's lexicographic range scan bound to one bundle.

#### Scenario: Round-trip + key-shape invariants

- GIVEN a bundle `b` with 257 events (2 chunks)
- WHEN `save_counterexample_bundle(record_for_b)` runs
- THEN `load_counterexample_bundle_events("b")` returns the 257 events in order
- AND the side table contains 2 keys of exactly 20 bytes each, both starting with `blake3("b").as_bytes()[..16]`

---

### Requirement: Side-table values carry `bundle_id` for identity verification

`encode_chunk_value` MUST persist `bincode((bundle_id: String, Vec<TraceEvent>))`. The reader MUST drop any v3 chunk whose decoded `bundle_id` differs from the queried `bundle_id` (D8 / R2 collision defense).

#### Scenario: Value embeds bundle_id

- GIVEN a bundle `b_evt` saved via the API
- WHEN a side-table value is read and decoded
- THEN `bincode::deserialize::<(String, Vec<TraceEvent>)>` succeeds and the String equals `"b_evt"`

---

### Requirement: Read path does a v3 range scan first, falling back to v2

`load_counterexample_bundle_events(bundle_id)` MUST compute the 16-byte prefix, iterate `table.range(prefix..prefix || 0xFF 0xFF 0xFF 0xFF)`, and decode each value's `(bundle_id, chunk)`. If zero chunks match, the reader MUST try the v2 fallback (`collect_bundle_chunks_legacy`).

#### Scenario: v3 / v2 / collision / fresh store

- GIVEN a v3 bundle `b3` and a v2 bundle `b2` written directly with v2 keys
- WHEN `load_counterexample_bundle_events("b3")` and `load_counterexample_bundle_events("b2")` run
- THEN both return the correct events; `b3` iteration count equals its chunk count
- AND two bundles forced to share a blake3[..16] prefix return ONLY their own chunks
- AND on a fresh store, `load_counterexample_bundle_events("ghost")` returns `Ok(vec![])` without scanning legacy keys

---

### Requirement: Save writes v3 chunks and removes both v3 and v2 prior chunks

`save_bundle_record_and_events` MUST, inside ONE write transaction: collect prior v3 keys (range scan) and prior v2 keys (`collect_bundle_chunks_legacy`); remove both sets; write new chunks under v3 keys; commit. A re-save of a v2 bundle leaves only v3 chunks (D6 / R3 lazy migration).

#### Scenario: Re-save migrates v2 → v3

- GIVEN a bundle persisted only in v2 layout
- WHEN `save_counterexample_bundle(record)` runs for the same bundle_id
- THEN `load_counterexample_bundle_events` returns the freshly saved events
- AND no row whose key decodes via `decode_chunk_key_legacy` to that bundle_id remains

---

### Requirement: `CURRENT_BUNDLE_SCHEMA_VERSION` is bumped to 3

`CURRENT_BUNDLE_SCHEMA_VERSION` MUST equal `3`. `KNOWN_BUNDLE_SCHEMA_VERSIONS` MUST equal `[1, 2, 3]`. Every save MUST write `schema_version = 3` on both `record` and `record.summary`. The m9-01 D3 hard-reject on `record.schema_version > CURRENT` is preserved unchanged.

#### Scenario: Saved v3 + future-version rejection

- GIVEN a fresh in-memory store
- WHEN a bundle is saved via the API
- THEN `loaded.schema_version == 3` AND `loaded.summary.schema_version == 3`
- AND a hand-injected blob with `schema_version == 4` makes `load_counterexample_bundle(id)` return an error mentioning "newer than supported"

---

### Requirement: Consumer chokepoints remain signature-stable

`save_counterexample_bundle`, `load_counterexample_bundle_events`, `count_counterexample_bundle_events`, `bundle_events_or_legacy`, `bundle_events_count_or_legacy` MUST keep their signatures. `chronos-services`, `chronos-cli`, and `chronos-mcp` MUST compile and pass tests without source changes outside `tests/`.

#### Scenario: Services + replay chokepoint under v3 + v2

- GIVEN a bundle saved via `ChronosCounterexampleService::save` with N events
- WHEN the same service issues `events_count` and `bundle_events_or_legacy`
- THEN `events_count` returns N (O(1) via summary)
- AND `bundle_events_or_legacy` returns the N events via the v3 range scan

- GIVEN a v2 bundle persisted on disk
- WHEN `chronos_cli::replay::run_replay` runs for that bundle
- THEN the verdict is computed (events flow through the v2 fallback inside the chokepoint)

---

## MODIFIED Requirements

None. m9-04 layers new key-layout behaviour on the m9-02 side-table contract; m9-01 / m9-02 semantics stay.

## REMOVED Requirements

None.
