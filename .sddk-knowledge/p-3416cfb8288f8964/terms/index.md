# Terms Index — m9+ follow-ups

Terms tracked from released cycles awaiting resolution in milestone m9 or later.

## Active terms

### Disclosures (R1–R4 from m9-01 scoping)

| ID | Cycle | Título | Severity | Priority | Owner | Destino |
|---|---|---|---|---|---|---|
| m9-01-R1 | m9-01 | `schema_version` silently overwritten on save (D5 canonical writer) | — | backlog | unassigned | m9+ |
| m9-01-R2 | m9-01 | Future-versioned bundles appear in list (best-effort; load() rejects individually) | — | backlog | unassigned | m9+ |
| m9-01-R3 | m9-01 | `schema_version` is internal-only (not on MCP wire or services-side summary) | — | backlog | unassigned | m9+ |
| m9-01-R4 | m9-01 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` unused; `#[allow(dead_code)]` | — | backlog | unassigned | m9+ |

### Debt findings from m9-01

| ID | Cycle | Cluster | Severity | Priority | Título | Owner | Destino |
|---|---|---|---|---|---|---|---|
| FIND-M9-01-DV-COUP-01 | m9-01 | coupling | MEDIUM | P2 | Duplicated version envelopes: `schema_version` on record + summary, no loader equality check | unassigned | m9+ backlog |
| FIND-M9-01-DV-COUP-02 | m9-01 | coupling | LOW | P3 | List/load policy asymmetry: hard-reject on load vs best-effort include on list | unassigned | m9+ backlog |
| FIND-M9-01-DV-OE-01 | m9-01 | overeng | LOW | P3 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` dead speculative code | unassigned | m9+ backlog |

### Follow-ups inherited from prior cycles

| ID | Cycle | Título | Owner | Destino |
|---|---|---|---|---|
| m8-04-R4 | m8-04 | bundle-as-blob → side table: events still ride inside bundle blob | unassigned | m9+ |
| m8-06-R4 | m8-06 | Cross-variant existence predicate shrinking: variant still fixed in ExistencePredicateShrinker | unassigned | m9+ |
| m8-04-R-hypothesis-fallback | m8-04 | property_target lost in fallback reconstruction | unassigned | m9+ (non-issue post m8-07 but synthetic defaults remain for pre-m8-07 bundles) |

## Terminated terms

| ID | Cycle | Título | Closed by |
|---|---|---|---|
| m8-07-R2 | m8-07 | Unknown future wire fields are dropped | m9-01-schema-versioning (`v0.6.0`) |

## Metadata

| Campo | Valor |
|---|---|
| Project | chronos |
| Vault | `.sddk-knowledge/p-3416cfb8288f8964/` |
| Last updated | 2026-09-11T17:44:00Z |
| Last archive | m9-01-schema-versioning |
