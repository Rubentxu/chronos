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

| ID | Cycle | Cluster | Severity | Priority | Título | Owner | Destino |
|---|---|---|---|---|---|---|---|
| FIND-M9-02-DV-PERF-01 | m9-02 | perf | MEDIUM | P2 | Side-table key layout forces full-scan for single-bundle reads | unassigned | m9+ backlog |

### Debt findings from m9-01

| ID | Cycle | Cluster | Severity | Priority | Título | Owner | Destino |
|---|---|---|---|---|---|---|---|
| FIND-M9-01-DV-COUP-01 | m9-01 | coupling | MEDIUM | P2 | Duplicated version envelopes: `schema_version` on record + summary, no loader equality check | unassigned | m9+ backlog |
| FIND-M9-01-DV-COUP-02 | m9-01 | coupling | LOW | P3 | List/load policy asymmetry: hard-reject on load vs best-effort include on list | unassigned | m9+ backlog |
| FIND-M9-01-DV-OE-01 | m9-01 | overeng | LOW | P3 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` dead speculative code | unassigned | m9+ backlog |

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

## Metadata

| Campo | Valor |
|---|---|
| Project | chronos |
| Vault | `.sddk-knowledge/p-3416cfb8288f8964/` |
| Last updated | 2026-09-11T23:17:00Z |
| Last archive | m9-03-side-table-debt-cleanup |
