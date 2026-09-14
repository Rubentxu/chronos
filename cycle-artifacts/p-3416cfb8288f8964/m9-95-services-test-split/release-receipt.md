# Release Receipt — m9-95-services-test-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-95-services-test-split |
| Path | B-direct |
| Branch | chore/m9-95-services-test-split |
| Date | 2026-09-14 |

## Release details

| Field | Value |
|---|---|
| Base SHA | 72bff2808457a4ead6f4caec233dc404a20c35d8 |
| Head SHA | eb96861b17711f7d525cedacccc17905bc17e6e0 |
| Main SHA | eb96861b17711f7d525cedacccc17905bc17e6e0 |
| Remote tag | v0.7.97 |
| Remote tag_peel | eb96861b17711f7d525cedacccc17905bc17e6e0 |
| Peel match | true |

## SHAs (canonical table)

| Field | Value |
|---|---|
| Branch | chore/m9-95-services-test-split |
| Date | 2026-09-14 |
| Base SHA | 72bff2808457a4ead6f4caec233dc404a20c35d8 |
| Head SHA | eb96861b17711f7d525cedacccc17905bc17e6e0 |
| Remote tag | v0.7.97 |
| Remote tag_peel | eb96861b17711f7d525cedacccc17905bc17e6e0 |
| Peel match | true |

## Release notes

- B-direct mechanical refactor cycle. Closes FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC.
- Extracts the 2169-line inline `mod tests { ... }` block from
  `crates/chronos-services/src/counterexample.rs` (lines 1786-3955)
  into a sibling file `crates/chronos-services/src/ce_services_tests.rs`
  using the `#[path = "..."]` submodule pattern established by m9-94.
- After m9-95: `counterexample.rs` shrinks from 3955 → 1794 lines
  (production code only); `ce_services_tests.rs` holds the 45 test
  functions + 4 test helpers.
- No public API change; no behavior change; tests use `use super::*;`
  so the parent's private items remain accessible via the `#[path]`
  submodule declaration.
- No new tests added (all 45 tests + 4 helpers moved verbatim).
- Workspace lib tests: 1042 pass (no regressions vs m9-94 baseline).
- Chronos-services lib tests: 268 pass (same as m9-94).

## Cross-checks

- `bash scripts/check_vault_drift.sh`: clean (no vault drift introduced by source commit).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean (after vault cascade).
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p chronos-services --lib --no-fail-fast`: 268 pass.
- `cargo test --workspace --lib --no-fail-fast -- --test-threads=1`: 1042 pass.
- `apply-checkpoint.peel_match == true`.
- `apply-checkpoint.head_sha == release-receipt.Head SHA == eb96861`.
- `Remote tag` v0.7.97 peel: `eb96861` (source commit / cycle-artifacts commit).
