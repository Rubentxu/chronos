# Archive Manifest — m9-87-cc001-god-module-schema-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-87-cc001-god-module-schema-split |
| Path | A-min |
| Branch | feat/m9-87-cc001-god-module-schema-split |
| Date | 2026-09-14 |
| Base SHA | 8df5c57b93bef0ff612a20bad1301b2c5875fb5a |
| Head SHA | `1bfd6a75493b7dc491866c76e52a2a6978e5af16` |
| Merge SHA | e222d854d97e4adae5f8b20410644609c191e545 |
| Remote tag | v0.7.89 |
| Tag peel SHA | e222d854d97e4adae5f8b20410644609c191e545 |

## Summary

Behaviour-preserving lexical refactor: extracted 9 items from
`crates/chronos-store/src/counterexample_storage.rs` (lines 145-363
pre-split, ~219 lines including extensive doc comments) into a new
sibling submodule `crates/chronos-store/src/ce_schema.rs` (240 lines,
NEW).

`counterexample_storage.rs`: 2077 → 1954 lines (-123 net extracted).
Public surface unchanged: all 9 items reachable at
`chronos_store::counterexample_storage::*` via `pub use` re-exports
at the parent path.

## Items moved to ce_schema.rs

| Item | Visibility | Reachability after |
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

All 10 items (9 named items + the invariant block) are reachable at
the same path callers used before the split.

## Files changed

| File | Status | Lines |
|---|---|---|
| crates/chronos-store/src/ce_schema.rs | NEW | 240 |
| crates/chronos-store/src/counterexample_storage.rs | modified | 1954 (was 2077) |

## Verification

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | clean |
| T1 | `cargo test -p chronos-store --lib --no-fail-fast` | 77 / 77 |
| T2 | `cargo test -p chronos-services --lib --no-fail-fast` | 264 / 264 |
| T3 | `cargo test -p chronos-cli --no-fail-fast` | 11+22+2 = 35 / 35 |
| T4-serial | `cargo test -p chronos-native --lib -- --test-threads=1` | 103 / 103 |
| Sanity | `cargo build -p chronos-mcp` | succeeds |

No regressions. All 4 downstream consumers (chronos-services,
chronos-cli, chronos-native, chronos-mcp) compile and pass with the
new `ce_schema` submodule.

## Findings (carry-forward)

None introduced. None closed.

## Smell audit

- 3 unused `use` statements removed in `counterexample_storage.rs`
  (`classify_read_table_error`, `ReadableTable`, `TableDefinition`)
  that became unused after the schema section was extracted.
- 1 deliberate `#[cfg(test)] #[allow(unused_imports)]` annotation
  for test-only re-exports (`bundle_prefix`, `decode_chunk_key`,
  `decode_chunk_value` from `ce_chunk_keys`) used by the m9-04 layout
  tests in the parent file.

No code-quality regressions. cc-001 god-module decomposition
remains a behaviour-preserving lexical refactor.

## Cross-cycle context

cc-001 god-module decomposition progress:

| Cycle | Submodule created | Parent (lines) | Delta |
|---|---|---|---|
| m9-84 | ce_chunk_keys | 2821 → 2720 | -101 (key encoding helpers) |
| m9-85 | ce_write, ce_read, ce_test_hooks | 2720 → 2287 | -433 (impl blocks) |
| m9-86 | ce_types | 2287 → 2077 | -210 (types) |
| m9-87 | ce_schema | 2077 → 1954 | -123 (schema) |
| **Total** | 6 submodules | **-766 (-28%)** | across 4 cycles |

Remaining work in cc-001: ~1954 lines split into impl blocks across
3 method submodules (~750 lines), free helpers (~100 lines), and
integration tests (~1000 lines). Further slimming possible via
extracting tests to a `tests.rs` submodule, but diminishing returns.

## Carry-forward for m9-88+

- FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK (CLI ledger FK bug) — open.
- M7 milestone (events_read merge, observe merge, session_compare/explain
  split, lifecycle) — open.
- cc-001 final slimming (tests submodule extraction) — optional,
  diminishing returns.

## Evidence bindings

- `cycle-artifacts/p-3416cfb8288f8964/m9-87-cc001-god-module-schema-split/apply-checkpoint.json` → d527926c9c8e76fff2e7a30d3f2c85f6c98be9d8f90647072603f356a16addf4
- `cycle-artifacts/p-3416cfb8288f8964/m9-87-cc001-god-module-schema-split/implementation-receipt.md` → b67608cf28ba96376100aea4f672ebf454437b8b5b99150fac9534d142bb2843
- `cycle-artifacts/p-3416cfb8288f8964/m9-87-cc001-god-module-schema-split/merge-receipt.md` → 0911692d0b6ca251e5106969753c1ee1f17042fd0dcbb362b310ff40ac66b5b5
- `cycle-artifacts/p-3416cfb8288f8964/m9-87-cc001-god-module-schema-split/release-receipt.md` → 323d1cef5a9ac0690c3115eb164619f696c20a266baf216bfd8023f5578ab817
- `cycle-artifacts/p-3416cfb8288f8964/m9-87-cc001-god-module-schema-split/release-report.md` → 6a2f5fba93b1bd8485c9395349e133ad7cd73f0072f672f0ad4a7cde0ff9558c
- `cycle-artifacts/p-3416cfb8288f8964/m9-87-cc001-god-module-schema-split/verify-findings.json` → 38311393a6f9ef7a9ba13f18e26bf3addca3f0ac27a5caf7297285013cc53d06
- `cycle-artifacts/p-3416cfb8288f8964/m9-87-cc001-god-module-schema-split/verify-report.md` → 495495cdc3791444346af1d9d50810f64d2bd3c5fb052c570fc0e48b2e5fdbd8
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-87-cc001-god-module-schema-split/exploration-report.md` → 84d35414710a21dfdced5d87ef736c3a88ba953568091e8c9ee24e585811ecb1
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-87-cc001-god-module-schema-split/proposal.md` → e8538c455ceb94f517c9f3bc0d684b2f334a874dfc8f1986e87705df00854e51
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-87-cc001-god-module-schema-split/spec.md` → a33fc3de4119f8231d8ee9db3a034573bbf58e2b31b9d0d9b7f6933408cb6953
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-87-cc001-god-module-schema-split/tasks.md` → 5c0fa8bccc2c51897de7fb4b41b06ffc48a9f729257914fdf10ff0cd2ddb9cf3

## Cross-checks

- `apply-checkpoint.head_sha` == `release-receipt.head_sha` ==
  `merge-receipt.head SHA` == `1bfd6a75493b7dc491866c76e52a2a6978e5af16`.
- `Remote tag` v0.7.89 peel: `e222d854d97e4adae5f8b20410644609c191e545` (clean match).
- `counterexample_storage.rs` line count: 1954 (was 2077; net -123).
- `ce_schema.rs` line count: 240 (NEW file).
- `cargo test -p chronos-store --lib`: 77/77 (matches baseline).
- `cargo test -p chronos-services --lib`: 264/264 (downstream).
- `cargo test -p chronos-cli`: 35/35 (downstream, includes replay_integration round-trip).
- `cargo test -p chronos-native --lib -- --test-threads=1`: 103/103 (per AGENTS.md §6.5).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `bash scripts/check_vault_drift.sh`: to be verified post-archive.
