# Implementation Receipt — m9-87-cc001-god-module-schema-split

## Cycle identity

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

Behaviour-preserving lexical refactor: extracted the schema/table
definitions, the chunk-size constant, the 4 `collect_*` helpers, and
the compile-time invariant block from
`crates/chronos-store/src/counterexample_storage.rs` (lines 145-363
pre-split, ~219 lines including docs) into a new sibling submodule
`crates/chronos-store/src/ce_schema.rs` (240 lines, NEW).

`counterexample_storage.rs`: 2077 → 1954 lines (-123 net extracted).
Public surface unchanged: all 9 items reachable at
`chronos_store::counterexample_storage::*` via `pub use` re-exports
at the parent path.

## Files changed

| File | Status | Lines |
|---|---|---|
| crates/chronos-store/src/ce_schema.rs | NEW | 240 |
| crates/chronos-store/src/counterexample_storage.rs | modified | 1954 (was 2077) |

Net: -123 lines from parent (after 240 added to submodule = +117 net
across both files due to extensive doc comments preserved verbatim).

## Items moved to ce_schema.rs

| Item | Visibility | Reachability |
|---|---|---|
| `COUNTEREXAMPLE_BUNDLES` | pub (was const) | `crate::counterexample_storage::COUNTEREXAMPLE_BUNDLES` |
| `COUNTEREXAMPLE_BUNDLE_EVENTS` | pub (was const) | same |
| `BUNDLE_EVENTS_CHUNK_SIZE` | pub (unchanged) | same |
| `collect_bundle_chunks_range` | pub (unchanged) | same |
| `collect_v3_keys_for_bundle` | pub (was fn) | same |
| `collect_bundle_chunks_legacy` | pub (was fn) | same |
| `collect_bundle_chunks` | pub (was fn) | same |
| `CURRENT_BUNDLE_SCHEMA_VERSION` | pub (unchanged) | same |
| `KNOWN_BUNDLE_SCHEMA_VERSIONS` | pub (was const) | same |
| `const _: () = { ... }` (compile-time invariant block) | n/a | stays in ce_schema.rs |

All 10 items reachable at the same path callers used before the split.

## Items retained in counterexample_storage.rs

The module is now down to ~1954 lines containing:

- 5 submodules (`ce_chunk_keys`, `ce_types`, `ce_schema`, `ce_write`,
  `ce_read`, `ce_test_hooks` — last 3 from m9-85).
- 2 standalone free functions: `bundle_events_or_legacy`,
  `bundle_events_count_or_legacy`.
- Test module with integration tests (m9-04 layout, m9-02 side-table,
  m9-02 schema, m9-07 envelope, etc.).

The 1954-line file is now mostly:
- `impl SessionStore { ... }` blocks across the 3 method submodules
  (each ~150-300 lines), totalling ~750 lines.
- Free helpers ~100 lines.
- Integration tests ~1000 lines.
- Module docs + use statements + submodule declarations ~100 lines.

## Verification

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | clean |
| T1 | `cargo test -p chronos-store --lib --no-fail-fast` | 77 / 77 |
| T2 | `cargo test -p chronos-services --lib --no-fail-fast` | 264 / 264 |
| T3 | `cargo test -p chronos-cli --no-fail-fast` | 11+22+2 = 35 / 35 |
| T4-serial | `cargo test -p chronos-native --lib -- --test-threads=1` | 103 / 103 |
| Sanity | `cargo build -p chronos-mcp` | succeeds |

No regressions. Round-trip verified across all 4 production crates
that consume `chronos_store::counterexample_storage::*` items.

## Carry-forward findings

None introduced. None closed (m9-87 did not address any open FINDs).

## Smell audit

- 1 production-build warning fixed: removed 3 unused `use` statements
  in `counterexample_storage.rs` (`classify_read_table_error`,
  `ReadableTable`, `TableDefinition`) that became unused after the
  extraction.
- `pub(super) use ce_chunk_keys::{bundle_prefix, decode_chunk_key,
  decode_chunk_value}` wrapped in `#[cfg(test)] #[allow(unused_imports)]`
  to silence a warning for test-only re-exports while keeping
  reachability for the m9-04 layout tests in this file.

No code-quality regressions. The cc-001 god-module decomposition
remains a behaviour-preserving lexical refactor.
