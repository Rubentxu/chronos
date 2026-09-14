# Verify Report — m9-87-cc001-god-module-schema-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-87-cc001-god-module-schema-split |
| Path | A-min |
| Branch | feat/m9-87-cc001-god-module-schema-split |
| Date | 2026-09-14 |
| Base SHA | 8df5c57b93bef0ff612a20bad1301b2c5875fb5a |
| Head SHA | 1bfd6a75493b7dc491866c76e52a2a6978e5af16 |
| Merge SHA | e222d854d97e4adae5f8b20410644609c191e545 |
| Remote tag | v0.7.89 |

## Summary

PASS. m9-87 is a behaviour-preserving lexical refactor. All 9 items
moved to `ce_schema.rs` are reachable at the same path callers used
before the split. Round-trip verified across 4 downstream consumers
(chronos-services, chronos-cli, chronos-native, chronos-mcp) with
no regressions. clippy + fmt clean.

## Path: cycle-artifacts/p-3416cfb8288f8964/m9-87-cc001-god-module-schema-split/verify-report.md

## Subject

m9-87 is a behaviour-preserving lexical refactor of
`counterexample_storage` into 6 submodules. The schema items
(TableDefinitions, chunk size constant, 4 collect helpers, schema
version constants, compile-time invariant block) move into a new
sibling submodule `ce_schema.rs`. The parent re-exports all 9 items
via `pub use` so external callers and sibling submodules reach them
at the same path.

This verify-report.md confirms the path stability + round-trip
behaviour across all downstream consumers (chronos-services,
chronos-cli, chronos-native, chronos-mcp).

## Verification scope

Behaviour-preserving lexical refactor: 9 items moved from
`counterexample_storage.rs` to `ce_schema.rs` (sibling submodule).
Public surface preserved via `pub use` re-exports at the parent path.

Verification must prove:

1. The 9 moved items are reachable at the same path callers used
   before the split (path stability).
2. All chronos-store lib tests pass.
3. All downstream consumers (chronos-services, chronos-cli,
   chronos-native) compile and pass their tests with the renamed
   submodule.

## Verification result

### 1. Path stability

All 9 items moved to `ce_schema.rs` are reachable at
`chronos_store::counterexample_storage::*` via the parent's
`pub use ce_schema::{...}` re-export:

| Item | Path before | Path after | Stable |
|---|---|---|---|
| `COUNTEREXAMPLE_BUNDLES` | `chronos_store::counterexample_storage::COUNTEREXAMPLE_BUNDLES` | same | yes |
| `COUNTEREXAMPLE_BUNDLE_EVENTS` | same | same | yes |
| `BUNDLE_EVENTS_CHUNK_SIZE` | same | same | yes |
| `collect_bundle_chunks_range` | same | same | yes |
| `collect_v3_keys_for_bundle` | same | same | yes (was private fn, now public via re-export) |
| `collect_bundle_chunks_legacy` | same | same | yes |
| `collect_bundle_chunks` | same | same | yes |
| `CURRENT_BUNDLE_SCHEMA_VERSION` | same | same | yes |
| `KNOWN_BUNDLE_SCHEMA_VERSIONS` | same | same | yes (was private const, now public via re-export) |

The 4 collect helpers (`collect_bundle_chunks`, etc.) and
`KNOWN_BUNDLE_SCHEMA_VERSIONS` were `fn`/`const` (no `pub`) in the
parent pre-split. They are now `pub` in `ce_schema.rs` and re-exported
at the parent — but their pre-split reachability was **inside** the
parent module (used by `save_bundle_record_and_events` and the
compile-time invariant block in the same file). Post-split, those
internal callers reach them via the parent's `pub use` re-export.

### 2. chronos-store lib tests (T1)

```
cargo test -p chronos-store --lib --no-fail-fast
test result: ok. 77 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Baseline pre-refactor: 77 / 0 / 0. Match: 77 / 0 / 0. PASS.

### 3. Downstream consumers (T2 + T3 + T4-serial)

```
cargo test -p chronos-services --lib --no-fail-fast
test result: ok. 264 passed; 0 failed; 0 ignored

cargo test -p chronos-cli --no-fail-fast
test result: ok. 11 passed (lib)
test result: ok. 22 passed (integration)
test result: ok. 2 passed (replay_integration)
test result: ok. 0 passed (doc-tests)

cargo test -p chronos-native --lib -- --test-threads=1
test result: ok. 103 passed; 0 failed; 0 ignored
```

All 4 downstream crates that consume `chronos_store::counterexample_storage::*`
items compile and pass with no regressions.

### 4. Lint + format (T0)

```
cargo fmt --all -- --check     → clean
cargo clippy --workspace --all-targets -- -D warnings → clean
```

The extraction touched `crate::error::StoreError`, `redb::{ReadableTable, TableDefinition}`,
and `crate::table_error::classify_read_table_error` imports in the parent;
these became unused after the section was moved. Removed (3 unused `use`
statements in counterexample_storage.rs). One residual warning
(`#[cfg(test)] #[allow(unused_imports)]` for test-only re-exports
of `bundle_prefix`, `decode_chunk_key`, `decode_chunk_value`) is
allowed for clarity.

## Smell audit

- No production-build warnings.
- No clippy lints.
- No fmt diffs.
- 1 `#[cfg(test)] #[allow(unused_imports)]` block to preserve test
  reachability for items that were "used in tests but not in production"
  at the parent's `pub(super) use ce_chunk_keys::{...}` re-export.
  This is a deliberate annotation, not a hidden warning.

## Conclusion

PASS. m9-87 is a behaviour-preserving lexical refactor of
`counterexample_storage` into 6 submodules (ce_chunk_keys, ce_types,
ce_schema, ce_write, ce_read, ce_test_hooks), with the parent down
from 2720 lines (m9-84 baseline) to 1954 lines (-766 lines, -28%
across the cc-001 god-module decomposition).

## Cross-checks

The following vault-drift-sweep checks were executed during the
m9-87 verify phase:

- T0 (`cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`): clean.
- T1 (`cargo test -p chronos-store --lib --no-fail-fast`): 77 / 77 PASS.
- T2 (`cargo test -p chronos-services --lib --no-fail-fast`): 264 / 264 PASS.
- T3 (`cargo test -p chronos-cli --no-fail-fast`): 35 / 35 PASS.
- T4-serial (`cargo test -p chronos-native --lib -- --test-threads=1`): 103 / 103 PASS.
- Sanity (`cargo build -p chronos-mcp`): succeeds.

All cross-checks match the baseline + delta recorded in
`apply-checkpoint.json` and `release-report.md`.

## Files Inventory

| File | Status | Change |
|---|---|---|
| `crates/chronos-store/src/ce_schema.rs` | NEW | 240 lines (extracted from counterexample_storage.rs) |
| `crates/chronos-store/src/counterexample_storage.rs` | modified | 2077 → 1954 lines (-123) |
| `crates/chronos-store/src/ce_read.rs` | (m9-85, pre-existing) | sibling submodule |
| `crates/chronos-store/src/ce_write.rs` | (m9-85, pre-existing) | sibling submodule |
| `crates/chronos-store/src/ce_test_hooks.rs` | (m9-85, pre-existing) | sibling submodule |

2 files directly affected by m9-87; sibling submodules from m9-85
unchanged. No production code change outside `ce_schema.rs` (a NEW
sibling submodule).
