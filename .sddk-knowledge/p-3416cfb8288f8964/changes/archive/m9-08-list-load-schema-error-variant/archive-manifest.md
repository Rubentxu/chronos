# Archive Manifest — m9-08-list-load-schema-error-variant

## Identidad del ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-08-list-load-schema-error-variant` |
| Change name | `m9-08-list-load-schema-error-variant` |
| Path | `B-direct` |
| Status | **CLOSED** |
| Published SHA | `d89862bbe67256cf6274be1d71ee7f8857cb8808` |
| Tag | `v0.7.6` (annotated, peel matches HEAD) |
| Base SHA | `50553969309b945156c239fd5a5a7f794431df21` |
| Delivery kind | local (no CLI storage; orchestrator owns transition) |

## Evidence bindings

### Release receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/release-receipt.md
sha256: 6526bd6370fa080f5330f982ac5a858dcdec09b46afdb668498a2d882a819154
verified_at: 2026-09-12T07:03:02Z
remote_tag_peel: d89862bbe67256cf6274be1d71ee7f8857cb8808
main_sha: d89862bbe67256cf6274be1d71ee7f8857cb8808
peel_match: true
pipeline_state: { release: complete, archive: closed }
```

### Merge receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/merge-receipt.md
sha256: 054bd94746a670d12b2ca61a4249eba901fe0a7821f01b6366e31e46d8031318
git_effect: git.push → cb47fe8... (docs head; tag peeled to d89862b fix head per m9-04..m9-07 convention)
sha_match: true
```

### Verify evidence

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/verify-report.md
sha256: 635efcc50a6a0fcfa1d35eb2066ad1b3befad48943693fc6cb389b351367b3a7
status: PASS
mode: light-verify inline (B-direct)
scenarios: 1 (R1)
results: 1/1 COMPLIANT
```

### Release report

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/release-report.md
sha256: 621fc4b9b2602f4ea4492ec384e8994b1d94005c05ff423511bd2f199ef86d85
verdict: success
git_effects: all PASS
verification_summary: PASS, 1/1 scenarios COMPLIANT
```

## Vault sync

### Findings resolved by m9-08

One finding closed by this cycle:

| ID | Cycle | Cluster | Severity | Priority | Título | Evidence |
|---|---|---|---|---|---|---|
| FIND-M9-01-DV-COUP-02 | m9-01 | coupling | LOW | P3 | List/load policy asymmetry shipped as an error-kind overload (forward-compat reject should not be `Serialization`) | `crates/chronos-store/src/error.rs` — new `StoreError::SchemaTooNew { found, supported }` variant; `crates/chronos-store/src/counterexample_storage.rs` — loader returns it instead of `Serialization(format!(...))`; covered by `error::tests::test_schema_too_new_variant_display_and_fields` and updated `m9_02_future_version_load_is_rejected` + `m9_04_saved_v3_schema_and_future_rejection` |

**Closed by:** `m9-08-list-load-schema-error-variant` · SHA `d89862bbe67256cf6274be1d71ee7f8857cb8808` · tag `v0.7.6`

### Findings NOT resolved (m9+ backlog inherited)

| ID | Cluster | Severity | Título | Reason |
|---|---|---|---|---|
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.6K LoC | Splits require design pass + compat shim |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write | Concurrency design required |
| m9-02 R1-R8 | various | — | See `changes/m9-02-events-side-table/change-entry.md` | Deferred from m9-02 |
| m8-06 R4 | various | — | Cross-variant existence predicate shrinking | Deferred from m8-06 |
| m8-04-R-hypothesis-fallback | various | — | property_target lost in fallback reconstruction | Deferred from m8-04 |

## Knowledge nodes updated

| Node kind | Path |
|---|---|
| Cycle entry (m9-08) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-08-list-load-schema-error-variant/` |
| Terms (m9+) | `.sddk-knowledge/p-3416cfb8288f8964/terms/` (1 finding closed, 2 still active) |
| Cycles index | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` |
| Archive manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-08-list-load-schema-error-variant/archive-manifest.md` |

## Ledger state (pre-archive)

```
note: CLI ledger not available (sddk not in PATH). Ad-hoc cycle, no CLI storage.
      Runtime status RELEASED recorded in release receipt.
```

## Specs synced

No new specs introduced. Trivial B-direct cleanup:
- Added 1 enum variant (`SchemaTooNew { found, supported }`) with `#[error(...)]` Display
- Replaced `Serialization(format!(...))` with `SchemaTooNew { ... }` in loader
- Added 1 unit test (Display + field pattern match)
- Added 2 `match &err { ... }` blocks to pre-existing tests

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-08-list-load-schema-error-variant/archive-manifest.md` | `5821de4a7867f0a3503f9f857c47a53901b349c61a5e33d65e390d53c91d2b96` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-08-list-load-schema-error-variant/change-entry.md` | `fe60bbb9e279bc9fc7643fbbcd06c707f9fc7e4f403e9d3642ac74b7080fd122` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/merge-receipt.md` | `054bd94746a670d12b2ca61a4249eba901fe0a7821f01b6366e31e46d8031318` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/release-receipt.md` | `6526bd6370fa080f5330f982ac5a858dcdec09b46afdb668498a2d882a819154` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/release-report.md` | `621fc4b9b2602f4ea4492ec384e8994b1d94005c05ff423511bd2f199ef86d85` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/verify-report.md` | `635efcc50a6a0fcfa1d35eb2066ad1b3befad48943693fc6cb389b351367b3a7` |
