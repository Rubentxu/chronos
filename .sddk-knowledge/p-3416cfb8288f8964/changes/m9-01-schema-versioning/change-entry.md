# Change: m9-01 schema versioning

## Summary

Drift closure cycle for this milestone.


## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-01-schema-versioning` |
| Path | `A-min` |
| Status | CLOSED |
| Base SHA | `bceddc946b73209756d3360b0aec4dca592296c3` |
| Head SHA | `25948147869307343a28e3896e8289c715efbc81` |
| Tag | `v0.6.0` |

## Commits

| SHA | Subject |
|---|---|
| `0060a27` | docs(m9-01): scoping — schema_version on CounterexampleBundleRecord/Summary |
| `c346d8b` | feat(chronos-store): schema_version on CounterexampleBundleRecord/Summary (m9-01) |
| `cb4de8d` | chore(m9-01): apply-checkpoint after T4-smoke pass |
| `0aec8ba` | chore(fmt): apply rustfmt to m9-01 new test bodies (verify correction) |
| `2594814` | docs(m9-01): verify + debt-verify evidence artifacts |

## Scope

Added `schema_version: u32` field to `CounterexampleBundleRecord` and
`CounterexampleBundleSummary`. Canonical writer sets both fields atomically.
`load()` rejects `schema_version > CURRENT` with `StoreError::Serialization`.
`list()` best-effort includes future-versioned rows.

### Changed paths

- `crates/chronos-store/src/counterexample_storage.rs`
- `crates/chronos-services/src/counterexample.rs`
- `crates/chronos-cli/src/replay.rs`

## Gates

| Gate | Status | Notes |
|---|---|---|
| T0 fmt + clippy | PASS | dead_code suppress on KNOWN_BUNDLE_SCHEMA_VERSIONS per R4 |
| T1 chronos-store lib | PASS | 32/32 tests |
| T2 chronos-services lib | PASS | 260/260 tests |
| T2 chronos-store integration | PASS | 32/32 tests |
| T2 chronos-cli | PASS | 20/20 tests |

## Disclosures

| ID | Título | Estado | Destino |
|---|---|---|---|
| R1 | schema_version silently overwritten on save | open | m9+ |
| R2 | Future-versioned bundles in list (best-effort) | open | m9+ |
| R3 | schema_version is internal-only | open | m9+ |
| R4 | KNOWN_BUNDLE_SCHEMA_VERSIONS unused | open | m9+ |

## Deuda técnica

| ID | Cluster | Severity | Priority | Título |
|---|---|---|---|---|
| FIND-M9-01-DV-COUP-01 | coupling | MEDIUM | P2 | Duplicated version envelopes |
| FIND-M9-01-DV-COUP-02 | coupling | LOW | P3 | List/load policy asymmetry |
| FIND-M9-01-DV-OE-01 | overeng | LOW | P3 | KNOWN_BUNDLE_SCHEMA_VERSIONS dead code |

Verdict: PASS_WITH_WARNINGS · All findings → backlog · No INC files

## Artefactos

| Kind | Path |
|---|---|
| Apply checkpoint | `apply-checkpoint.json` |
| Scoping doc | `docs/milestones/m9-01-schema-versioning-scoping.md` |
| Merge receipt | `cycle-artifacts/m9-01-schema-versioning/receipts/merge-receipt.json` |
| Release receipt | `cycle-artifacts/m9-01-schema-versioning/receipts/release-receipt.json` |
| Release report | `cycle-artifacts/m9-01-schema-versioning/receipts/release-report.md` |
| Debt report | `cycle-artifacts/m9-01-schema-versioning/debt-verify/debt-report.json` |
| Archive manifest | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-manifest.md` |
| Archive report | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-report.md` |

## Files changed

- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-01-schema-versioning/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Seguimiento m9+ pendiente

- R1: evaluar si merece enforcement de igualdad record.summary en loader
- R2: añadir `SchemaTooNew { found, supported }` error variant en v2 bump
- R3: documentar que schema_version no cruza la frontera MCP
- R4: eliminar KNOWN_BUNDLE_SCHEMA_VERSIONS + re-add at first version-set tightening
- FIND-M9-01-DV-COUP-01: loader assertion `record.schema_version == summary.schema_version`
- FIND-M9-01-DV-COUP-02: dedicated error variant for v2 bump
- FIND-M9-01-DV-OE-01: delete dead code
