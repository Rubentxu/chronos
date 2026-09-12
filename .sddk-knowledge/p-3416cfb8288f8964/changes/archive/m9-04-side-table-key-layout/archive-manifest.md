# Archive Manifest — m9-04-side-table-key-layout

## Identidad del ciclo

| Campo | Valor |
|---|---|
| Cycle | `m9-04-side-table-key-layout` |
| Change name | `m9-04-side-table-key-layout` |
| Path | `A-min` |
| Status | **CLOSED** |
| Published SHA | `d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc` |
| Head SHA | `d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc` |
| Tag | `v0.7.2` (annotated, peel matches HEAD) |
| Base SHA | `eb2cccd6fdfcd4b00bf980453bc40c606fa69885` |
| Delivery kind | local (no CLI storage; orchestrator owns transition) |

## Evidence bindings

### Release receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/release-receipt.md
sha256: b870428a2a990a1b80c921ebff38b70aff5e1e4a9339390849ba7d4694fa8d4a
verified_at: 2026-09-12T08:21:00Z
remote_tag_peel: d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc
main_sha: d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc
peel_match: true
pipeline_state: { release: complete, archive: closed }
```

### Merge receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/merge-receipt.md
sha256: 2a5652e6e1ad40d21dc49224ef3270369dac1604bdca41409595d68eb87f5b0a
git_effect: git.push → d6b3b8c... (direct push to origin/main)
sha_match: true
```

### Verify evidence

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/verify-report.md
sha256: 9786a2b530156f5bd9961318ed8e70c3e021aeab05150fd8f6f890259419151c
status: PASS
mode: coordinator (verify-round, second round — remediation of 6 prior findings)
scenarios: 9 (6 in store + 1 in services + 2 in cli replay)
results: 9/9 COMPLIANT
```

### Release report

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/release-report.md
sha256: pending (computed after write)
verdict: success
git_effects: all PASS
verification_summary: PASS, 9/9 scenarios COMPLIANT
```

## Vault sync

### Findings resolved by m9-04

One m9-02 debt finding is closed by this cycle:

| ID | Cluster | Severity | Priority | Título | Evidence |
|---|---|---|---|---|---|
| FIND-M9-02-DV-PERF-01 | perf | MEDIUM | P2 | Side-table key layout forces full-scan for single-bundle reads | `crates/chronos-store/src/counterexample_storage.rs` v3 encoding (`encode_chunk_key` + `collect_bundle_chunks_range` + `bundle_prefix`) + 13 m9_04 store unit tests + 2 cli integration tests + 1 services round-trip test |

**Closed by:** `m9-04-side-table-key-layout` · SHA `d6b3b8c...` · tag `v0.7.2`

### Findings introduced by m9-04 (debt-verify round)

| ID | Cluster | Severity | Priority | Target | Título | Owner | Destino |
|---|---|---|---|---|---|---|---|
| overeng-001-v3-chunk-decode-dup | overeng | MEDIUM | P2 | apply | v3 chunk decode ladder duplicated in `load_counterexample_bundle_events` and `count_counterexample_bundle_events` | unassigned | m9-05 |
| overeng-002-v3-range-scan-verify-dup | overeng | LOW | P3 | apply | v3 range-scan + identity-verify ladder duplicated in `collect_bundle_chunks_range` and `save_bundle_record_and_events` | unassigned | m9-05 |
| overeng-003-events-count-none-branch | overeng | LOW | P3 | apply | `collect_bundle_chunks(events_count: Option<u64>)` and `get_bundle_events_count` Option wrapper; no caller exercises None post-remediation | unassigned | m9-05 |
| cc-003-wrong-direction-visibility | coupling | MEDIUM | P2 | apply | `storage.rs::db()` widened `pub(crate)` → `pub`; `COUNTEREXAMPLE_BUNDLES` and `COUNTEREXAMPLE_BUNDLE_EVENTS` widened to `pub const` | unassigned | m9-05 |
| cc-001-god-module | coupling | MEDIUM | P2 | backlog | `counterexample_storage.rs` at 2,556 lines; 5 distinct concerns (keys/records/persistence/schema-version/v2-legacy) | unassigned | m9+ backlog |
| cc-004-implicit-io-toctou | coupling | LOW | P3 | backlog | `save_bundle_record_and_events` opens read-then-write; pre-existing TOCTOU pattern (m9-02 R7) | unassigned | m9+ backlog |
| cc-002-env-coupling-test | coupling | LOW | P3 | none | `replay_integration::temp_db_path` reads `std::env::temp_dir()` (test-only, no domain-layer coupling) | — | terminated (no-action) |

### Findings NOT resolved (m9+ backlog inherited)

| ID | Cluster | Severity | Título | Reason |
|---|---|---|---|---|
| m9-02 R1–R8 | various | — | See `changes/m9-02-events-side-table/change-entry.md` | Deferred from m9-02 scoping |
| m9-01 R1–R4 | various | — | See `changes/m9-01-schema-versioning/change-entry.md` | Deferred from m9-01 scoping |
| m8-06 R4 | various | — | Cross-variant existence predicate shrinking | Deferred from m8-06 |
| m8-04-R-hypothesis-fallback | various | — | property_target lost in fallback reconstruction | Deferred from m8-04 |

## Knowledge nodes updated

| Node kind | Path |
|---|---|
| Cycle entry (m9-04) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-04-side-table-key-layout/` |
| Terms (m9+) | `.sddk-knowledge/p-3416cfb8288f8964/terms/` (1 finding closed, 4 apply, 2 backlog, 1 terminated) |
| Cycles index | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` |
| Archive manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-04-side-table-key-layout/archive-manifest.md` |

## Ledger state (pre-archive)

```
note: CLI ledger not available (sddk not in PATH). Ad-hoc cycle, no CLI storage.
      Runtime status RELEASED recorded in release receipt.
```

## Specs synced

- **R4 disclosure pin**: `m9_04_r4_v2_bundle_never_resaved_uses_legacy_path` test asserts that a v2 bundle that is never resaved continues to read through `collect_bundle_chunks_legacy`.
- **R6 disclosure pin**: `m9_04_known_bundle_schema_versions_pinned` test asserts `KNOWN_BUNDLE_SCHEMA_VERSIONS == [1, 2, 3]`.
- **m9-01 R2 hardening**: future-versioned bundles (schema_version > 3) rejected at load; pin asserted by `m9_04_known_bundle_schema_versions_pinned`.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-04-side-table-key-layout/archive-manifest.md` | `2d2d189142773d02a738f94106d1783c87dbfde6bf95f80892d75c59592a4d3d` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-04-side-table-key-layout/change-entry.md` | `a6e3ae78435674a2f1694e9d3ff6267d74d803badadde09adffc6f52d06e964d` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/merge-receipt.md` | `2a5652e6e1ad40d21dc49224ef3270369dac1604bdca41409595d68eb87f5b0a` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/release-receipt.md` | `b870428a2a990a1b80c921ebff38b70aff5e1e4a9339390849ba7d4694fa8d4a` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/release-report.md` | `e4f13e25f6ddf93bc8a61473aebfde449b8578e5d652d0c35bc208aa72e8be61` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/verify-report.md` | `9786a2b530156f5bd9961318ed8e70c3e021aeab05150fd8f6f890259419151c` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/verify-findings.json` | `fd8de7a8416f184cab8389f6a36db2bca375d63b7cc8071177bc9fe2b7c87c23` |