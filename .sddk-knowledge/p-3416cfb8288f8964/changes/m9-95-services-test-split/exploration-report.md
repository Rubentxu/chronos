# Exploration Report — m9-95: services counterexample test-split

## Context

The m9-94 closure handoff identified
**FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC** as the next
carry-forward for the cc-001-god-module family. m9-95 closes that
finding by extracting the 2169-line inline `mod tests { ... }` block
from `crates/chronos-services/src/counterexample.rs` to a sibling file.

m9-94 established the `#[path = "..."]` submodule pattern for test
code extraction in `crates/chronos-store/src/counterexample_storage.rs`.
m9-95 applies the identical pattern to the chronos-services counterpart.

## Current state

`crates/chronos-services/src/counterexample.rs`:
- 3955 lines total
- 1785 lines of production code (45% of file)
- 2169 lines of inline `mod tests { ... }` block (55% of file)

The production code is well-organized into named sections (visible
from grep on `^/// ` doc-comment headers):
- `CounterexampleContext<'a>` (lifetime-tied borrow handle)
- `CounterexampleShrinkInput` enum (service-internal input)
- `CounterexampleOutput` enum (service-internal output variants)
- `CounterexampleRunError` (distinct failure modes)
- `ShrinkConfig` (proptest runner config wrapper)
- Proptest strategies + ValueTree impls (custom shrinking machinery)
- `ChronosCounterexampleService` (the service struct + impl methods)

The 2169-line inline test block contains:
- 4 helpers (`assert_send`, `dummy_target_invariant`, ...)
- ~45 test functions across multiple sub-categories:
  - Type-system invariants (~10 tests)
  - Shrink config mapping (~3 tests)
  - m8-02 roundtrip/proptest integration (~10 tests)
  - m8-06 strategy shrinking (~10 tests)
  - m8-07 hypothesis input roundtrip (~10 tests)
  - m9-01 save/load schema version (~2 tests)

The test block uses `use super::*;` + a few proptest imports
(`use proptest::strategy::{Strategy, ValueTree};`,
`use proptest::test_runner::Config;`). All imports are preserved
verbatim when the block moves to the sibling file.

## Pattern: `#[path = "..."]` submodule (already verified in m9-94)

The m9-94 cycle demonstrated this works for test extraction:

```rust
// In counterexample_storage.rs (parent)
#[cfg(test)]
#[path = "ce_storage_tests.rs"]
mod tests;
```

```rust
// In ce_storage_tests.rs (sibling)
//! Tests for ...
use super::*;
#[test] fn save_then_load_roundtrip() { ... }
```

Sibling `#[path]` modules inherit the parent's scope, so
`use super::*;` inside the test block continues to resolve to the
parent's private items. **Identical pattern applies for m9-95.**

## What stays vs. what moves

### Stays in counterexample.rs (1785 lines + 8 lines for the new declaration):

- Module doc comments
- `CounterexampleContext<'a>`, `CounterexampleShrinkInput`,
  `CounterexampleOutput`, `CounterexampleRunError`, `ShrinkConfig`
- Proptest strategy types + ValueTree impls
- `ChronosCounterexampleService` struct + impl methods
- New `#[path]` declaration for `tests` (8 lines)

### Moves to ce_services_tests.rs (~2172 lines):

- `mod tests { ... }` body verbatim (2169 lines + 3 lines for the
  file's outer wrapper + doc comment header)

## Test inventory (45 test functions + 4 helpers)

By category:

- **Type-system invariants** (10): `counterexample_context_is_send_when_inner_refs_are_send`, `counterexample_get_signature_accepts_bundle_id_string`, `counterexample_list_filter_default_has_zero_limit`, `counterexample_input_default_max_rounds_is_64`, `counterexample_bundle_summary_has_full_bundle_false_is_m8_01_state`, ...
- **Shrink config** (3): `m8_02_shrink_config_default_matches_pinned_max_rounds`, `m8_02_shrink_config_to_proptest_enforces_case_ceiling`, `m8_02_run_error_maps_to_documented_service_error_variants`
- **m8-02 roundtrip** (~10): hypothesis_input roundtrip invariants
- **m8-06 strategy shrinking** (~10): call_path_strategy, drive_strategy shrinks
- **m8-07 hypothesis input roundtrip** (~10): existence, call_path, invariant, all-predicate variants
- **m9-01 schema version** (~2): save_persists_schema_version_1, ...

Total: 45 test functions + 4 helpers. All 49 entities move verbatim.

## Why B-direct (not A-min)

- No public API change.
- No behavior change.
- No architectural decision.
- Pure mechanical file reorganization using an established pattern
  (m9-94 already verified it works for test code extraction).
- Tier T1+T2 only.
- No sandbox needed (no probe/mcp touched; no behavior change).

## Risk analysis

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `#[path]` attribute breaks compile | very low | medium | m9-94 already verified; same pattern |
| Private items become inaccessible | very low | medium | `use super::*;` already in test block; sibling `#[path]` preserves scope |
| Test count changes | very low | low | `cargo test -p chronos-services --lib` before + after — must report 268 either way |
| File ends up with stray whitespace | low | very low | `cargo fmt` runs as part of T0 |

## Recommendation

Proceed with m9-95 as B-direct. Single commit (Rust source) +
standard cycle artifacts + push.

## Out-of-scope (deferred to future cycles)

- cc-001-god-module production code split for `counterexample.rs` (1785 lines of production code). Not needed at this time — the production code is well-organized into named sections.
- cc-004-implicit-io-toctou (P3, LOW): explicit-IO refactor. m10+.
