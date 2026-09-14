# Spec — m9-95: services counterexample test-split

## ADDED Requirements

### REQ-m9-95-1: Test code must live in a sibling file

The 2169-line `mod tests { ... }` block currently embedded in
`crates/chronos-services/src/counterexample.rs` (lines 1786-3955) must
be extracted into a sibling file
`crates/chronos-services/src/ce_services_tests.rs`. The parent file
must contain only a one-line `#[path]` submodule declaration that
points at the sibling.

**Acceptance criterion**:
```bash
$ wc -l crates/chronos-services/src/counterexample.rs
1788 crates/chronos-services/src/counterexample.rs

$ wc -l crates/chronos-services/src/ce_services_tests.rs
2172 crates/chronos-services/src/ce_services_tests.rs
```

### REQ-m9-95-2: All 268 chronos-services tests must continue to pass

After the extraction, `cargo test -p chronos-services --lib --no-fail-fast`
must report **268 passed** (the same count as before m9-95). No test
may be lost, modified, or skipped.

**Acceptance criterion**:
```bash
$ cargo test -p chronos-services --lib --no-fail-fast
...
test result: ok. 268 passed; 0 failed; ...
```

### REQ-m9-95-3: No public API change

The extraction is internal-only (`#[cfg(test)]`). No `pub` or
`pub(crate)` items change. No re-exports added or removed. External
callers see no API difference.

**Acceptance criterion**: `cargo build --workspace --all-targets`
produces the same set of public items.

## MODIFIED Requirements

None.

## REMOVED Requirements

None.

## Scenarios

### Scenario S1: chronos-services test count unchanged

**Given**: chronos-services built with m9-95 changes
**When**: `cargo test -p chronos-services --lib` runs
**Then**: 268 tests pass (same as m9-94 baseline).

### Scenario S2: Workspace test count unchanged

**Given**: m9-95 changes applied
**When**: `cargo test --workspace --lib --no-fail-fast -- --test-threads=1` runs
**Then**: 1042 tests pass (same as m9-94 baseline).

### Scenario S3: File structure reflects extraction

**Given**: m9-95 changes applied
**When**: `wc -l crates/chronos-services/src/counterexample.rs crates/chronos-services/src/ce_services_tests.rs` runs
**Then**: counterexample.rs has ~1788 lines; ce_services_tests.rs has ~2172 lines.

### Scenario S4: No clippy regression

**Given**: m9-95 changes applied
**When**: `cargo clippy --workspace --all-targets -- -D warnings` runs
**Then**: No warnings introduced; the test code uses the same idioms as before.

### Scenario S5: Sibling file pattern matches m9-94

**Given**: m9-95 changes applied
**When**: `grep -E '#\[path =' crates/chronos-services/src/counterexample.rs` runs
**Then**: 1 `#[path = "ce_services_tests.rs"]` declaration visible.

## Out-of-scope

- cc-001-god-module production code split for `counterexample.rs` (separate cycle if/when warranted).
- cc-004-implicit-io-toctou (deferred to m10+).

## Acceptance criteria (summary)

1. `cargo fmt --all -- --check` clean.
2. `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. `cargo test -p chronos-services --lib --no-fail-fast` reports 268 passed.
4. `cargo test --workspace --lib --no-fail-fast -- --test-threads=1` reports 1042 passed.
5. `cargo build --workspace --all-targets` succeeds (downstream crates unaffected).
6. `wc -l` confirms the extraction: counterexample.rs ~1788 lines; ce_services_tests.rs ~2172 lines.
