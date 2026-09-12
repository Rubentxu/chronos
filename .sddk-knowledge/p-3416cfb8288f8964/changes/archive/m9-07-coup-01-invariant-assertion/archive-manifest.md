# Archive Manifest — m9-07-coup-01-invariant-assertion

## Identidad del ciclo

| Campo | Valor |
|---|---|
| Cycle | `m9-07-coup-01-invariant-assertion` |
| Change name | `m9-07-coup-01-invariant-assertion` |
| Path | `B-direct` |
| Status | **CLOSED** |
| Published SHA | `3edb01f0a17c3ac8877df1e7868d4e3cca374217` |
| Head SHA | `3edb01f0a17c3ac8877df1e7868d4e3cca374217` |
| Tag | `v0.7.5` (annotated, peel matches HEAD) |
| Base SHA | `aa96e5a51b5424a844f1f153d7ce719a53017119` |
| Delivery kind | local (no CLI storage; orchestrator owns transition) |

## Evidence bindings

### Release receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/release-receipt.md
sha256: ac5d1b925cfe1494ee0bb272e98d3f8d9ee0c49f47965c5e6347f7839562cf0d
verified_at: 2026-09-12T06:54:48Z
remote_tag_peel: 3edb01f0a17c3ac8877df1e7868d4e3cca374217
main_sha: 3edb01f0a17c3ac8877df1e7868d4e3cca374217
peel_match: true
pipeline_state: { release: complete, archive: closed }
```

### Merge receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/merge-receipt.md
sha256: 1afa15d6ed6c0af0a7611653289d647b4882f99e8e4d309d69c5d159e913a9c4
git_effect: git.push → 5c5b3ec... (docs head; tag peeled to 3edb01f fix head per m9-04..m9-06 convention)
sha_match: true
```

### Verify evidence

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/verify-report.md
sha256: 30215cd1769cbe1f9e999b518379b0cec22ec439701377f2e7b28c3af840fc84
status: PASS
mode: light-verify inline (B-direct)
scenarios: 1 (R1)
results: 1/1 COMPLIANT
```

### Release report

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/release-report.md
sha256: 0b2f7ae1db435b8f0d05231e812729766e6e0743c1cfd5b82bbec3421000d49c
verdict: success
git_effects: all PASS
verification_summary: PASS, 1/1 scenarios COMPLIANT
```

## Vault sync

### Findings resolved by m9-07

One finding closed by this cycle:

| ID | Cycle | Cluster | Severity | Priority | Título | Evidence |
|---|---|---|---|---|---|---|
| FIND-M9-01-DV-COUP-01 | m9-01 | coupling | MEDIUM | P2 | Duplicated `schema_version` on record + summary with no equality enforcement at load time | `crates/chronos-store/src/counterexample_storage.rs` — 5-line guard in `load_counterexample_bundle` returns `StoreError::Serialization` with `"disagrees with summary"` when `record.schema_version != record.summary.schema_version`; covered by `m9_07_loader_rejects_envelope_summary_version_mismatch` test |

**Closed by:** `m9-07-coup-01-invariant-assertion` · SHA `3edb01f0a17c3ac8877df1e7868d4e3cca374217` · tag `v0.7.5`

### Findings NOT resolved (m9+ backlog inherited)

| ID | Cluster | Severity | Título | Reason |
|---|---|---|---|---|
| FIND-M9-01-DV-COUP-02 | coupling | LOW | List/load policy asymmetry | Requires list-side change with a dedicated `SchemaTooNew` error variant at the next wire-format break |
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.6K LoC | Splits require design pass + compat shim |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write | Concurrency design required |
| m9-02 R1-R8 | various | — | See `changes/m9-02-events-side-table/change-entry.md` | Deferred from m9-02 |
| m8-06 R4 | various | — | Cross-variant existence predicate shrinking | Deferred from m8-06 |
| m8-04-R-hypothesis-fallback | various | — | property_target lost in fallback reconstruction | Deferred from m8-04 |

## Knowledge nodes updated

| Node kind | Path |
|---|---|
| Cycle entry (m9-07) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-07-coup-01-invariant-assertion/` |
| Terms (m9+) | `.sddk-knowledge/p-3416cfb8288f8964/terms/` (1 finding closed, 3 still active from m9-01/m9-02) |
| Cycles index | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` |
| Archive manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-07-coup-01-invariant-assertion/archive-manifest.md` |

## Ledger state (pre-archive)

```
note: CLI ledger not available (sddk not in PATH). Ad-hoc cycle, no CLI storage.
      Runtime status RELEASED recorded in release receipt.
```

## Specs synced

No new specs introduced. Trivial B-direct cleanup:
- Added 5-line guard in `load_counterexample_bundle`
- Added 1 doc comment explaining the invariant rationale
- Added 1 lib test pinning the new behavior

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-07-coup-01-invariant-assertion/archive-manifest.md` | `7794674eb58cd8b801f2e1670b37a8a1103ecf5ae262e338da845ba7a2da06fe` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-07-coup-01-invariant-assertion/change-entry.md` | `55722fa497b276cf22d06f70582082ac4fcc66135f4e76a2c83f1d306568e0f5` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/merge-receipt.md` | `1afa15d6ed6c0af0a7611653289d647b4882f99e8e4d309d69c5d159e913a9c4` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/release-receipt.md` | `ac5d1b925cfe1494ee0bb272e98d3f8d9ee0c49f47965c5e6347f7839562cf0d` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/release-report.md` | `0b2f7ae1db435b8f0d05231e812729766e6e0743c1cfd5b82bbec3421000d49c` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/verify-report.md` | `30215cd1769cbe1f9e999b518379b0cec22ec439701377f2e7b28c3af840fc84` |
