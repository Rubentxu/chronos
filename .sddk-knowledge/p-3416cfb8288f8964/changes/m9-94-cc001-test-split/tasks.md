# Tasks — m9-94: cc-001 god-module test-split

## Phase 1: Test extraction

### T1.1: Read existing test block

**File**: `crates/chronos-store/src/counterexample_storage.rs` (lines 189-1960)

Read the inline `mod tests { ... }` block. Note that:
- Line 189: `mod tests {`
- Line 1780-1787: comment header + `mod tests {` opens
- Line 1960: closing `}`

The full content is 1771 lines.

### T1.2: Create `ce_storage_tests.rs`

**File**: `crates/chronos-store/src/ce_storage_tests.rs` (new)

Copy the entire content of `mod tests { ... }` from
`counterexample_storage.rs` (excluding the `mod tests {` opener and
the closing `}` — those become the parent file's `#[path]`
declaration).

### T1.3: Replace inline `mod tests` with `#[path]` declaration

**File**: `crates/chronos-store/src/counterexample_storage.rs`

Replace the inline `mod tests { ... }` block (lines 1787-1960) with:
```rust
/// Tests for counterexample bundle storage.
///
/// Sibling file extracted in m9-94 to keep `counterexample_storage.rs`
/// focused on production code (188 LoC; cc-001 production split
/// completed in m9-84..m9-87).
#[cfg(test)]
#[path = "ce_storage_tests.rs"]
mod tests;
```

**Verify**: `wc -l` reports ~188 lines for counterexample_storage.rs.

### T1.4: Run T0 lint gate

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

**Expected**: clean (no diff; no new warnings).

### T1.5: Run T1 lib unit tests

```bash
cargo test -p chronos-store --lib --no-fail-fast
```

**Expected**: 77 passed.

## Phase 2: Cycle artifacts + archive

### T2.1: Write 7 cycle artifacts under `cycle-artifacts/p-3416cfb8288f8964/m9-94-cc001-test-split/`

Per AGENTS.md standard cycle artifact pattern:
- `apply-checkpoint.json`
- `implementation-receipt.md`
- `merge-receipt.md`
- `release-receipt.md`
- `release-report.md`
- `verify-findings.json`
- `verify-report.md`

### T2.2: Write archive-manifest

**File**: `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-94-cc001-test-split/archive-manifest.md`

Include SHA-256 evidence bindings for all 2 modified files + 7 cycle artifacts + 5 knowledge artifacts.

### T2.3: Update cycles/index.md

Add m9-94 row + bump Total 93 → 94.

### T2.4: Update terms/index.md

Set Last archive = m9-94-cc001-test-split.

### T2.5: Run `python3 scripts/regen_manifest_index_shas.py`

Cascade SHA fixpoint across all archive-manifests.

### T2.6: Pre-create tag v0.7.96 at cycle-artifacts commit

Per CC#42 fixpoint-cascade workaround. Tag stays at pre-cascade-fixpoint commit.

### T2.7: `--no-ff` merge into main + cascade SHA bumps + push

Standard pattern: source commit (Rust) → cycle-artifacts commit → cascade commits (SHA-256 fixpoint) → merge → archive + handoff → push to origin.

### T2.8: Write handoff

**File**: `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-94-cc001-test-split-closure-2026-09-14.md`

Include lessons learned + recommendations for next cycle.

### T2.9: Delete cycle branch

Per CC#53 hygiene.
