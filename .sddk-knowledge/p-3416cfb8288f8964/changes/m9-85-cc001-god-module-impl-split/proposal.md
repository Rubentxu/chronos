# Proposal — m9-85-cc001-god-module-impl-split

## Intent

Close the `impl SessionStore` concern of `cc-001-god-module` by
extracting 10 of 11 methods into 3 sibling submodules: `ce_write`,
`ce_read`, `ce_test_hooks`. (`save_bundle_record_and_events` stays
in `ce_write` as the private helper.)

## Approach

1. Create `crates/chronos-store/src/ce_write.rs` (write-path methods).
2. Create `crates/chronos-store/src/ce_read.rs` (read-path methods).
3. Create `crates/chronos-store/src/ce_test_hooks.rs` (test chokepoints).
4. In `counterexample_storage.rs`: declare the 3 submodules via
   `#[path = "x.rs"] pub mod x;`. **No `pub use` re-exports** — methods
   inside `impl SessionStore` blocks are not module-level items, so
   `pub use ce_write::save_counterexample_bundle` fails with E0432.
   The methods are still callable via `<SessionStore as _>::method_name`
   or `instance.method_name()` regardless of which file the `impl`
   lives in, so the public path is unchanged for all real callers.
5. Remove the moved `impl SessionStore` block from
   `counterexample_storage.rs`.

## Path

A-min (cross-crate, bounded scope, single-crate change but with
several downstream consumers that must continue to compile).

## Tier required

T1 + T2 + T3 — lib tests + downstream crates + integration tests
(`chronos-cli/tests/replay_integration.rs` uses the test chokepoints).

## Tier plan

- T0: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`.
- T1: `cargo test -p chronos-store --lib --no-fail-fast` (77 tests).
- T2: `cargo test -p chronos-services --lib --no-fail-fast` (264 tests).
- T3: `cargo test -p chronos-cli --no-fail-fast` (35 tests across 3 binaries, including the replay_integration integration test).

No sandbox needed (no probe/mcp touched).

## Carry-forward

Closes the `impl SessionStore` concern of `cc-001-god-module`. Does
NOT close the debt finding itself (the file remains 2,287 lines
after this cycle, down from 2,720 before m9-85). The remaining
concerns (type definitions, tests) will be split across m9-86+.

## Out of scope

- Type definitions section (~140 lines) — could split but reference
  coupling makes it lower priority.
- Tests (~1,770 lines) — kept in same file as the impl they cover.
- The standalone pub fns `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` — not methods on `SessionStore`,
  stay in parent.
