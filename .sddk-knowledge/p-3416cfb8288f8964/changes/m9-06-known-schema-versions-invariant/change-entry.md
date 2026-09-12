# Change: m9-06 known schema versions invariant

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-06-known-schema-versions-invariant` |
| Path | `B-direct` |
| Status | CLOSED |
| Base SHA | `9d2c676bfc6331f730e7db9eac3ec9c057db1de4` |
| Head SHA | `3383905d93ac66b6e90b6de8cea760c1ad4e2c99` |
| Tag | `v0.7.4` (annotated, peel matches published SHA) |

## Commits

| SHA | Subject |
|---|---|
| `1feeab4` | fix(m9-06): remove stale #[allow(dead_code)] on KNOWN_BUNDLE_SCHEMA_VERSIONS; add compile-time invariant + test (closes m9-01-R4, FIND-M9-01-DV-OE-01) |
| `3383905` | docs(m9-06): light-verify evidence (R1+R2 PASS, 2/2 findings closed) |

## Scope

Trivial B-direct cleanup targeting two stale findings from the m9-01 cycle:

1. **`m9-01-R4`** — `KNOWN_BUNDLE_SCHEMA_VERSIONS` was marked `#[allow(dead_code)]` when it was speculative. After m9-04 added `m9_04_known_bundle_schema_versions_pinned` (lib test that pins `[1, 2, 3]`), the constant was no longer dead in tests. m9-06 makes it no longer dead in the production build by adding a compile-time invariant that requires `CURRENT_BUNDLE_SCHEMA_VERSION ∈ KNOWN_BUNDLE_SCHEMA_VERSIONS`. Future schema bumps will fail to compile if the new version isn't added to the known list.

2. **`FIND-M9-01-DV-OE-01`** — the constant was classified as "dead speculative code". After m9-06, it is referenced from a `const _: () = { ... }` block at module scope, making it production-used.

### Changed paths

- `crates/chronos-store/src/counterexample_storage.rs` — removed `#[allow(dead_code)]`; added compile-time invariant block; added `m9_06_current_schema_version_is_listed_as_known` test.

## Findings resolved

| ID | Cluster | Título | Closed by |
|---|---|---|---|
| m9-01-R4 | disclosure | `KNOWN_BUNDLE_SCHEMA_VERSIONS` unused; `#[allow(dead_code)]` | m9-06-known-schema-versions-invariant (`v0.7.4`) |
| FIND-M9-01-DV-OE-01 | overeng | `KNOWN_BUNDLE_SCHEMA_VERSIONS` dead speculative code | m9-06-known-schema-versions-invariant (`v0.7.4`) |

Verdict: **PASS** (2/2 findings closed)

## Artefactos

| Kind | Path |
|---|---|
| Merge receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/merge-receipt.md` |
| Release receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/release-receipt.md` |
| Release report | `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/release-report.md` |
| Verify report | `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/verify-report.md` |
| Archive manifest | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-06-known-schema-versions-invariant/archive-manifest.md` |