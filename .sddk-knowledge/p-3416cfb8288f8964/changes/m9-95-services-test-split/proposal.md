# Proposal — m9-95: services counterexample test-split

## Identification

| Field | Value |
|---|---|
| Cycle ID | `m9-95-services-test-split` |
| Workspace | `p-3416cfb8288f8964` |
| Path | B-direct (mechanical test code extraction, no behavior change) |
| Status | proposed |
| Branch | `chore/m9-95-services-test-split` |
| Tag | `v0.7.97` |

## Subject

Extract the 2169-line `mod tests { ... }` block from
`crates/chronos-services/src/counterexample.rs` (lines 1786-3955) into
a sibling file `crates/chronos-services/src/ce_services_tests.rs` using
the `#[path = "..."]` submodule pattern established by m9-94.

## Problem

`cc-001-god-module` was tracked across two files: the production code
splits (m9-84..m9-87) extracted concerns from
`crates/chronos-store/src/counterexample_storage.rs`, and the test
half split (m9-94) extracted the 1771-line `mod tests { ... }` from
the same file. The remaining test-half concern is the even larger
`mod tests { ... }` block in
`crates/chronos-services/src/counterexample.rs` — flagged as
`FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC` by the m9-94
closure handoff.

After m9-87 + m9-94: `counterexample_storage.rs` is well-split, but
`counterexample.rs` is still 3955 lines with 2169 lines (55% of the
file) in a single inline `mod tests { ... }` block.

## Approach

B-direct mechanical refactor using the same `#[path = "..."]`
submodule pattern as m9-94:

1. Create `crates/chronos-services/src/ce_services_tests.rs` (new
   sibling file).
2. Move the entire content of `mod tests { ... }` block from
   `counterexample.rs` into `ce_services_tests.rs` verbatim (with the
   `use super::*;` + `use proptest::strategy::{...};` imports — the
   sibling `#[path]` submodule preserves the parent's scope).
3. Replace the inline `mod tests { ... }` block in `counterexample.rs`
   with a one-line declaration:
   ```rust
   #[cfg(test)]
   #[path = "ce_services_tests.rs"]
   mod tests;
   ```

This is a **purely mechanical refactor**:
- No public API change (tests are `#[cfg(test)]` only).
- No behavior change (every test moves verbatim).
- No fixture changes (`assert_send`, `dummy_target_invariant`, etc. are
  all test-internal helpers).
- No re-export changes (the test block uses `super::*` which still
  resolves to the parent module's items including private types like
  `ChronosCounterexampleService`, `CounterexampleContext`,
  `CounterexampleShrinkInput`, etc.).

## Out-of-scope

- cc-001-god-module production code split for
  `counterexample.rs` (1785 lines of production code). Separate cycle
  if/when warranted; cc-001 has been at LOW risk since m9-87 closed
  the storage half and m9-94 closed the storage test half. The
  services-side production code is well-organized (separate sections
  for context, input/output, errors, shrink config, service struct,
  impl methods, etc.).
- cc-004-implicit-io-toctou (P3, LOW, deferred to m10+).

## Files changed

| Bucket | Files | Lines | Notes |
|---|---|---|---|
| chronos-services/src/ce_services_tests.rs | 1 added | +2170 | verbatim copy of inline test block + doc comment header |
| chronos-services/src/counterexample.rs | 1 modified | -2169 / +3 | removed test block, added `#[path]` declaration |

**Total**: 1 file added + 1 file modified; net ~+5 LoC (file header + module decl).

## Tier

- **T0**: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`.
- **T1**: `cargo test -p chronos-services --lib --no-fail-fast` (must still pass 268 tests, now from the new sibling file).
- **T2**: `cargo test --workspace --lib --no-fail-fast -- --test-threads=1` (must still pass 1042 tests).

No sandbox needed (no probe/mcp touched; no behavior change).

## Verification

- All 268 chronos-services lib tests continue to pass.
- All 1042 workspace lib tests continue to pass.
- The new sibling file `ce_services_tests.rs` compiles as
  `#[cfg(test)]` only and inherits the parent module's scope
  (private items accessible via `super::*`).
- No clippy warnings introduced (the tests use the same idioms as
  before).
- `counterexample.rs` shrinks from 3955 → ~1788 lines (production
  code only).
