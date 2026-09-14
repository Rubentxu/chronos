# Spec — m9-94: cc-001 god-module test-split

## ADDED Requirements

### REQ-m9-94-1: Test code must live in a sibling file

The 1771-line `mod tests { ... }` block currently embedded in
`crates/chronos-store/src/counterexample_storage.rs` (lines 189-1960)
must be extracted into a sibling file
`crates/chronos-store/src/ce_storage_tests.rs`. The parent file
must contain only a one-line `#[path]` submodule declaration that
points at the sibling.

**Acceptance criterion**:
```bash
$ wc -l crates/chronos-store/src/counterexample_storage.rs
188 crates/chronos-store/src/counterexample_storage.rs

$ wc -l crates/chronos-store/src/ce_storage_tests.rs
1772 crates/chronos-store/src/ce_storage_tests.rs
```

### REQ-m9-94-2: All 77 chronos-store tests must continue to pass

After the extraction, `cargo test -p chronos-store --lib --no-fail-fast`
must report **77 passed** (the same count as before m9-94). No test
may be lost, modified, or skipped.

**Acceptance criterion**:
```bash
$ cargo test -p chronos-store --lib --no-fail-fast
...
test result: ok. 77 passed; 0 failed; ...
```

### REQ-m9-94-3: No public API change

The extraction is internal-only (`#[cfg(test)]`). No `pub` or
`pub(crate)` items change. No re-exports added or removed. External
callers see no API difference.

**Acceptance criterion**: `cargo build --workspace --all-targets`
produces the same set of public items (verified by `cargo doc`
output diff if needed; in practice: clippy clean + downstream crate
builds pass).

## MODIFIED Requirements

None.

## REMOVED Requirements

None.

## Scenarios

### Scenario S1: Test count unchanged

**Given**: chronos-store built with m9-94 changes
**When**: `cargo test -p chronos-store --lib` runs
**Then**: 77 tests pass (same as m9-93 baseline).

### Scenario S2: File structure reflects extraction

**Given**: m9-94 changes applied
**When**: `wc -l crates/chronos-store/src/counterexample_storage.rs crates/chronos-store/src/ce_storage_tests.rs` runs
**Then**: counterexample_storage.rs has ~188 lines; ce_storage_tests.rs has ~1772 lines.

### Scenario S3: No clippy regression

**Given**: m9-94 changes applied
**When**: `cargo clippy --workspace --all-targets -- -D warnings` runs
**Then**: No warnings introduced; the test code uses the same idioms as before.

### Scenario S4: Sibling file pattern matches m9-84..m9-87

**Given**: m9-94 changes applied
**When**: `grep -E '#\[path =' crates/chronos-store/src/counterexample_storage.rs` runs
**Then**: 5 `#[path = "..."]` declarations visible (ce_chunk_keys, ce_write, ce_read, ce_test_hooks, ce_types, ce_schema, ce_storage_tests — including the new one).

## Out-of-scope

- `crates/chronos-services/src/counterexample.rs` (2169 lines of tests). Same pattern can be applied in a future cycle.
- cc-001 production code re-split (already complete in m9-84..m9-87).
- cc-004-implicit-io-toctou (deferred to m10+).

## Acceptance criteria (summary)

1. `cargo fmt --all -- --check` clean.
2. `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. `cargo test -p chronos-store --lib --no-fail-fast` reports 77 passed.
4. `cargo build --workspace --all-targets` succeeds (downstream crates unaffected).
5. `wc -l` confirms the extraction: counterexample_storage.rs ~188 lines; ce_storage_tests.rs ~1772 lines.
