# Tasks — m9-95: services counterexample test-split

## Phase 1: Test extraction

### T1.1: Read existing test block

**File**: `crates/chronos-services/src/counterexample.rs` (lines 1786-3955)

The inline `mod tests { ... }` block:
- Line 1786: `#[cfg(test)]`
- Line 1787: `mod tests {`
- Line 1788: `use super::*;` + proptest imports
- Line 3955: closing `}`

Total: 2169 lines.

### T1.2: Create `ce_services_tests.rs`

**File**: `crates/chronos-services/src/ce_services_tests.rs` (new)

Copy the entire content of `mod tests { ... }` from `counterexample.rs`
(excluding the `#[cfg(test)]` + `mod tests {` opener and the closing
`}` — those become the parent file's `#[path]` declaration).

### T1.3: Replace inline `mod tests` with `#[path]` declaration

**File**: `crates/chronos-services/src/counterexample.rs`

Replace the inline `mod tests { ... }` block (lines 1786-3955) with:
```rust
/// Tests for chronos-services counterexample module.
///
/// Sibling file extracted in m9-95 to keep `counterexample.rs` focused
/// on production code (~1785 LoC). The inline `mod tests { ... }`
/// block (2169 lines, 55% of the file) was the remaining
/// contributor to file length.
///
/// Tests use `use super::*;` so the parent's private items
/// (`ChronosCounterexampleService`, `CounterexampleContext`,
/// `CounterexampleShrinkInput`, etc.) remain accessible. The
/// `#[path = "ce_services_tests.rs"]` declaration in the parent file
/// makes this module a submodule of `counterexample`, preserving
/// the same module graph as before the extraction.
#[cfg(test)]
#[path = "ce_services_tests.rs"]
mod tests;
```

### T1.4: Run T0 lint gate

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

**Expected**: clean (no diff; no new warnings).

### T1.5: Run T1 lib unit tests

```bash
cargo test -p chronos-services --lib --no-fail-fast
```

**Expected**: 268 passed.

### T1.6: Run T2 workspace lib regression sweep

```bash
cargo test --workspace --lib --no-fail-fast -- --test-threads=1
```

**Expected**: 1042 passed (same as m9-94 baseline).

## Phase 2: Cycle artifacts + archive

### T2.1: Write 7 cycle artifacts under `cycle-artifacts/p-3416cfb8288f8964/m9-95-services-test-split/`

Per AGENTS.md standard cycle artifact pattern:
- `apply-checkpoint.json`
- `implementation-receipt.md`
- `merge-receipt.md`
- `release-receipt.md`
- `release-report.md` (with `## Cross-checks` section per CC#39 Part B)
- `verify-findings.json`
- `verify-report.md`

### T2.2: Write archive-manifest

**File**: `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-95-services-test-split/archive-manifest.md`

Include SHA-256 evidence bindings for all 2 modified files + 7 cycle artifacts + 5 knowledge artifacts.

### T2.3: Update cycles/index.md

Add m9-95 row + bump Total 94 → 95.

### T2.4: Update terms/index.md

Set Last archive = m9-95-services-test-split.

### T2.5: Run `python3 scripts/regen_manifest_index_shas.py`

Cascade SHA fixpoint across all archive-manifests.

### T2.6: Pre-create tag v0.7.97 at cycle-artifacts commit

Per CC#42 fixpoint-cascade workaround. Tag stays at pre-cascade-fixpoint commit.

### T2.7: `--no-ff` merge into main + cascade SHA bumps + push

Standard pattern: source commit (Rust) → cycle-artifacts commit → cascade commits (SHA-256 fixpoint) → merge → archive + handoff → push to origin.

### T2.8: Write handoff

**File**: `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-95-services-test-split-closure-2026-09-14.md`

Include lessons learned + recommendations for next cycle.

### T2.9: Delete cycle branch

Per CC#53 hygiene.

## Verification commands

```bash
# T0
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# T1
cargo test -p chronos-services --lib --no-fail-fast

# T2
cargo test --workspace --lib --no-fail-fast -- --test-threads=1

# Vault
python3 scripts/regen_manifest_index_shas.py --check
bash scripts/check_vault_drift.sh
```
