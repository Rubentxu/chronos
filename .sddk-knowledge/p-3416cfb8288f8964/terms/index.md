# Terms Index — m9+ follow-ups

Terms tracked from released cycles awaiting resolution in milestone m9 or later.

## Active terms

### Disclosures (m9-02 scoping R1–R8)

| ID | Cycle | Título | Severity | Priority | Owner | Destino |
|---|---|---|---|---|---|---|
| m9-02-R1 | m9-02 | `schema_version` bumped to 2; bundles with >2 hard-rejected by loader | — | backlog | unassigned | m9+ |
| m9-02-R2 | m9-02 | Pre-m9-02 bundles get `events_count: 0` via serde default; fallback to `record.events.len()` for count | — | backlog | unassigned | m9+ |
| m9-02-R3 | m9-02 | Public `save_counterexample_bundle_events` is not atomic w.r.t. the record (only the internal wrapper is) | — | backlog | unassigned | m9+ |
| m9-02-R4 | m9-02 | `counterexample_bundle_events` MCP tool deferred to m9+ (m9-02 ships storage primitive only) | — | backlog | unassigned | m9+ |
| m9-02-R5 | m9-02 | No chunk compression (lz4/zstd) — chunk size 256 is acceptable for typical bundles | — | backlog | unassigned | m9+ |
| m9-02-R6 | m9-02 | `list_counterexample_bundles` still deserializes full blob per row (`events: vec![]` is the optimisation) | — | backlog | unassigned | m9+ |
| m9-02-R7 | m9-02 | Re-save overwrites all prior chunks (no append semantics) | — | backlog | unassigned | m9+ |
| m9-02-R8 | m9-02 | Per-chunk load not publicly exposed (concat-then-paginate is fast enough for realistic bundle sizes) | — | backlog | unassigned | m9+ |

### Disclosures (m9-01 scoping R1–R4)

| ID | Cycle | Título | Severity | Priority | Owner | Destino |
|---|---|---|---|---|---|---|
| m9-01-R1 | m9-01 | `schema_version` silently overwritten on save (D5 canonical writer) | — | backlog | unassigned | m9+ |
| m9-01-R2 | m9-01 | Future-versioned bundles appear in list (best-effort; load() rejects individually) | — | backlog | unassigned | m9+ |
| m9-01-R3 | m9-01 | `schema_version` is internal-only (not on MCP wire or services-side summary) | — | backlog | unassigned | m9+ |
| m9-01-R4 | m9-01 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` unused; `#[allow(dead_code)]` | — | backlog | unassigned | m9+ |

### Debt findings from m9-02

(empty — last item closed by m9-04, see Terminated terms below)

### Debt findings from m9-04

| ID | Cycle | Cluster | Severity | Priority | Título | Owner | Destino |
|---|---|---|---|---|---|---|---|
| cc-001-god-module | m9-04 | coupling | MEDIUM | P2 | `counterexample_storage.rs` at 2,556 lines; 5 distinct concerns | unassigned | m9+ backlog |
| cc-004-implicit-io-toctou | m9-04 | coupling | LOW | P3 | `save_bundle_record_and_events` opens read-then-write; pre-existing TOCTOU pattern (m9-02 R7) | unassigned | m9+ backlog |

### Debt findings from m9-01

(empty — both FIND-M9-01-DV-COUP-01 and FIND-M9-01-DV-COUP-02 terminated by m9-07/m9-08; see Terminated terms below)

### Follow-ups inherited from prior cycles

| ID | Cycle | Título | Owner | Destino |
|---|---|---|---|---|
| m8-06-R4 | m8-06 | Cross-variant existence predicate shrinking: variant still fixed in ExistencePredicateShrinker | unassigned | m9+ |
| m8-04-R-hypothesis-fallback | m8-04 | property_target lost in fallback reconstruction | unassigned | m9+ (non-issue post m8-07 but synthetic defaults remain for pre-m8-07 bundles) |

## Terminated terms

| ID | Cycle | Título | Closed by |
|---|---|---|---|
| m8-04-R4 | m8-04 | bundle-as-blob → side table: events still ride inside bundle blob | m9-02-events-side-table (`v0.7.0`) |
| m8-07-R2 | m8-07 | Unknown future wire fields are dropped | m9-01-schema-versioning (`v0.6.0`) |
| FIND-M9-02-DV-API-01 | m9-02 | `save_counterexample_bundle_events` dead API: no caller uses it standalone | m9-03-side-table-debt-cleanup (`v0.7.1`) |
| FIND-M9-02-DV-DOC-01 | m9-02 | `events_count` doc drift: fallback branch not documented at call site | m9-03-side-table-debt-cleanup (`v0.7.1`) |
| FIND-M9-02-DV-OE-01 | m9-02 | Side-table chunk-iteration skeleton duplicated in 3 places | m9-03-side-table-debt-cleanup (`v0.7.1`) |
| FIND-M9-02-DV-COUP-01 | m9-02 | Fallback outside D5 chokepoint: wrong module boundary | m9-03-side-table-debt-cleanup (`v0.7.1`) |
| FIND-M9-02-DV-PERF-01 | m9-02 | Side-table key layout forces full-scan for single-bundle reads | m9-04-side-table-key-layout (`v0.7.2`) |
| cc-002-env-coupling-test | m9-04 | `replay_integration::temp_db_path` reads `std::env::temp_dir()` (test-only) | m9-04-side-table-key-layout (no-action; test-scope coupling accepted) |
| overeng-001-v3-chunk-decode-dup | m9-04 | v3 chunk decode ladder duplicated in `load_counterexample_bundle_events` and `count_counterexample_bundle_events` | m9-05-side-table-overeng-cleanup (`v0.7.3`) |
| overeng-002-v3-range-scan-verify-dup | m9-04 | v3 range-scan + identity-verify ladder duplicated in `collect_bundle_chunks_range` and `save_bundle_record_and_events` | m9-05-side-table-overeng-cleanup (`v0.7.3`) |
| overeng-003-events-count-none-branch | m9-04 | `collect_bundle_chunks(events_count: Option<u64>)` Option wrapper; no caller exercises None post-remediation | m9-05-side-table-overeng-cleanup (`v0.7.3`) |
| cc-003-wrong-direction-visibility | m9-04 | `storage.rs::db()` widened `pub(crate)` → `pub`; table constants widened to `pub const` | m9-05-side-table-overeng-cleanup (`v0.7.3`) |
| m9-01-R4 | m9-01 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` unused; `#[allow(dead_code)]` | m9-06-known-schema-versions-invariant (`v0.7.4`) |
| FIND-M9-01-DV-OE-01 | m9-01 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` dead speculative code | m9-06-known-schema-versions-invariant (`v0.7.4`) |
| FIND-M9-01-DV-COUP-01 | m9-01 | Duplicated version envelopes: `schema_version` on record + summary, no loader equality check | m9-07-coup-01-invariant-assertion (`v0.7.5`) |
| FIND-M9-01-DV-COUP-02 | m9-01 | List/load policy asymmetry: hard-reject on load vs best-effort include on list | m9-08-list-load-schema-error-variant (`v0.7.6`) |

## Metadata

| Campo | Valor |
|---|---|
| Project | chronos |
| Vault | `.sddk-knowledge/p-3416cfb8288f8964/` |
| Last updated | 2026-09-12T07:05:00Z |
| Last archive | m9-08-list-load-schema-error-variant |
