# Release Receipt — m9-94-cc001-test-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-94-cc001-test-split |
| Path | B-direct |
| Branch | chore/m9-94-cc001-test-split |
| Date | 2026-09-14 |

## Release details

| Field | Value |
|---|---|
| Base SHA | 645eedb530f1b827a600544d01e501d82b410820 |
| Head SHA | 3a8494967c366761f10da6caf89745971fce9f78 |
| Main SHA | 3a8494967c366761f10da6caf89745971fce9f78 |
| Remote tag | v0.7.96 |
| Remote tag_peel | 3a8494967c366761f10da6caf89745971fce9f78 |
| Peel match | true |

## SHAs (canonical table)

| Field | Value |
|---|---|
| Branch | chore/m9-94-cc001-test-split |
| Date | 2026-09-14 |
| Base SHA | 645eedb530f1b827a600544d01e501d82b410820 |
| Head SHA | 3a8494967c366761f10da6caf89745971fce9f78 |
| Remote tag | v0.7.96 |
| Remote tag_peel | 3a8494967c366761f10da6caf89745971fce9f78 |
| Peel match | true |

## Release notes

- B-direct mechanical refactor cycle. Closes FIND-M9-94-CC001-TEST-CODE-MONOLITHIC.
- Extracts the 1771-line inline `mod tests { ... }` block from
  `crates/chronos-store/src/counterexample_storage.rs` (lines 188-1960)
  into a sibling file `crates/chronos-store/src/ce_storage_tests.rs`
  using the `#[path = "..."]` submodule pattern established by
  m9-84..m9-87.
- After m9-94: `counterexample_storage.rs` shrinks from 1960 → 191 lines
  (production code only); `ce_storage_tests.rs` holds the 43 test
  functions + 4 test helpers.
- No public API change; no behavior change; tests use `use super::*;`
  so the parent's private items remain accessible via the `#[path]`
  submodule declaration.
- No new tests added (all 43 tests + 4 helpers moved verbatim).
- Workspace lib tests: 1042 pass (no regressions vs m9-93 baseline).
- Chronos-store lib tests: 77 pass (same as m9-93).

## Cross-checks

- `bash scripts/check_vault_drift.sh`: clean (no vault drift introduced by source commit).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean (after vault cascade).
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p chronos-store --lib --no-fail-fast`: 77 pass.
- `cargo test --workspace --lib --no-fail-fast -- --test-threads=1`: 1042 pass.
- `apply-checkpoint.peel_match == true`.
- `apply-checkpoint.head_sha == release-receipt.Head SHA == 3a84949`.
- `Remote tag` v0.7.96 peel: `3a84949` (source commit / cycle-artifacts commit).
